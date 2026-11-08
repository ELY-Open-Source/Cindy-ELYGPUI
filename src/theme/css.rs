use anyhow::{Context as _, Result, anyhow, bail};
use gpui::Rgba;

/// Evaluates a CSS color as Cindy's tokens write them, `var()` already expanded.
pub fn color(text: &str) -> Result<Rgba> {
    let text = text.trim();
    if text == "transparent" {
        return Ok(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    let (name, args) = function(text).with_context(|| format!("not a color: {text}"))?;
    match name {
        "rgb" | "rgba" => rgb(args),
        "hsl" | "hsla" => hsl(args),
        "color-mix" => mix(args),
        "oklch" => relative_oklch(args),
        _ => bail!("unsupported color function {name}: {text}"),
    }
}

fn function(text: &str) -> Option<(&str, &str)> {
    let open = text.find('(')?;
    let args = text[open + 1..].strip_suffix(')')?;
    Some((&text[..open], args))
}

fn parse_hex(hex: &str) -> Result<Rgba> {
    let digits: Vec<u8> = match hex.len() {
        3 | 4 => hex
            .chars()
            .map(|c| c.to_string().repeat(2))
            .collect::<Vec<_>>()
            .concat(),
        6 | 8 => hex.to_owned(),
        _ => bail!("bad hex color #{hex}"),
    }
    .as_bytes()
    .chunks(2)
    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair)?, 16).map_err(Into::into))
    .collect::<Result<_>>()
    .with_context(|| format!("bad hex color #{hex}"))?;
    let channel = |i: usize| digits.get(i).map_or(1.0, |v| f32::from(*v) / 255.0);
    Ok(Rgba {
        r: channel(0),
        g: channel(1),
        b: channel(2),
        a: channel(3),
    })
}

/// Splits on commas and spaces outside parentheses; `/` stays its own part.
fn parts(args: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, ch) in args.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' | ' ' | '/' if depth == 0 => {
                if start < i {
                    out.push(&args[start..i]);
                }
                if ch == '/' {
                    out.push("/");
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < args.len() {
        out.push(&args[start..]);
    }
    out
}

fn number(part: &str) -> Result<(f32, bool)> {
    let (digits, percent) = match part.strip_suffix('%') {
        Some(digits) => (digits, true),
        None => (part.strip_suffix("deg").unwrap_or(part), false),
    };
    let value = digits
        .parse()
        .with_context(|| format!("bad number {part}"))?;
    Ok((value, percent))
}

fn channels(args: &str) -> Result<([(f32, bool); 3], f32)> {
    let parts: Vec<&str> = parts(args).into_iter().filter(|p| *p != "/").collect();
    let alpha = match parts.get(3) {
        Some(part) => {
            let (value, percent) = number(part)?;
            if percent { value / 100.0 } else { value }
        }
        None => 1.0,
    };
    if parts.len() < 3 || parts.len() > 4 {
        bail!("expected 3 or 4 channels in ({args})");
    }
    Ok((
        [number(parts[0])?, number(parts[1])?, number(parts[2])?],
        alpha,
    ))
}

fn rgb(args: &str) -> Result<Rgba> {
    let (c, a) = channels(args)?;
    let v = |(value, percent): (f32, bool)| {
        if percent {
            value / 100.0
        } else {
            value / 255.0
        }
    };
    Ok(Rgba {
        r: v(c[0]),
        g: v(c[1]),
        b: v(c[2]),
        a,
    })
}

fn hsl(args: &str) -> Result<Rgba> {
    let ([(h, _), (s, _), (l, _)], a) = channels(args)?;
    let (s, l) = (s / 100.0, l / 100.0);
    let k = |n: f32| (n + h / 30.0).rem_euclid(12.0);
    let f = |n: f32| l - s * l.min(1.0 - l) * (k(n) - 3.0).min(9.0 - k(n)).clamp(-1.0, 1.0);
    Ok(Rgba {
        r: f(0.0),
        g: f(8.0),
        b: f(4.0),
        a,
    })
}

/// `color-mix(in srgb, a p1, b p2)`, premultiplied as CSS Color 5 mixes.
fn mix(args: &str) -> Result<Rgba> {
    let mut items = split_top(args, ',').into_iter().map(str::trim);
    let space = items.next().context("color-mix without a space")?;
    if space != "in srgb" {
        bail!("color-mix space {space} is not supported");
    }
    let mut side = || -> Result<(Rgba, Option<f32>)> {
        let item = items.next().context("color-mix needs two colors")?;
        match item.rsplit_once(' ') {
            Some((color_text, pct)) if pct.ends_with('%') => {
                Ok((color(color_text)?, Some(number(pct)?.0 / 100.0)))
            }
            _ => Ok((color(item)?, None)),
        }
    };
    let ((a, pa), (b, pb)) = (side()?, side()?);
    let (pa, pb) = match (pa, pb) {
        (Some(pa), Some(pb)) => (pa / (pa + pb), pb / (pa + pb)),
        (Some(pa), None) => (pa, 1.0 - pa),
        (None, Some(pb)) => (1.0 - pb, pb),
        (None, None) => (0.5, 0.5),
    };
    let alpha = a.a * pa + b.a * pb;
    if alpha == 0.0 {
        return Ok(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }
    let ch = |x: f32, y: f32| (x * a.a * pa + y * b.a * pb) / alpha;
    Ok(Rgba {
        r: ch(a.r, b.r),
        g: ch(a.g, b.g),
        b: ch(a.b, b.b),
        a: alpha,
    })
}

fn split_top(text: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out
}

/// `oklch(from <color> <l> c h)`, the form `filledButtonStates.ts` writes.
fn relative_oklch(args: &str) -> Result<Rgba> {
    let rest = args
        .trim()
        .strip_prefix("from ")
        .context("oklch without from")?;
    let parts = parts(rest);
    let [base, lightness, "c", "h"] = parts.as_slice() else {
        bail!("unsupported oklch form: oklch({args})");
    };
    let base = color(base)?;
    let (l, c, h) = super::oklch::from_srgb(base);
    let l = match function(lightness) {
        Some(("calc", expr)) => calc(expr, l)?,
        _ => bail!("unsupported oklch lightness {lightness}"),
    };
    Ok(super::oklch::to_srgb(l, c, h, base.a))
}

/// `calc()` over numbers and `l`: `+ - *`, parentheses, `sign()`, `max()`.
fn calc(expr: &str, l: f32) -> Result<f32> {
    let tokens: Vec<String> = expr
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace(',', " , ")
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut at = 0;
    let value = sum(&tokens, &mut at, l)?;
    if at != tokens.len() {
        bail!("trailing tokens in calc({expr})");
    }
    Ok(value)
}

fn sum(t: &[String], at: &mut usize, l: f32) -> Result<f32> {
    let mut value = product(t, at, l)?;
    while let Some(op) = t.get(*at).filter(|op| *op == "+" || *op == "-") {
        let plus = op == "+";
        *at += 1;
        let rhs = product(t, at, l)?;
        value = if plus { value + rhs } else { value - rhs };
    }
    Ok(value)
}

fn product(t: &[String], at: &mut usize, l: f32) -> Result<f32> {
    let mut value = atom(t, at, l)?;
    while t.get(*at).is_some_and(|op| op == "*") {
        *at += 1;
        value *= atom(t, at, l)?;
    }
    Ok(value)
}

fn expect(t: &[String], at: &mut usize, want: &str) -> Result<()> {
    match t.get(*at) {
        Some(token) if token == want => {
            *at += 1;
            Ok(())
        }
        other => Err(anyhow!("calc expected {want}, found {other:?}")),
    }
}

fn atom(t: &[String], at: &mut usize, l: f32) -> Result<f32> {
    let token = t.get(*at).context("calc ended early")?.clone();
    *at += 1;
    match token.as_str() {
        "l" => Ok(l),
        "(" => {
            let value = sum(t, at, l)?;
            expect(t, at, ")")?;
            Ok(value)
        }
        "sign" | "max" => {
            expect(t, at, "(")?;
            let first = sum(t, at, l)?;
            let value = if token == "sign" {
                if first > 0.0 {
                    1.0
                } else if first < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            } else {
                expect(t, at, ",")?;
                first.max(sum(t, at, l)?)
            };
            expect(t, at, ")")?;
            Ok(value)
        }
        number => number
            .parse()
            .with_context(|| format!("calc cannot read {number}")),
    }
}
