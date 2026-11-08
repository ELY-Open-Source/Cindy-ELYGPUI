use anyhow::{Result, bail};

use super::color::{
    BLACK, Rgb, WHITE, contrast_ratio, mix, to_hex, to_hsl_triplet, to_rgba_string,
};
use crate::theme::Kind;

/// Cindy's 13 palette roles.
#[derive(Clone, Debug)]
pub struct ThemePalette {
    pub surface: Rgb,
    pub elevated: Rgb,
    pub elevated_soft: Rgb,
    pub hover: Rgb,
    pub chip: Rgb,
    pub border: Rgb,
    pub text_primary: Rgb,
    pub text_secondary: Rgb,
    pub text_tertiary: Rgb,
    pub text_disabled: Rgb,
    pub accent_primary: Rgb,
    pub accent_soft: Rgb,
    pub accent_deep: Rgb,
    pub input_bg: Option<Rgb>,
}

/// Heading and bold colors; tokens come only for those given.
#[derive(Clone, Debug, Default)]
pub struct MarkdownPalette {
    pub headings: Option<Vec<Option<Rgb>>>,
    pub strong: Option<Rgb>,
}

const SWITCH_TARGET: f64 = 3.2;
const SEARCH_STEPS: u32 = 255;

fn minimum_contrast(color: Rgb, against: &[Rgb]) -> f64 {
    against
        .iter()
        .map(|other| contrast_ratio(color, *other))
        .fold(f64::INFINITY, f64::min)
}

/// Cindy's `deriveUncheckedSwitchTrack`.
pub fn derive_unchecked_switch_track(preferred: Rgb, against: &[Rgb]) -> Result<Rgb> {
    if minimum_contrast(preferred, against) >= SWITCH_TARGET {
        return Ok(preferred);
    }
    let mut best: Option<(Rgb, f64, f64)> = None;
    for target in [BLACK, WHITE] {
        for step in 1..=SEARCH_STEPS {
            let candidate = mix(preferred, target, f64::from(step) / f64::from(SEARCH_STEPS));
            let contrast = minimum_contrast(candidate, against);
            if contrast < SWITCH_TARGET {
                continue;
            }
            let distance = (candidate.r - preferred.r).powi(2)
                + (candidate.g - preferred.g).powi(2)
                + (candidate.b - preferred.b).powi(2);
            let better = match best {
                None => true,
                Some((_, d, c)) => distance < d || (distance == d && contrast > c),
            };
            if better {
                best = Some((candidate, distance, contrast));
            }
            break;
        }
    }
    match best {
        Some((color, ..)) => Ok(color),
        None => bail!("Unable to derive unchecked Switch colors with 3.2:1 contrast"),
    }
}

/// Cindy's `buildThemeColorsFromPalette`, in its key order.
pub fn build_theme_colors(
    p: &ThemePalette,
    kind: Kind,
    markdown: &MarkdownPalette,
) -> Result<Vec<(String, String)>> {
    let dark = kind == Kind::Dark;
    let pick = |d: &str, l: &str| if dark { d.to_owned() } else { l.to_owned() };
    let (surface, elevated, elevated_soft) = (
        to_hex(p.surface),
        to_hex(p.elevated),
        to_hex(p.elevated_soft),
    );
    let (hover, chip, border) = (to_hex(p.hover), to_hex(p.chip), to_hex(p.border));
    let text_primary = to_hex(p.text_primary);
    let text_secondary = to_hex(p.text_secondary);
    let text_tertiary = to_hex(p.text_tertiary);
    let text_disabled = to_hex(p.text_disabled);
    let (accent, accent_soft, accent_deep) = (
        to_hex(p.accent_primary),
        to_hex(p.accent_soft),
        to_hex(p.accent_deep),
    );
    let white = to_hex(WHITE);
    let surface_hsl = to_hsl_triplet(p.surface);
    let elevated_hsl = to_hsl_triplet(p.elevated);
    let hover_hsl = to_hsl_triplet(p.hover);
    let chip_hsl = to_hsl_triplet(p.chip);
    let border_hsl = to_hsl_triplet(p.border);
    let text_primary_hsl = to_hsl_triplet(p.text_primary);
    let text_secondary_hsl = to_hsl_triplet(p.text_secondary);
    let text_tertiary_hsl = to_hsl_triplet(p.text_tertiary);

    let default_on_accent = if dark { p.surface } else { WHITE };
    let on_accent = if contrast_ratio(p.accent_primary, default_on_accent) >= 3.0 {
        pick(&surface, &white)
    } else if contrast_ratio(p.accent_primary, WHITE) >= contrast_ratio(p.accent_primary, BLACK) {
        white.clone()
    } else {
        to_hex(BLACK)
    };
    let accent_readable = pick(&accent_soft, &accent_deep);
    let link = pick(&accent_soft, &accent);
    let muted_icon = pick(&text_disabled, &text_tertiary);
    let input_bg = to_hex(p.input_bg.unwrap_or(p.surface));
    let lifts = p.input_bg.is_some_and(|bg| to_hex(bg) != to_hex(p.surface));
    let quick_hover = to_hex(if lifts { p.surface } else { p.hover });
    let quick_icon_bg = if lifts {
        to_hex(p.surface)
    } else {
        pick(&to_hex(p.border), &to_hex(p.chip))
    };
    let thumb_off = if dark { p.surface } else { p.elevated };
    let track_off = to_hex(derive_unchecked_switch_track(
        p.text_secondary,
        &[p.surface, p.elevated, p.hover, thumb_off],
    )?);

    let s = |v: &str| v.to_owned();
    let mut colors: Vec<(&str, String)> = vec![
        ("surface", s(&surface)),
        ("surface-hsl", s(&surface_hsl)),
        ("surface-elevated", s(&elevated)),
        ("surface-elevated-soft", pick(&elevated, &elevated_soft)),
        ("surface-card-ivory", s(&elevated)),
        ("surface-chip", s(&chip)),
        ("surface-chip-alt", pick(&chip, &elevated_soft)),
        ("surface-hover", s(&hover)),
        ("surface-hover-soft", pick(&hover, &surface)),
        ("surface-hover-hsl", s(&hover_hsl)),
        ("surface-on-card", pick(&surface, &elevated)),
        ("border-default", s(&border)),
        ("border-default-hsl", s(&border_hsl)),
        ("border-shadcn-hsl", s(&border_hsl)),
        ("border-transparent-mixed", pick(&border, "transparent")),
        ("text-primary", s(&text_primary)),
        ("text-primary-on-dark", s(&text_primary)),
        ("text-primary-emphasis", s(&text_primary)),
        ("text-primary-inv", s(&text_primary)),
        ("text-primary-body-strong", s(&text_primary)),
        ("text-primary-hsl", s(&text_primary_hsl)),
        ("text-secondary", s(&text_secondary)),
        ("text-secondary-cross", s(&text_secondary)),
        ("text-secondary-mid", s(&text_secondary)),
        ("text-tertiary", s(&text_tertiary)),
        ("text-tertiary-stone", s(&text_tertiary)),
        ("text-tertiary-mid", s(&text_tertiary)),
        ("text-tertiary-hsl", s(&text_tertiary_hsl)),
        ("text-disabled", s(&text_disabled)),
        ("text-disabled-tertiary", s(&text_disabled)),
        ("accent-cta-bg", s(&accent)),
        ("accent-cta-bg-pure", s(&accent)),
        ("accent-emphasis", s(&accent)),
        ("accent-soft", pick(&accent_soft, &accent_deep)),
        ("accent-hover", s(&accent_deep)),
        ("accent-pure-cta-fg", s(&on_accent)),
        ("status-badge-fg", s("#1F1F1F")),
        ("accent", s(&hover_hsl)),
        ("agent-actions-rail", s(&border)),
        ("ask-checkbox-border", s(&muted_icon)),
        ("background", s(&surface_hsl)),
        ("chat-input-chip-border", s(&border)),
        ("chat-input-text", s(&text_primary)),
        ("color-primary", s(&text_primary)),
        ("confirm-bg", s(&elevated)),
        ("confirm-btn-primary-bg", s(&accent)),
        ("confirm-btn-primary-text", s(&on_accent)),
        ("confirm-btn-secondary-border", s(&border)),
        (
            "confirm-btn-secondary-hover",
            pick("rgba(255, 255, 255, 0.06)", "rgba(0, 0, 0, 0.06)"),
        ),
        ("confirm-btn-secondary-text", s(&text_primary)),
        ("confirm-title", s(&text_primary)),
        ("create-agent-control-bg", s(&input_bg)),
        ("create-agent-control-bg-hover", s(&quick_hover)),
        ("create-agent-control-bg-pressed", pick(&chip, &hover)),
        ("create-agent-control-border", s(&border)),
        ("create-agent-control-icon", s(&text_primary)),
        ("create-agent-control-text", s(&text_primary)),
        ("create-agent-quick-card-bg", s(&input_bg)),
        ("create-agent-quick-card-bg-hover", s(&quick_hover)),
        ("create-agent-quick-card-border", s(&border)),
        ("create-agent-quick-card-icon", s(&text_primary)),
        ("create-agent-quick-card-icon-bg", s(&quick_icon_bg)),
        ("create-agent-quick-card-text", s(&text_primary)),
        (
            "create-agent-segment-inactive-text",
            pick(&text_tertiary, &text_secondary),
        ),
        ("create-agent-segment-track-bg", s(&input_bg)),
        ("drop-overlay-bg", to_rgba_string(p.accent_primary, 0.1)),
        ("file-chip-bg", pick(&border, &chip)),
        ("file-remove-bg", s(&muted_icon)),
        ("info-700", s(&accent_readable)),
        ("model-trigger-hover", s(&hover)),
        ("msg-link", s(&link)),
        ("msg-scrollbar-hover", s(&muted_icon)),
        ("muted", s(&chip_hsl)),
        ("muted-foreground", s(&text_secondary_hsl)),
        ("perm-allow-btn-bg", s(&accent)),
        ("perm-allow-btn-text", s(&on_accent)),
        ("perm-allow-kbd-bg", s(&chip)),
        ("perm-allow-kbd-border", s(&border)),
        ("perm-code-bg", pick(&surface, &elevated_soft)),
        ("perm-item-selected-bg", s(&chip)),
        ("plan-outline-active-bg", s(&chip)),
        ("plan-toolbar-btn-hover-bg", s(&hover)),
        ("popover", s(&elevated_hsl)),
        ("primary-foreground", pick(&surface_hsl, "0 0% 100%")),
        ("search-match-fg", s(&text_primary_hsl)),
        ("secondary", pick(&elevated_hsl, &chip_hsl)),
        ("send-btn-bg", pick(&text_primary, &surface)),
        ("send-btn-icon", pick(&surface, &text_primary)),
        ("settings-btn-primary-text", s(&on_accent)),
        ("settings-btn-secondary-hover-bg", s(&hover)),
        ("text-placeholder", s(&text_disabled)),
        ("settings-integration-avatar-bg", s(&chip)),
        ("settings-logout-bg", s(&chip)),
        ("settings-menu-bg-hover", s(&hover)),
        ("settings-menu-bg-selected", s(&chip)),
        ("settings-source-link", s(&link)),
        ("settings-theme-auto-dark", s(&surface)),
        ("switch-thumb-off", to_hex(thumb_off)),
        ("switch-track-off", track_off),
        ("sidebar-action-icon", s(&text_tertiary_hsl)),
        ("sidebar-item-active", s(&chip_hsl)),
        ("splash-bg", s(&surface_hsl)),
        ("splash-text", s(&text_secondary_hsl)),
        ("splash-text-destructive", s(&text_primary_hsl)),
        ("splash-text-muted", s(&text_tertiary_hsl)),
        ("titlebar-icon", s(&text_secondary_hsl)),
        ("tooltip-bg", pick(&surface, &text_primary)),
        ("tooltip-text", pick(&text_primary, &surface)),
        ("update-btn-border", s(&accent)),
        ("update-btn-text", s(&accent)),
    ];
    let headings = [
        "md-h1-fg", "md-h2-fg", "md-h3-fg", "md-h4-fg", "md-h5-fg", "md-h6-fg",
    ];
    for (index, token) in headings.into_iter().enumerate() {
        let value = markdown
            .headings
            .as_ref()
            .and_then(|h| h.get(index).copied().flatten());
        if let Some(value) = value {
            colors.push((token, to_hex(value)));
        }
    }
    if let Some(strong) = markdown.strong {
        colors.push(("md-strong-fg", to_hex(strong)));
    }
    Ok(colors.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}
