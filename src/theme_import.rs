pub mod color;
pub mod palette;
pub mod vscode;

use anyhow::Result;

use crate::theme::Kind;

const PROTECTED_PREFIXES: [&str; 4] = ["login-", "diff-", "error-", "overlay-"];
const PROTECTED_IDS: [&str; 24] = [
    "destructive",
    "warning-accent",
    "warning-bg-soft",
    "warning-fg",
    "status-bar-accent",
    "settings-integration-warning",
    "plan-action-approve-icon-bg",
    "plan-action-approve-icon-fg",
    "perm-auto-selected-text",
    "perm-bypass-selected-text",
    "process-agent-task-icon",
    "process-agent-service-icon",
    "process-main-icon",
    "process-renderer-icon",
    "process-gpu-icon",
    "process-utility-icon",
    "pr-open-on-light",
    "pr-open-on-dark",
    "focus-ring",
    "focus-ring-soft",
    "text-selection-bg",
    "shadow-menu",
    "cmd-palette-shadow",
    "confirm-shadow",
];

/// Cindy's `isProtectedToken`: tokens an imported theme never sets.
pub fn is_protected(id: &str) -> bool {
    PROTECTED_IDS.contains(&id) || PROTECTED_PREFIXES.iter().any(|p| id.starts_with(p))
}

/// One converted theme, colors in Cindy's key order.
pub struct ConvertedTheme {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the import button reads it, chapter 12")
    )]
    pub name: String,
    pub kind: Kind,
    pub colors: Vec<(String, String)>,
}

/// Cindy's `ThemeConversionReport`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the import button reads it, chapter 12")
)]
pub struct Report {
    pub resolved_roles: usize,
    pub derived_roles: Vec<String>,
    pub unresolved: Vec<String>,
    pub skipped_protected: usize,
}

pub struct Conversion {
    pub themes: Vec<ConvertedTheme>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the import button reads it, chapter 12")
    )]
    pub report: Report,
}

/// Cindy's `convertVsCodeTheme`: none when the text is not a color theme.
pub fn convert_vscode_theme(raw: &str, fallback_name: &str) -> Result<Option<Conversion>> {
    let Some(parsed) = vscode::parse_theme_json(raw) else {
        return Ok(None);
    };
    let Some(extracted) = vscode::extract_palette(&parsed, fallback_name) else {
        return Ok(parsed.include.map(|include| Conversion {
            themes: Vec::new(),
            report: Report {
                resolved_roles: 0,
                derived_roles: Vec::new(),
                unresolved: vec![format!("include:{include}")],
                skipped_protected: 0,
            },
        }));
    };
    let built =
        palette::build_theme_colors(&extracted.palette, extracted.kind, &extracted.markdown)?;
    let (skipped, colors): (Vec<_>, Vec<_>) =
        built.into_iter().partition(|(id, _)| is_protected(id));
    Ok(Some(Conversion {
        themes: vec![ConvertedTheme {
            name: extracted.name,
            kind: extracted.kind,
            colors,
        }],
        report: Report {
            resolved_roles: extracted.resolved_roles,
            derived_roles: extracted.derived_roles,
            unresolved: extracted.unresolved,
            skipped_protected: skipped.len(),
        },
    }))
}

#[cfg(test)]
mod tests;
