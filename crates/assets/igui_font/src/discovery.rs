//! System font discovery: scan font directories into face metadata.
//!
//! Metadata only — the file bytes are read to parse the `name`/`OS/2`/`cmap`
//! tables and then dropped. A face's bytes are loaded (and leaked) lazily by
//! [`crate::face::FontFace::load`] the first time it is used.

use std::path::{Path, PathBuf};

use igui_core::FontWeight;

/// Sorted, merged Unicode codepoint ranges covered by one face.
///
/// Built from the cmap with only the file memory-mapped and the mapping dropped
/// afterwards, so a fallback probe never copies (or leaks) a whole font file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Coverage {
    ranges: Vec<(u32, u32)>,
}

impl Coverage {
    fn from_codepoints(mut points: Vec<u32>) -> Self {
        points.sort_unstable();
        points.dedup();
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for point in points {
            if let Some((_, end)) = ranges.last_mut() {
                if point == end.saturating_add(1) {
                    *end = point;
                    continue;
                }
            }
            ranges.push((point, point));
        }
        Self { ranges }
    }

    pub(crate) fn contains(&self, ch: char) -> bool {
        let point = ch as u32;
        self.ranges
            .binary_search_by(|&(start, end)| {
                if point < start {
                    std::cmp::Ordering::Greater
                } else if point > end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }
}

/// Reads the Unicode coverage of `file`/`index` without keeping the file.
pub(crate) fn coverage(file: &Path, index: u32) -> Option<Coverage> {
    let mapping = map_file(file)?;
    let face = ttf_parser::Face::parse(&mapping, index).ok()?;
    let cmap = face.tables().cmap?;
    let mut points = Vec::new();
    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|point| {
            let Some(ch) = char::from_u32(point) else {
                return;
            };
            // `codepoints` also lists mappings whose glyph id is 0; only keep
            // entries the face can actually draw.
            if face.glyph_index(ch).is_some() {
                points.push(point);
            }
        });
    }
    Some(Coverage::from_codepoints(points))
}

/// One discovered face: enough to resolve and load it, without the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceInfo {
    pub(crate) family: String,
    pub(crate) weight: FontWeight,
    pub(crate) file: PathBuf,
    pub(crate) index: u32,
}

/// Scans `dirs` recursively for font files and returns the normal-style faces,
/// sorted deterministically by `(family, weight, file, index)`.
pub(crate) fn scan(dirs: &[PathBuf]) -> Vec<FaceInfo> {
    let mut faces = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        walk(dir, 0, &mut |path| {
            if !has_font_extension(path) {
                return;
            }
            collect_file(path, &mut faces);
        });
    }
    faces.sort_by(|a, b| {
        a.family
            .cmp(&b.family)
            .then(a.weight.cmp(&b.weight))
            .then(a.file.cmp(&b.file))
            .then(a.index.cmp(&b.index))
    });
    faces.dedup_by(|a, b| {
        a.family == b.family && a.weight == b.weight && a.file == b.file && a.index == b.index
    });
    faces
}

/// Per-OS default font directories.
pub(crate) fn system_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/System/Library/Fonts/Supplemental"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = home_dir() {
            dirs.push(home.join("Library/Fonts"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = home_dir() {
            dirs.push(home.join(".fonts"));
            dirs.push(home.join(".local/share/fonts"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        dirs.push(PathBuf::from("C:/Windows/Fonts"));
    }
    dirs
}

/// Additional macOS asset directories that hold optional system fonts
/// (PingFang lives under `AssetsV2/com_apple_MobileAsset_Font*`).
#[cfg(target_os = "macos")]
pub(crate) fn asset_dirs() -> Vec<PathBuf> {
    let root = PathBuf::from("/System/Library/AssetsV2");
    if !root.is_dir() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with("com_apple_MobileAsset_Font")
                })
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn asset_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// Memory-maps `path` read-only.
///
/// Discovery only reads the `name` / `OS/2` tables, so mapping lets the OS
/// fault in just the pages it touches instead of copying a whole (possibly
/// tens-of-MB) collection into a heap buffer.
pub(crate) fn map_file(path: &Path) -> Option<memmap2::Mmap> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: the file is opened read-only and is only ever read through this
    // shared mapping; the font files are system-installed and not truncated
    // while mapped (the same assumption the previous `fs::read` made).
    unsafe { memmap2::Mmap::map(&file).ok() }
}

/// Parses every face of `path` and appends its normal-style metadata.
fn collect_file(path: &Path, out: &mut Vec<FaceInfo>) {
    let Some(mapping) = map_file(path) else {
        return;
    };
    let bytes: &[u8] = &mapping;
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    for index in 0..count {
        let Ok(face) = ttf_parser::Face::parse(bytes, index) else {
            continue;
        };
        if face.is_italic() || face.is_oblique() {
            continue;
        }
        let Some(family) = family_name(&face) else {
            continue;
        };
        out.push(FaceInfo {
            family,
            weight: FontWeight::new(face.weight().to_number()),
            file: path.to_path_buf(),
            index,
        });
    }
}

/// Best English family name from a face's `name` table (id 1, else 16).
pub(crate) fn family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    let names = face.names();
    for wanted in [1u16, 16] {
        let mut fallback = None;
        for name in names {
            if name.name_id != wanted {
                continue;
            }
            let Some(text) = name_text(&name) else {
                continue;
            };
            if name.language() == ttf_parser::Language::English_UnitedStates {
                return Some(text);
            }
            fallback.get_or_insert(text);
        }
        if fallback.is_some() {
            return fallback;
        }
    }
    None
}

/// Decodes a name record to text.
///
/// `ttf_parser::Name::to_string` only decodes Unicode/Windows records. Core
/// macOS faces such as `Menlo` and `Monaco` carry only Macintosh/Roman name
/// records, so without this fallback they never enter discovery and a request
/// for them silently resolves to the proportional default family.
fn name_text(name: &ttf_parser::name::Name<'_>) -> Option<String> {
    if let Some(text) = name.to_string() {
        return Some(text);
    }
    if name.platform_id == ttf_parser::PlatformId::Macintosh && name.encoding_id == 0 {
        return Some(name.name.iter().map(|&byte| mac_roman(byte)).collect());
    }
    None
}

/// Mac OS Roman (`platform 1`, `encoding 0`) -> Unicode.
fn mac_roman(byte: u8) -> char {
    const HIGH: [char; 128] = [
        'Ä', 'Å', 'Ç', 'É', 'Ñ', 'Ö', 'Ü', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ç', 'é', 'è', 'ê', 'ë',
        'í', 'ì', 'î', 'ï', 'ñ', 'ó', 'ò', 'ô', 'ö', 'õ', 'ú', 'ù', 'û', 'ü', '†', '°', '¢', '£',
        '§', '•', '¶', 'ß', '®', '©', '™', '´', '¨', '≠', 'Æ', 'Ø', '∞', '±', '≤', '≥', '¥', 'µ',
        '∂', '∑', '∏', 'π', '∫', 'ª', 'º', 'Ω', 'æ', 'ø', '¿', '¡', '¬', '√', 'ƒ', '≈', '∆', '«',
        '»', '…', '\u{a0}', 'À', 'Ã', 'Õ', 'Œ', 'œ', '–', '—', '“', '”', '‘', '’', '÷', '◊', 'ÿ',
        'Ÿ', '⁄', '€', '‹', '›', 'ﬁ', 'ﬂ', '‡', '·', '‚', '„', '‰', 'Â', 'Ê', 'Á', 'Ë', 'È', 'Í',
        'Î', 'Ï', 'Ì', 'Ó', 'Ô', '\u{f8ff}', 'Ò', 'Ú', 'Û', 'Ù', 'ı', 'ˆ', '˜', '¯', '˘', '˙', '˚',
        '¸', '˝', '˛', 'ˇ',
    ];
    if byte < 0x80 {
        byte as char
    } else {
        HIGH[usize::from(byte - 0x80)]
    }
}

fn has_font_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("ttf" | "otf" | "ttc" | "otc")
    )
}

/// Depth-limited recursive walk (fonts are shallow; assets are not).
fn walk(dir: &Path, depth: usize, visit: &mut impl FnMut(&Path)) {
    const MAX_DEPTH: usize = 6;
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk(&path, depth + 1, visit);
        } else {
            visit(&path);
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_merges_contiguous_codepoints() {
        let coverage = Coverage::from_codepoints(vec![0x44, 0x41, 0x42, 0x42, 0x50]);
        assert!(coverage.contains('A'));
        assert!(coverage.contains('B'));
        assert!(!coverage.contains('C'));
        assert!(coverage.contains('D'));
        assert!(coverage.contains('P'));
        assert!(!coverage.contains('Q'));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metadata_coverage_matches_a_parsed_face() {
        let Some(mapping) = map_file(Path::new("/System/Library/Fonts/Menlo.ttc")) else {
            return;
        };
        let Some(face) = ttf_parser::Face::parse(&mapping, 0).ok() else {
            return;
        };
        let Some(coverage) = coverage(Path::new("/System/Library/Fonts/Menlo.ttc"), 0) else {
            return;
        };
        for ch in ['A', 'i', ' ', '中', '\u{1F600}'] {
            assert_eq!(
                coverage.contains(ch),
                face.glyph_index(ch).is_some(),
                "coverage mismatch for {ch:?}"
            );
        }
    }

    #[test]
    fn font_extension_is_recognized() {
        assert!(has_font_extension(Path::new("/x/Foo.ttf")));
        assert!(has_font_extension(Path::new("/x/Foo.TTC")));
        assert!(!has_font_extension(Path::new("/x/Foo.txt")));
        assert!(!has_font_extension(Path::new("/x/Foo")));
    }

    #[test]
    fn scan_missing_dir_is_empty() {
        assert!(scan(&[PathBuf::from("/definitely/not/here")]).is_empty());
    }

    #[test]
    fn mac_roman_decodes_names_beyond_ascii() {
        assert_eq!(mac_roman(b'M'), 'M');
        assert_eq!(mac_roman(0x80), 'Ä');
        assert_eq!(mac_roman(0xA5), '•');
        assert_eq!(mac_roman(0xCA), '\u{a0}');
        assert_eq!(mac_roman(0xFF), 'ˇ');
    }

    /// Core macOS faces carry Macintosh/Roman name records only; they must
    /// still be discovered or a fixed-pitch request silently falls back to the
    /// proportional default.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_core_faces_are_discovered() {
        let faces = scan(&system_dirs());
        for family in ["Menlo", "Monaco"] {
            assert!(
                faces.iter().any(|face| face.family == family),
                "{family} was not discovered"
            );
        }
    }
}
