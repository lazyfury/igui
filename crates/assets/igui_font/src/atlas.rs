//! A shelf-packing RGBA8 glyph atlas shared by every loaded face.
//!
//! Glyphs are white RGB with coverage alpha, so `texel * color` tints text.
//! One atlas holds the glyphs of all faces (regular, bold, fallback scripts),
//! so a backend still uploads a single texture.
//!
//! The atlas grows downward when a shelf no longer fits (1024² → 1024×2048 →
//! 1024×4096). Glyph slots are addressed in **texels**, so growing does not
//! invalidate cached slots or a cached draw list. `take_dirty` reports only the
//! changed rectangle; a growth reports the full atlas so the backend can
//! recreate its texture once.

/// Fixed atlas width in texels.
pub(crate) const ATLAS_WIDTH: u32 = 1024;
/// Initial atlas height in texels.
pub(crate) const ATLAS_MIN_HEIGHT: u32 = 1024;
/// Hard height cap (16 MB of RGBA at 1024 wide).
pub(crate) const ATLAS_MAX_HEIGHT: u32 = 4096;
/// Gap between packed glyphs.
const PADDING: u32 = 1;

/// A tight atlas update: `pixels` holds RGBA rows for `(x, y, width, height)`.
#[derive(Debug, Clone)]
pub struct AtlasUpdate {
    /// Full atlas size after this update.
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// Dirty rectangle inside the atlas.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA pixels for the dirty rectangle.
    pub pixels: Vec<u8>,
}

/// A simple shelf-packing RGBA8 atlas (white RGB + coverage alpha).
pub(crate) struct Atlas {
    pixels: Vec<u8>,
    height: u32,
    pen_x: u32,
    pen_y: u32,
    row_height: u32,
    /// `(x0, y0, x1, y1)` of unscanned glyph changes.
    dirty: Option<[u32; 4]>,
    /// The atlas grew since the last upload; the backend must recreate the
    /// texture and upload every pixel.
    resized: bool,
}

impl Atlas {
    pub(crate) fn new() -> Self {
        Self {
            pixels: vec![0u8; (ATLAS_WIDTH * ATLAS_MIN_HEIGHT * 4) as usize],
            height: ATLAS_MIN_HEIGHT,
            pen_x: 0,
            pen_y: 0,
            row_height: 0,
            dirty: None,
            resized: false,
        }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (ATLAS_WIDTH, self.height)
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The next update, or `None` when nothing changed.
    ///
    /// A growth returns the full atlas (one large upload and texture rebuild);
    /// otherwise the buffer covers only the dirty rectangle.
    pub(crate) fn take_dirty(&mut self) -> Option<AtlasUpdate> {
        if self.resized {
            self.resized = false;
            self.dirty = None;
            return Some(AtlasUpdate {
                atlas_width: ATLAS_WIDTH,
                atlas_height: self.height,
                x: 0,
                y: 0,
                width: ATLAS_WIDTH,
                height: self.height,
                pixels: self.pixels.clone(),
            });
        }
        let [x0, y0, x1, y1] = self.dirty.take()?;
        let width = x1 - x0;
        let height = y1 - y0;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        let stride = (ATLAS_WIDTH * 4) as usize;
        for row in y0..y1 {
            let start = row as usize * stride + (x0 * 4) as usize;
            pixels.extend_from_slice(&self.pixels[start..start + (width * 4) as usize]);
        }
        Some(AtlasUpdate {
            atlas_width: ATLAS_WIDTH,
            atlas_height: self.height,
            x: x0,
            y: y0,
            width,
            height,
            pixels,
        })
    }

    /// Reserves a `width × height` glyph cell, growing the atlas if needed.
    pub(crate) fn alloc(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 {
            return None;
        }
        if width + PADDING > ATLAS_WIDTH || height + PADDING > ATLAS_MAX_HEIGHT {
            // A single glyph larger than the atlas can never fit.
            return None;
        }
        if self.pen_x + width + PADDING > ATLAS_WIDTH {
            self.pen_x = 0;
            self.pen_y += self.row_height + PADDING;
            self.row_height = 0;
        }
        while self.pen_y + height + PADDING > self.height {
            if self.height >= ATLAS_MAX_HEIGHT {
                return None;
            }
            self.grow((self.height * 2).min(ATLAS_MAX_HEIGHT));
        }
        let origin = (self.pen_x, self.pen_y);
        self.pen_x += width + PADDING;
        self.row_height = self.row_height.max(height);
        Some(origin)
    }

    pub(crate) fn blit(&mut self, x: u32, y: u32, width: u32, height: u32, coverage: &[u8]) {
        let rect = self.dirty.get_or_insert([x, y, x + width, y + height]);
        rect[0] = rect[0].min(x);
        rect[1] = rect[1].min(y);
        rect[2] = rect[2].max(x + width);
        rect[3] = rect[3].max(y + height);
        let stride = (ATLAS_WIDTH * 4) as usize;
        for row in 0..height {
            for col in 0..width {
                let alpha = coverage[(row * width + col) as usize];
                if alpha == 0 {
                    continue;
                }
                let index =
                    y as usize * stride + x as usize * 4 + row as usize * stride + col as usize * 4;
                self.pixels[index] = 255;
                self.pixels[index + 1] = 255;
                self.pixels[index + 2] = 255;
                self.pixels[index + 3] = alpha;
            }
        }
    }

    fn grow(&mut self, height: u32) {
        let mut pixels = vec![0u8; (ATLAS_WIDTH * height * 4) as usize];
        pixels[..self.pixels.len()].copy_from_slice(&self.pixels);
        self.pixels = pixels;
        self.height = height;
        self.resized = true;
        // The resized upload covers everything, so a pending rect is redundant.
        self.dirty = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(alpha: u8) -> [u8; 4] {
        [255, 255, 255, alpha]
    }

    #[test]
    fn grows_height_and_keeps_existing_pixels() {
        let mut atlas = Atlas::new();
        let (x, y) = atlas.alloc(4, 4).expect("first cell");
        atlas.blit(x, y, 1, 1, &[200]);
        assert_eq!(atlas.size(), (ATLAS_WIDTH, ATLAS_MIN_HEIGHT));
        assert!(atlas.take_dirty().is_some());

        let mut allocs = 1;
        while atlas.size().1 == ATLAS_MIN_HEIGHT {
            assert!(atlas.alloc(32, 32).is_some(), "atlas stopped fitting cells");
            allocs += 1;
            assert!(allocs < 2048, "atlas never grew");
        }
        let (width, height) = atlas.size();
        assert_eq!(width, ATLAS_WIDTH);
        assert!(height > ATLAS_MIN_HEIGHT);
        let update = atlas.take_dirty().expect("growth upload");
        assert!(update.x == 0 && update.y == 0);
        assert_eq!((update.width, update.height), (ATLAS_WIDTH, height));
        assert_eq!(update.pixels.len(), (ATLAS_WIDTH * height * 4) as usize);
        // The first pixel survived the copy.
        let index = (y * ATLAS_WIDTH + x) as usize * 4;
        assert_eq!(&update.pixels[index..index + 4], &rgba(200));
    }

    #[test]
    fn growth_stops_at_the_cap() {
        let mut atlas = Atlas::new();
        let mut ran_out = false;
        for _ in 0..1000 {
            if atlas.alloc(256, 256).is_none() {
                ran_out = true;
                break;
            }
        }
        assert!(ran_out, "the atlas never hit its cap");
        assert_eq!(atlas.size().1, ATLAS_MAX_HEIGHT);
    }

    #[test]
    fn dirty_updates_are_tight() {
        let mut atlas = Atlas::new();
        let (x0, y0) = atlas.alloc(4, 4).expect("first");
        let (x1, y1) = atlas.alloc(4, 4).expect("second");
        atlas.blit(x0, y0, 2, 2, &[255; 4]);
        atlas.blit(x1, y1, 2, 2, &[255; 4]);
        let update = atlas.take_dirty().expect("dirty");
        assert_eq!((update.x, update.y), (x0.min(x1), y0.min(y1)));
        assert_eq!((update.width, update.height), (x1 + 2 - x0, y1 + 2 - y0));
        assert_eq!(
            update.pixels.len(),
            (update.width * update.height * 4) as usize
        );
        assert!(atlas.take_dirty().is_none());
    }

    #[test]
    fn pixels_are_white_with_coverage_alpha() {
        let mut atlas = Atlas::new();
        let (x, y) = atlas.alloc(2, 2).expect("cell");
        atlas.blit(x, y, 1, 1, &[128]);
        let update = atlas.take_dirty().expect("dirty");
        assert_eq!(&update.pixels[..4], &[255, 255, 255, 128]);
    }
}
