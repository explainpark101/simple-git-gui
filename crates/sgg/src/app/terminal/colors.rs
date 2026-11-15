//! ANSI / truecolor conversion for terminal cells (Zed-style palette).

use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui_kit::{Hsla, Rgba};

/// Dark-theme ANSI palette close to common editor terminals (incl. Zed defaults).
const ANSI: [[u8; 3]; 16] = [
    [0x0e, 0x0e, 0x0e], // black
    [0xff, 0x5c, 0x57], // red
    [0x28, 0xc8, 0x40], // green
    [0xff, 0xd3, 0x2c], // yellow
    [0x57, 0xc7, 0xff], // blue
    [0xff, 0x6a, 0xc1], // magenta
    [0x56, 0xd8, 0xc9], // cyan
    [0xf2, 0xf2, 0xf2], // white
    [0x66, 0x66, 0x66], // bright black
    [0xff, 0x5c, 0x57], // bright red
    [0x28, 0xc8, 0x40], // bright green
    [0xff, 0xd3, 0x2c], // bright yellow
    [0x57, 0xc7, 0xff], // bright blue
    [0xff, 0x6a, 0xc1], // bright magenta
    [0x56, 0xd8, 0xc9], // bright cyan
    [0xff, 0xff, 0xff], // bright white
];

const FG: [u8; 3] = [0xe0, 0xe0, 0xe0];
const BG: [u8; 3] = [0x1e, 0x1e, 0x1e];

#[inline]
pub fn rgb_to_hsla(r: u8, g: u8, b: u8) -> Hsla {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

pub fn named_rgb(named: NamedColor) -> [u8; 3] {
    match named {
        NamedColor::Black => ANSI[0],
        NamedColor::Red => ANSI[1],
        NamedColor::Green => ANSI[2],
        NamedColor::Yellow => ANSI[3],
        NamedColor::Blue => ANSI[4],
        NamedColor::Magenta => ANSI[5],
        NamedColor::Cyan => ANSI[6],
        NamedColor::White => ANSI[7],
        NamedColor::BrightBlack => ANSI[8],
        NamedColor::BrightRed => ANSI[9],
        NamedColor::BrightGreen => ANSI[10],
        NamedColor::BrightYellow => ANSI[11],
        NamedColor::BrightBlue => ANSI[12],
        NamedColor::BrightMagenta => ANSI[13],
        NamedColor::BrightCyan => ANSI[14],
        NamedColor::BrightWhite => ANSI[15],
        NamedColor::Foreground | NamedColor::BrightForeground => FG,
        NamedColor::Background => BG,
        NamedColor::Cursor => [0xff, 0xff, 0xff],
        NamedColor::DimBlack => dim(ANSI[0]),
        NamedColor::DimRed => dim(ANSI[1]),
        NamedColor::DimGreen => dim(ANSI[2]),
        NamedColor::DimYellow => dim(ANSI[3]),
        NamedColor::DimBlue => dim(ANSI[4]),
        NamedColor::DimMagenta => dim(ANSI[5]),
        NamedColor::DimCyan => dim(ANSI[6]),
        NamedColor::DimWhite => dim(ANSI[7]),
        NamedColor::DimForeground => dim(FG),
    }
}

fn dim(rgb: [u8; 3]) -> [u8; 3] {
    [
        (rgb[0] as u16 * 2 / 3) as u8,
        (rgb[1] as u16 * 2 / 3) as u8,
        (rgb[2] as u16 * 2 / 3) as u8,
    ]
}

/// 256-color cube + grayscale (same mapping Alacritty / xterm use).
pub fn indexed_rgb(index: u8) -> [u8; 3] {
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            let i = index - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = i % 6;
            let level = |n: u8| if n == 0 { 0 } else { 55 + 40 * n };
            [level(r), level(g), level(b)]
        }
        232..=255 => {
            let gray = 8 + 10 * (index - 232);
            [gray, gray, gray]
        }
    }
}

pub fn color_rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Named(named) => named_rgb(named),
        Color::Spec(Rgb { r, g, b }) => [r, g, b],
        Color::Indexed(i) => indexed_rgb(i),
    }
}

pub fn is_default_background(color: Color) -> bool {
    matches!(color, Color::Named(NamedColor::Background))
}

pub fn terminal_bg_hsla() -> Hsla {
    rgb_to_hsla(BG[0], BG[1], BG[2])
}
