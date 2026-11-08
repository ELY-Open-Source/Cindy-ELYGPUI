use ely_gpui_component::theme::Palette;

use super::{Appearance, Kind};

/// The active Cindy theme as an Ely palette; slots Cindy has no token for stay Ely's.
pub fn palette(appearance: &Appearance) -> Palette {
    let t = |token| appearance.token(token);
    let mut p = match appearance.kind() {
        Kind::Light => Palette::light(false),
        Kind::Dark => Palette::dark(false),
    };
    p.bg = t("surface");
    p.surface = t("surface-elevated");
    p.sunken = t("surface-chip");
    p.overlay = t("surface-elevated");
    p.hover = t("surface-hover");
    p.active = t("button-secondary-pressed");
    p.border = t("border-default");
    p.border_strong = t("text-tertiary");
    p.fg = t("text-primary");
    p.fg_muted = t("text-secondary");
    p.fg_subtle = t("text-tertiary");
    p.fg_disabled = t("text-disabled");
    p.accent = t("accent-cta-bg");
    p.accent_hover = t("accent-hover");
    p.on_accent = t("accent-pure-cta-fg");
    p.focus = t("focus-ring");
    p.link = t("msg-link");
    p.selection = t("text-selection-bg");
    p.success = t("status-success");
    p.warning = t("warning-fg");
    p.danger = t("error-fg");
    p.info = t("status-info");
    p.warning_subtle = t("warning-bg-soft");
    p.danger_subtle = t("danger-bg-soft");
    p.backdrop = t("overlay-modal");
    p.media_backdrop = t("overlay-lightbox");
    p.glass = t("surface-translucent-overlay");
    p.tooltip_bg = t("tooltip-bg");
    p.tooltip_fg = t("tooltip-text");
    p
}
