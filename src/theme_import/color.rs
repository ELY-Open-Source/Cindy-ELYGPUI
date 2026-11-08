/// An sRGB color, channels 0..255 as Cindy's `Rgb` keeps them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

pub const WHITE: Rgb = Rgb {
    r: 255.0,
    g: 255.0,
    b: 255.0,
};
pub const BLACK: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 0.0,
};

/// ECMAScript white space and line terminators, as `\s` and `trim()` match them.
pub fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// JavaScript's `String.prototype.trim`.
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(js_space)
}

/// JavaScript's `Math.round`: halves go up.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn clamp255(value: f64) -> f64 {
    js_round(value).clamp(0.0, 255.0)
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// JavaScript's `parseFloat`: the longest decimal literal at the start, else NaN.
fn parse_float(text: &str) -> f64 {
    let text = text.trim_start_matches(js_space);
    let b = text.as_bytes();
    let i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    if text[i..].starts_with("Infinity") {
        return if b[0] == b'-' {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let digits = |from: usize| from + b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int_end = digits(i);
    let mut end = int_end;
    if b.get(end) == Some(&b'.') {
        let frac_end = digits(end + 1);
        if frac_end > end + 1 || int_end > i {
            end = frac_end;
        }
    }
    if end == i || (end == i + 1 && b[i] == b'.') {
        return f64::NAN;
    }
    if matches!(b.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(b.get(end + 1), Some(b'+' | b'-')));
        let exp_end = digits(end + 1 + sign);
        if exp_end > end + 1 + sign {
            end = exp_end;
        }
    }
    text[..end].parse().unwrap_or(f64::NAN)
}

/// `[+-]?\d*\.?\d+`, the number in Cindy's `ANGLE_UNIT_RE`.
fn is_simple_number(text: &str) -> bool {
    let text = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (int, frac) = text.split_once('.').unwrap_or(("", text));
    !frac.is_empty()
        && int.bytes().all(|c| c.is_ascii_digit())
        && frac.bytes().all(|c| c.is_ascii_digit())
}

fn hex_parts(raw: &str) -> Option<(Rgb, f64)> {
    let body = js_trim(raw).strip_prefix('#').unwrap_or(js_trim(raw));
    if !(3..=8).contains(&body.len()) || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let (rgb6, alpha) = match body.len() {
        3 | 4 => {
            let doubled: String = body.chars().flat_map(|c| [c, c]).collect();
            let alpha = if body.len() == 4 {
                f64::from(u8::from_str_radix(&doubled[6..8], 16).ok()?) / 255.0
            } else {
                1.0
            };
            (doubled[..6].to_owned(), alpha)
        }
        6 => (body.to_owned(), 1.0),
        8 => (
            body[..6].to_owned(),
            f64::from(u8::from_str_radix(&body[6..], 16).ok()?) / 255.0,
        ),
        _ => return None,
    };
    let n = u32::from_str_radix(&rgb6, 16).ok()?;
    let channel = |shift: u32| f64::from((n >> shift) & 255);
    Some((
        Rgb {
            r: channel(16),
            g: channel(8),
            b: channel(0),
        },
        alpha,
    ))
}

/// HSL (0-360, 0-1, 0-1) to RGB, as `hslToRgb`.
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (((h % 360.0) + 360.0) % 360.0) / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp {
        hp if hp < 1.0 => (c, x, 0.0),
        hp if hp < 2.0 => (x, c, 0.0),
        hp if hp < 3.0 => (0.0, c, x),
        hp if hp < 4.0 => (0.0, x, c),
        hp if hp < 5.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Rgb {
        r: clamp255((r1 + m) * 255.0),
        g: clamp255((g1 + m) * 255.0),
        b: clamp255((b1 + m) * 255.0),
    }
}

struct Component {
    value: f64,
    percent: bool,
    unit: Option<String>,
}

fn component(part: &str) -> Component {
    if let Some(digits) = part.strip_suffix('%') {
        return Component {
            value: parse_float(digits),
            percent: true,
            unit: None,
        };
    }
    let lower = part.to_lowercase();
    let unit = ["grad", "deg", "rad", "turn"]
        .into_iter()
        .find(|u| lower.strip_suffix(u).is_some_and(is_simple_number));
    Component {
        value: parse_float(part),
        percent: false,
        unit: unit.map(str::to_owned),
    }
}

fn degrees(c: &Component) -> f64 {
    match c.unit.as_deref() {
        Some("rad") => c.value.to_degrees(),
        Some("grad") => c.value * 360.0 / 400.0,
        Some("turn") => c.value * 360.0,
        _ => c.value,
    }
}

/// `rgb()`/`hsl()` channels and alpha, as `parseNumberList` reads them.
fn functional(raw: &str) -> Option<(Rgb, f64)> {
    let raw = js_trim(raw);
    let open = raw.find('(')?;
    let name = raw[..open].to_lowercase();
    let body = raw[open + 1..].strip_suffix(')')?;
    if !matches!(name.as_str(), "rgb" | "rgba" | "hsl" | "hsla") || body.contains(')') {
        return None;
    }
    let parts: Vec<Component> = body
        .split(|c| c == ',' || c == '/' || js_space(c))
        .filter(|p| !p.is_empty())
        .take(4)
        .map(component)
        .collect();
    if parts.len() < 3 || parts.iter().any(|c| !c.value.is_finite()) {
        return None;
    }
    let rgb = if name.starts_with("rgb") {
        let channel = |c: &Component| {
            clamp255(if c.percent {
                c.value * 255.0 / 100.0
            } else {
                c.value
            })
        };
        Rgb {
            r: channel(&parts[0]),
            g: channel(&parts[1]),
            b: channel(&parts[2]),
        }
    } else {
        hsl_to_rgb(
            degrees(&parts[0]),
            clamp01(parts[1].value / 100.0),
            clamp01(parts[2].value / 100.0),
        )
    };
    let alpha = parts.get(3).map_or(1.0, |a| {
        clamp01(if a.percent { a.value / 100.0 } else { a.value })
    });
    Some((rgb, alpha))
}

/// Cindy's `parseCssColor`: hex, `rgb()` and `hsl()`; alpha is dropped.
pub fn parse_css_color(raw: &str) -> Option<Rgb> {
    if js_trim(raw).is_empty() {
        return None;
    }
    hex_parts(raw)
        .or_else(|| functional(raw))
        .map(|(rgb, _)| rgb)
}

pub fn composite_over(fg: Rgb, alpha: f64, bg: Rgb) -> Rgb {
    let a = clamp01(alpha);
    let ch = |f: f64, b: f64| clamp255(f * a + b * (1.0 - a));
    Rgb {
        r: ch(fg.r, bg.r),
        g: ch(fg.g, bg.g),
        b: ch(fg.b, bg.b),
    }
}

/// Cindy's `parseCssColorComposited`: a translucent color laid over `over`.
pub fn parse_css_color_composited(raw: &str, over: Option<Rgb>) -> Option<Rgb> {
    if js_trim(raw).is_empty() {
        return None;
    }
    let (rgb, alpha) = hex_parts(raw).or_else(|| functional(raw))?;
    match over {
        Some(bg) if alpha < 1.0 => Some(composite_over(rgb, alpha, bg)),
        _ => Some(rgb),
    }
}

pub fn to_hex(c: Rgb) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        clamp255(c.r) as u8,
        clamp255(c.g) as u8,
        clamp255(c.b) as u8
    )
}

/// `"H S% L%"`, as Cindy's `-hsl` tokens are written.
pub fn to_hsl_triplet(c: Rgb) -> String {
    let (r, g, b) = (
        clamp255(c.r) / 255.0,
        clamp255(c.g) / 255.0,
        clamp255(c.b) / 255.0,
    );
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    let (mut h, mut s) = (0.0, 0.0);
    if d > 0.0 {
        s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        h = if max == r {
            ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
        } else if max == g {
            ((b - r) / d + 2.0) * 60.0
        } else {
            ((r - g) / d + 4.0) * 60.0
        };
    }
    format!(
        "{} {}% {}%",
        js_round(h),
        js_round(s * 100.0),
        js_round(l * 100.0)
    )
}

/// JavaScript's number-to-string for the alphas Cindy writes.
pub fn to_rgba_string(c: Rgb, alpha: f64) -> String {
    format!(
        "rgba({}, {}, {}, {alpha})",
        clamp255(c.r),
        clamp255(c.g),
        clamp255(c.b)
    )
}

pub fn relative_luminance(c: Rgb) -> f64 {
    let ch = |v: f64| {
        let n = clamp255(v) / 255.0;
        if n <= 0.03928 {
            n / 12.92
        } else {
            ((n + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
}

pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

pub fn is_dark_background(c: Rgb) -> bool {
    relative_luminance(c) < 0.2
}

/// Toward white for `amount` above 0, toward black below.
pub fn shade(c: Rgb, amount: f64) -> Rgb {
    let target = if amount >= 0.0 { 255.0 } else { 0.0 };
    let t = clamp01(amount.abs());
    let ch = |v: f64| clamp255(v + (target - v) * t);
    Rgb {
        r: ch(c.r),
        g: ch(c.g),
        b: ch(c.b),
    }
}

pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let k = clamp01(t);
    let ch = |x: f64, y: f64| clamp255(x + (y - x) * k);
    Rgb {
        r: ch(a.r, b.r),
        g: ch(a.g, b.g),
        b: ch(a.b, b.b),
    }
}
