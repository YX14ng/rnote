//! Conversion between engine colors and `android.graphics.Color` ints (`0xAARRGGBB`).

use rnote_compose::Color;

/// Unpacks an `android.graphics.Color` int.
#[must_use]
pub fn color_from_argb(argb: i32) -> Color {
    let [alpha, red, green, blue] = argb.to_be_bytes();
    let channel = |value: u8| f64::from(value) / 255.0;
    Color { r: channel(red), g: channel(green), b: channel(blue), a: channel(alpha) }
}

/// Packs a color into an `android.graphics.Color` int. Channels are clamped to `0.0..=1.0`.
#[must_use]
pub fn color_to_argb(color: Color) -> i32 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    i32::from_be_bytes([channel(color.a), channel(color.r), channel(color.g), channel(color.b)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_channels_in_argb_order() {
        // 0x80FF4000: half transparent orange.
        let color = color_from_argb(i32::from_be_bytes([0x80, 0xFF, 0x40, 0x00]));

        assert_eq!(color.a, 128.0 / 255.0);
        assert_eq!((color.r, color.g, color.b), (1.0, 64.0 / 255.0, 0.0));
    }

    #[test]
    fn round_trips_through_argb() {
        let argb = i32::from_be_bytes([0xFF, 0x12, 0x34, 0x56]);

        assert_eq!(color_to_argb(color_from_argb(argb)), argb);
    }

    #[test]
    fn clamps_out_of_range_channels() {
        let color = Color { r: 2.0, g: -1.0, b: 0.5, a: 1.0 };

        assert_eq!(color_to_argb(color).to_be_bytes(), [0xFF, 0xFF, 0x00, 0x80]);
    }
}
