//! The Projects panel: drafts, then a row per project; each row opens its level (8.7e3c2b).

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{Panels, entry_list, head, scroll};
use crate::{
    appearance_settings::text_size,
    i18n::Translator,
    lucide,
    preferences::Preferences,
    sessions::{
        fold::{self, ShowAll},
        group::Project,
        main_list::{PROJECT_LIMIT_KEY, SESSION_LIMIT_KEY, lists, text},
    },
    theme::Appearance,
    tip::tip,
    units::css,
};

/// The head counts the projects; the Cindy Make group never shows, as its tasks never load.
pub(super) fn body(window: &mut Window, cx: &mut App) -> AnyElement {
    let (drafts, projects) = lists(cx)
        .map(|l| (l.unclassified, l.rail_projects))
        .unwrap_or_default();
    let prefs = cx.global::<Preferences>();
    let (session_limit, project_limit) = (
        fold::limit(text(prefs, SESSION_LIMIT_KEY), 5),
        fold::limit(text(prefs, PROJECT_LIMIT_KEY), 20),
    );
    let p = cx.global::<Panels>();
    let (show_all, show_projects) = (p.show_all, p.show_projects);
    let now = jiff::Timestamp::now().as_millisecond();
    let since = Some(fold::recent_since(now));
    let (visible, hidden) = fold::view(&drafts, session_limit, show_all, since, |s| s.activity_at);
    let mut children = entry_list(&visible, hidden, drafts.len(), now, cx);
    let (shown, folded) = fold::view(&projects, project_limit, show_projects, None, |_| 0);
    children.extend(shown.into_iter().map(|p| row(p, window, cx)));
    if folded > 0 {
        let more = ShowAll::new("rail-panel-show-projects".into(), projects.len(), |cx| {
            cx.global_mut::<Panels>().show_projects = true
        });
        children.push(more.into_any_element());
    }
    let title = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.railNav.projects", &[]);
    div()
        .flex()
        .flex_col()
        .child(head(title, projects.len(), cx))
        .child(scroll(children))
        .into_any_element()
}

/// A project's row: folder, name, task count, chevron; its lamp and dot come with 10.0.
fn row(p: &Project, window: &mut Window, cx: &mut App) -> AnyElement {
    let id = format!("rail-project-{}", p.key);
    let focus = window
        .use_keyed_state(
            ElementId::Name(format!("{id}-focus").into()),
            cx,
            |_, cx| cx.focus_handle().tab_stop(true),
        )
        .read(cx)
        .clone();
    let appearance = cx.global::<Appearance>();
    let tertiary = appearance.token("text-tertiary");
    let selector = id.clone();
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || selector)
        .track_focus(&focus)
        .role(Role::MenuItem)
        .aria_label(p.name.clone())
        .aria_expanded(false)
        .flex()
        .h(css(32.0))
        .w_full()
        .items_center()
        .gap(css(8.0))
        .rounded(css(8.0))
        .pl(css(10.0))
        .pr(css(6.0))
        .hover(|s| s.bg(appearance.hsl_token("sidebar-item-hover")))
        .child(lucide::icon(IconName::Folder.svg(), 13.0, 2.0).text_color(tertiary))
        .child(
            div()
                .id("rail-project-name")
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(css(text_size(14.0, cx)))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(appearance.hsl_token("foreground"))
                .tooltip(tip(p.name.clone()))
                .child(p.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(css(text_size(10.0, cx)))
                .text_color(tertiary)
                .child(p.sessions.len().to_string()),
        )
        .child(lucide::icon(IconName::ChevronRight.svg(), 12.0, 2.0).text_color(tertiary))
        .into_any_element()
}

#[cfg(test)]
mod tests;
