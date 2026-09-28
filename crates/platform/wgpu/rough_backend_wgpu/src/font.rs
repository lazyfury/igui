//! Font service re-export.
//!
//! Font loading, discovery, shaping and rasterization live in the
//! backend-neutral [`rough_font`] crate; this module keeps the backend's
//! historical names (`Font`, `FontConfig`, ...) pointing at it.

pub use rough_font::FontServer as Font;
pub use rough_font::{
    FontConfig, FontFamilyInfo, FontId, FontMetrics, FontMode, FontRequest, FontServer,
    PIXEL_GLYPH_RATIO,
};
