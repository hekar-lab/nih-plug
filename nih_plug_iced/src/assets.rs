//! Binary assets for use with `nih_plug_iced`.

use crate::core::Font;

// #[derive(Debug, Clone)]
// pub enum LoadMessage {
//     FontLoaded
// }

// This module provides a re-export and simple font wrappers around the re-exported fonts.
pub use nih_plug_assets::*;

pub const NOTO_SANS_REGULAR: Font = Font::with_name("Noto Sans Regular");
pub const NOTO_SANS_REGULAR_ITALIC: Font = Font::with_name("Noto Sans Regular Italic");
pub const NOTO_SANS_THIN: Font = Font::with_name("Noto Sans Thin");
pub const NOTO_SANS_THIN_ITALIC: Font = Font::with_name("Noto Sans Thin Italic");
pub const NOTO_SANS_LIGHT: Font = Font::with_name("Noto Sans Light");
pub const NOTO_SANS_LIGHT_ITALIC: Font = Font::with_name("Noto Sans Light Italic");
pub const NOTO_SANS_BOLD: Font = Font::with_name("Noto Sans Bold");
pub const NOTO_SANS_BOLD_ITALIC: Font = Font::with_name("Noto Sans Bold Italic");

pub fn load_fonts() {
    let _ = crate::Command::batch(vec![
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_REGULAR),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_REGULAR_ITALIC),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_THIN),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_THIN_ITALIC),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_LIGHT),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_LIGHT_ITALIC),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_BOLD),
        iced_baseview::runtime::font::load(fonts::NOTO_SANS_BOLD_ITALIC),
    ]);
}