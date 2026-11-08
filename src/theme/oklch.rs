use gpui::Rgba;

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_gamma(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB to OKLCH: lightness 0..1, chroma, hue in degrees.
pub fn from_srgb(color: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (to_linear(color.r), to_linear(color.g), to_linear(color.b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let ok_l = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let ok_a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let ok_b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (ok_l, ok_a.hypot(ok_b), ok_b.atan2(ok_a).to_degrees())
}

/// OKLCH to sRGB, clamped into the sRGB gamut.
pub fn to_srgb(l: f32, c: f32, h: f32, alpha: f32) -> Rgba {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let channel = |x: f32| to_gamma(x).clamp(0.0, 1.0);
    Rgba {
        r: channel(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        g: channel(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_),
        b: channel(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
        a: alpha,
    }
}
