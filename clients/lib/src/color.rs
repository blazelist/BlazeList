//! Tag color utilities shared across BlazeList clients.
//!
//! Provides hex formatting, luminance computation, and CSS style
//! generation for tag chips.

use rgb::RGB8;

/// Format an RGB8 color as a hex string like `#c07830`.
pub fn format_tag_hex(c: &RGB8) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Compute relative luminance (WCAG 2.0) of a color, range 0.0-1.0.
fn relative_luminance(c: &RGB8) -> f32 {
    fn linearize(v: u8) -> f32 {
        let s = v as f32 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
}

/// Brighten a color so it reads well on a dark background.
/// Returns a hex string with the lightened color.
fn ensure_readable(c: &RGB8) -> String {
    let lum = relative_luminance(c);
    if lum > 0.15 {
        format_tag_hex(c)
    } else {
        let factor = 0.45;
        let r = c.r as f32 + (255.0 - c.r as f32) * factor;
        let g = c.g as f32 + (255.0 - c.g as f32) * factor;
        let b = c.b as f32 + (255.0 - c.b as f32) * factor;
        format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8)
    }
}

/// Generate inline CSS style for a tag chip with the given color.
/// Returns an empty string if no color is provided.
pub fn tag_chip_style(color: &Option<RGB8>) -> String {
    match color {
        Some(c) => {
            let hex = format_tag_hex(c);
            let text = ensure_readable(c);
            format!("background: {hex}22; color: {text}; border-color: {hex}44;")
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tag_hex_basic() {
        assert_eq!(format_tag_hex(&RGB8::new(0, 0, 0)), "#000000");
        assert_eq!(format_tag_hex(&RGB8::new(255, 255, 255)), "#ffffff");
        assert_eq!(format_tag_hex(&RGB8::new(192, 120, 48)), "#c07830");
    }

    #[test]
    fn tag_chip_style_none_returns_empty() {
        assert_eq!(tag_chip_style(&None), "");
    }

    #[test]
    fn tag_chip_style_bright_color_uses_original() {
        let style = tag_chip_style(&Some(RGB8::new(255, 200, 100)));
        assert!(style.contains("background:"));
        assert!(style.contains("color:"));
        assert!(style.contains("border-color:"));
    }

    #[test]
    fn tag_chip_style_dark_color_lightens_text() {
        // Pure black has luminance 0, well below 0.15 threshold
        let style = tag_chip_style(&Some(RGB8::new(0, 0, 0)));
        // The text color should be lightened, not remain #000000
        let text_color = ensure_readable(&RGB8::new(0, 0, 0));
        assert_ne!(text_color, "#000000");
        assert!(style.contains(&format!("color: {text_color};")));
    }

    #[test]
    fn ensure_readable_bright_unchanged() {
        // White has luminance ~1.0, well above threshold
        let result = ensure_readable(&RGB8::new(255, 255, 255));
        assert_eq!(result, "#ffffff");
    }

    #[test]
    fn ensure_readable_dark_lightened() {
        let result = ensure_readable(&RGB8::new(0, 0, 0));
        // Should be lightened from #000000
        assert_ne!(result, "#000000");
    }
}
