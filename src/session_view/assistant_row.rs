//! `AssistantMessage`: an assistant message's text, drawn as Markdown with no bubble.

use std::{collections::HashMap, rc::Rc};

use gpui::{AnyElement, App, IntoElement as _, ParentElement as _, Styled as _, Window, div};

use super::{SessionView, links, markdown, session, target_link, width::Widths};
use crate::{
    chat_message::merge::{Fields, get},
    goal_verdict,
    markdown::hast::{self, Hast, Urls},
    rich_answer::{self, js},
    units::css,
};

/// The mounted content's trees: each message's shown text with the tree it parsed to.
#[derive(Default)]
pub struct Trees(HashMap<String, (String, Rc<Vec<Hast>>)>);

/// The tree of `text`, parsed once while the message shows it, as the renderer's memo keeps it.
pub(super) fn tree(key: &str, text: &str, cx: &mut App) -> Option<Rc<Vec<Hast>>> {
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    let trees = &mut shown?.content.as_mut()?.trees.0;
    if let Some((_, tree)) = trees.get(key).filter(|(held, _)| held == text) {
        return Some(tree.clone());
    }
    let tree = Rc::new(hast::render(text, true, Urls::Trusted));
    trees.insert(key.to_owned(), (text.to_owned(), tree.clone()));
    Some(tree)
}

/// The message's row; its cards, labels and action bar come with 9.11, a rich answer with 9.10.
pub(super) fn element(
    key: &str,
    message: &Fields,
    files: &[(String, String)],
    widths: Widths,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let flag = |name| js::truthy(get(message, name));
    if flag("isSyntheticTrigger") || flag("systemCardType") {
        return None;
    }
    let content = get(message, "content")?.str()?;
    let shown = goal_verdict::strip(content);
    if rich_answer::has(shown) {
        return None;
    }
    let tree = tree(key, shown, cx)?;
    let dir = session(cx).and_then(|task| task.working_dir.clone());
    let dir = dir.unwrap_or_default();
    let anchors = target_link::read(key, &tree, files, &dir, window, cx);
    let mut body = markdown::element(key, &tree, &anchors, widths.message, window, cx);
    if !anchors.is_empty() {
        body = links::host(&format!("{key}-md"), anchors.shared(), body, cx);
    }
    // `flex w-full flex-col items-start gap-1.5` about the bare bubble, `w-full min-w-0`.
    let bubble = div().w_full().min_w_0().child(body);
    let column = div()
        .flex()
        .w_full()
        .flex_col()
        .items_start()
        .gap(css(6.0))
        .child(bubble);
    Some(column.into_any_element())
}
