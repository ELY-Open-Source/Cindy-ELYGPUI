use serde_json::{Map, Value};

use super::color::{
    Rgb, is_dark_background, js_space, js_trim, parse_css_color, parse_css_color_composited, shade,
};
use super::palette::{MarkdownPalette, ThemePalette};
use crate::theme::Kind;

/// Drops jsonc comments and trailing commas, leaving strings alone.
pub fn strip_json_comments(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let (mut in_string, mut line, mut block) = (false, false, false);
    let mut i = 0;
    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        if line {
            if c == '\n' {
                line = false;
                out.push(c);
            }
        } else if block {
            if c == '*' && next == Some('/') {
                block = false;
                i += 1;
            }
        } else if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = next {
                    out.push(next);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && next == Some('/') {
            line = true;
            i += 1;
        } else if c == '/' && next == Some('*') {
            block = true;
            i += 1;
        } else {
            out.push(c);
        }
        i += 1;
    }
    strip_trailing_commas(&out)
}

fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.get(i + 1) {
                    out.push(*next);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c != ',' || !trailing(&chars[i + 1..]) {
            out.push(c);
        }
        i += 1;
    }
    out
}

fn trailing(rest: &[char]) -> bool {
    rest.iter()
        .find(|c| !js_space(**c))
        .is_some_and(|c| *c == '}' || *c == ']')
}

/// A VS Code color theme's parts Cindy reads.
pub struct VsCodeTheme {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub include: Option<String>,
    pub colors: Map<String, Value>,
    pub token_colors: Vec<Value>,
}

/// Cindy's `parseVsCodeThemeJson`: none when it is not a color theme.
pub fn parse_theme_json(raw: &str) -> Option<VsCodeTheme> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let Value::Object(parsed) = serde_json::from_str(&strip_json_comments(raw)).ok()? else {
        return None;
    };
    let colors = parsed.get("colors").and_then(Value::as_object).cloned();
    let token_colors = parsed.get("tokenColors").and_then(Value::as_array).cloned();
    if colors.is_none() && token_colors.is_none() {
        return None;
    }
    let text = |key: &str| parsed.get(key).and_then(Value::as_str).map(str::to_owned);
    Some(VsCodeTheme {
        name: text("name"),
        kind: text("type"),
        include: text("include"),
        colors: colors.unwrap_or_default(),
        token_colors: token_colors.unwrap_or_default(),
    })
}

fn scopes(scope: Option<&Value>) -> Vec<String> {
    match scope {
        Some(Value::Array(items)) => items.iter().flat_map(|s| scopes(Some(s))).collect(),
        Some(Value::String(text)) => text
            .split(',')
            .map(js_trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn read(fg: &str, over: Option<Rgb>) -> Option<Rgb> {
    match over {
        Some(_) => parse_css_color_composited(fg, over),
        None => parse_css_color(fg),
    }
}

/// A TextMate scope's foreground, last match winning, exact before prefix.
fn token_color(theme: &VsCodeTheme, wanted: &str, over: Option<Rgb>) -> Option<Rgb> {
    let mut prefix_hit = None;
    for entry in theme.token_colors.iter().rev() {
        let Some(fg) = entry
            .get("settings")
            .and_then(|s| s.get("foreground"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        for scope in scopes(entry.get("scope")) {
            if scope == wanted
                && let Some(rgb) = read(fg, over)
            {
                return Some(rgb);
            }
            if prefix_hit.is_none() && wanted.starts_with(&format!("{scope}.")) {
                prefix_hit = read(fg, over);
            }
        }
    }
    prefix_hit
}

/// What Cindy's `extractVsCodePalette` returns.
pub struct Extraction {
    pub palette: ThemePalette,
    pub kind: Kind,
    pub name: String,
    pub markdown: MarkdownPalette,
    pub derived_roles: Vec<String>,
    pub resolved_roles: usize,
    pub unresolved: Vec<String>,
}

struct Roles<'a> {
    theme: &'a VsCodeTheme,
    derived: Vec<String>,
    resolved: usize,
    unresolved: Vec<String>,
}

impl Roles<'_> {
    fn pick(&mut self, keys: &[&str], over: Option<Rgb>) -> Option<Rgb> {
        for key in keys {
            let Some(raw) = self.theme.colors.get(*key).and_then(Value::as_str) else {
                continue;
            };
            if let Some(rgb) = read(raw, over) {
                return Some(rgb);
            }
            self.unresolved.push((*key).to_owned());
        }
        None
    }

    fn role(
        &mut self,
        name: &str,
        keys: &[&str],
        over: Option<Rgb>,
        derive: impl FnOnce() -> Rgb,
    ) -> Rgb {
        match self.pick(keys, over) {
            Some(hit) => {
                self.resolved += 1;
                hit
            }
            None => {
                self.derived.push(name.to_owned());
                derive()
            }
        }
    }
}

/// Cindy's `extractVsCodePalette`.
pub fn extract_palette(theme: &VsCodeTheme, fallback_name: &str) -> Option<Extraction> {
    let mut r = Roles {
        theme,
        derived: Vec::new(),
        resolved: 0,
        unresolved: Vec::new(),
    };
    let surface = r.pick(&["editor.background", "editorPane.background"], None)?;
    r.resolved += 1;
    let kind = match theme.kind.as_deref().map(str::to_lowercase).as_deref() {
        Some("light") => Kind::Light,
        Some("dark") => Kind::Dark,
        _ if is_dark_background(surface) => Kind::Dark,
        _ => Kind::Light,
    };
    let dark = kind == Kind::Dark;
    let step = |base: Rgb, amount: f64| shade(base, if dark { amount } else { -amount });
    let over = Some(surface);
    let elevated = r.role(
        "elevated",
        &[
            "sideBar.background",
            "editorWidget.background",
            "panel.background",
            "activityBar.background",
        ],
        None,
        || step(surface, 0.05),
    );
    let hover = r.role(
        "hover",
        &[
            "list.hoverBackground",
            "toolbar.hoverBackground",
            "menu.selectionBackground",
        ],
        over,
        || step(surface, 0.08),
    );
    let chip = r.role(
        "chip",
        &[
            "list.activeSelectionBackground",
            "editorGroupHeader.tabsBackground",
            "badge.background",
        ],
        over,
        || step(hover, 0.04),
    );
    let border = r.role(
        "border",
        &[
            "panel.border",
            "editorGroup.border",
            "input.border",
            "contrastBorder",
            "widget.border",
        ],
        over,
        || step(surface, 0.2),
    );
    let text_primary = r.role(
        "textPrimary",
        &["editor.foreground", "foreground"],
        over,
        || {
            if dark {
                Rgb {
                    r: 212.0,
                    g: 212.0,
                    b: 212.0,
                }
            } else {
                Rgb {
                    r: 38.0,
                    g: 38.0,
                    b: 38.0,
                }
            }
        },
    );
    let comment = token_color(theme, "comment", over);
    if comment.is_some() {
        r.resolved += 1;
    }
    let text_secondary = match comment {
        Some(color) => color,
        None => r.role(
            "textSecondary",
            &["descriptionForeground", "editorCodeLens.foreground"],
            over,
            || shade(text_primary, if dark { -0.28 } else { 0.28 }),
        ),
    };
    let text_disabled = r.role(
        "textDisabled",
        &["editorLineNumber.foreground", "editorWhitespace.foreground"],
        over,
        || shade(text_secondary, if dark { -0.35 } else { 0.35 }),
    );
    let text_tertiary = r.role(
        "textTertiary",
        &["editorHint.foreground", "breadcrumb.foreground"],
        None,
        || shade(text_secondary, if dark { -0.16 } else { 0.16 }),
    );
    let accent_primary = r.role(
        "accentPrimary",
        &[
            "button.background",
            "focusBorder",
            "textLink.foreground",
            "activityBarBadge.background",
            "progressBar.background",
            "statusBarItem.remoteBackground",
        ],
        None,
        || text_primary,
    );
    let accent_soft = shade(accent_primary, if dark { 0.22 } else { -0.28 });
    let accent_deep = shade(accent_primary, if dark { -0.22 } else { -0.28 });
    let elevated_soft = if dark { elevated } else { step(elevated, 0.08) };
    r.derived
        .extend(["accentSoft", "accentDeep", "elevatedSoft"].map(str::to_owned));
    let heading = token_color(theme, "markup.heading", over);
    let markdown = MarkdownPalette {
        headings: heading.map(|h| vec![Some(h); 6]),
        strong: token_color(theme, "markup.bold", over),
    };
    if let Some(include) = &theme.include {
        r.unresolved.push(format!("include:{include}"));
    }
    let name = theme
        .name
        .as_deref()
        .map(js_trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(fallback_name);
    Some(Extraction {
        palette: ThemePalette {
            surface,
            elevated,
            elevated_soft,
            hover,
            chip,
            border,
            text_primary,
            text_secondary,
            text_tertiary,
            text_disabled,
            accent_primary,
            accent_soft,
            accent_deep,
            input_bg: None,
        },
        kind,
        name: name.to_owned(),
        markdown,
        derived_roles: r.derived,
        resolved_roles: r.resolved,
        unresolved: r.unresolved,
    })
}
