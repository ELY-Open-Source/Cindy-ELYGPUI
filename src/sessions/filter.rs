//! `sidebarFilterCore.ts`: the sidebar filter's preferences, read and written.

use gpui::App;

use crate::preferences::Preferences;

pub const STATUS: &str = "cc-agent.sidebar.filter.status";
pub const VENDOR: &str = "cc-agent.sidebar.filter.vendor";
pub const LAST_ACTIVITY: &str = "cc-agent.sidebar.filter.lastActivity";
pub const GROUP_BY: &str = "cc-agent.sidebar.filter.groupBy";
pub const GROUP_DIALOGUE: &str = "cc-agent.sidebar.filter.groupDialogue";
pub const SORT_BY: &str = "cc-agent.sidebar.filter.sortBy";
pub const PROJECT_ORDER: &str = "cc-agent.sidebar.filter.projectOrder";

pub const STATUSES: [&str; 3] = ["active", "archived", "all"];
pub const VENDORS: [&str; 4] = ["all", "cc", "codex", "pi"];
pub const LAST_ACTIVITIES: [&str; 5] = ["1d", "3d", "7d", "30d", "all"];
pub const SORTS: [&str; 3] = ["priority", "recency", "created"];

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// The filter as `useSidebarFilter` loads it; unknown values read as the defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filter {
    pub status: &'static str,
    pub vendor: &'static str,
    pub last_activity: &'static str,
    pub flat: bool,
    pub group_dialogue: bool,
    pub sort_by: &'static str,
    pub project_order: &'static str,
}

fn one_of(raw: Option<&str>, values: &[&'static str], fallback: &'static str) -> &'static str {
    raw.and_then(|r| values.iter().find(|v| **v == r).copied())
        .unwrap_or(fallback)
}

impl Filter {
    pub fn read(prefs: &Preferences) -> Self {
        let text = |key: &str| prefs.text(key).ok().flatten();
        // `loadProjectOrder`: the old `manual` sort is the custom project order.
        let project_order = match (text(PROJECT_ORDER), text(SORT_BY)) {
            (Some("custom"), _) => "custom",
            (Some("activity"), _) => "activity",
            (_, Some("manual")) => "custom",
            _ => "activity",
        };
        Self {
            status: one_of(text(STATUS), &STATUSES, "active"),
            vendor: one_of(text(VENDOR), &VENDORS, "all"),
            last_activity: one_of(text(LAST_ACTIVITY), &LAST_ACTIVITIES, "all"),
            flat: text(GROUP_BY) == Some("flat"),
            group_dialogue: text(GROUP_DIALOGUE) != Some("false"),
            sort_by: one_of(text(SORT_BY), &SORTS, "recency"),
            project_order,
        }
    }

    /// `cutoffForLastActivity`.
    pub fn cutoff(&self, now_ms: i64) -> Option<i64> {
        let days = match self.last_activity {
            "1d" => 1,
            "3d" => 3,
            "7d" => 7,
            "30d" => 30,
            _ => return None,
        };
        Some(now_ms - days * DAY_MS)
    }

    /// Filters that narrow the tasks; projects and creators come with 13.0.
    pub fn active_count(&self) -> usize {
        usize::from(self.vendor != "all") + usize::from(self.last_activity != "all")
    }
}

/// `persist*`: one preference, logged, and the list read again when the status bucket changes.
pub fn set(key: &'static str, value: &str, cx: &mut App) {
    log::info!(target: "sessions", "filter {key} = {value}");
    if let Err(error) = cx.global_mut::<Preferences>().set(key, value) {
        log::error!(target: "sessions", "filter {key} not saved: {error:#}");
    }
    if key == STATUS {
        super::reload(cx);
    }
    cx.refresh_windows();
}

/// `resetContentFilters`: harness and last activity back to all.
pub fn reset(cx: &mut App) {
    set(VENDOR, "all", cx);
    set(LAST_ACTIVITY, "all", cx);
}
