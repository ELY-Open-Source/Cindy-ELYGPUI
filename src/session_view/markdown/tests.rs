use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext, px};

use super::text::{self, Piece};
use crate::session_view::user_row::tests::{at, open_roles};

/// The content area with a user message and then this assistant text, `msg-c1`.
fn draw<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    text: &str,
) -> &'a mut VisualTestContext {
    open_roles(cx, dir, 2400.0, &[("user", "hi"), ("assistant", text)])
}

fn root(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    at(cx, "msg-c1-markdown").unwrap()
}

fn block(cx: &mut VisualTestContext, path: &str) -> Bounds<Pixels> {
    let found = at(cx, &format!("msg-c1-md-{path}"));
    found.unwrap_or_else(|| panic!("no block {path}"))
}

/// Each block's left, top and height from the root's corner.
fn placed(cx: &mut VisualTestContext, paths: &[&str]) -> Vec<(f32, f32, f32)> {
    let root = root(cx);
    let place = |path: &&str| {
        let block = block(cx, path);
        let at = block.origin - root.origin;
        (at.x.into(), at.y.into(), block.size.height.into())
    };
    paths.iter().map(place).collect()
}

const BLOCKS: &str = "# H one\n\npara one\n\n## H two\n\n### H three\n\n#### H four\n\n\
- tight one\n- tight two\n  - nested\n- loose a\n\n  loose b\n- last\n\n\
1. one\n2. two\n\n> quote\n\n---\n\n```\ncode\nmore\n```\n\nend\n";

#[gpui::test]
fn blocks_stack_as_blink_stacks_them(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), BLOCKS);
    // `logs/review/9.6e-blink-blocks.log`, the list's items all loose here.
    let paths = [
        "0",
        "1",
        "2",
        "3",
        "4",
        "5-0-0",
        "5-1-0",
        "5-1-1-0-0t",
        "5-2-0",
        "5-2-1",
        "5-3-0",
        "6-0-0t",
        "6-1-0t",
        "7-0",
        "8",
        "9",
        "10",
    ];
    let expected = [
        (0.0, 12.0, 28.0),
        (0.0, 52.0, 24.0),
        (0.0, 88.0, 28.0),
        (0.0, 128.0, 24.0),
        (0.0, 160.0, 24.0),
        (24.0, 192.0, 24.0),
        (24.0, 220.0, 24.0),
        (48.0, 252.0, 24.0),
        (24.0, 284.0, 24.0),
        (24.0, 316.0, 24.0),
        (24.0, 344.0, 24.0),
        (24.0, 376.0, 24.0),
        (24.0, 404.0, 24.0),
        (18.0, 436.0, 24.0),
        (0.0, 476.0, 1.0),
        (0.0, 493.0, 100.0),
        (0.0, 605.0, 24.0),
    ];
    for ((path, found), expected) in paths.iter().zip(placed(cx, &paths)).zip(expected) {
        assert_eq!(found, expected, "{path}");
    }
    let root = root(cx);
    assert_eq!(
        root.size.height,
        px(629.0),
        "the last paragraph's margin is none"
    );
    assert_eq!(root.size.width, px(914.0), "w-full");
    let column = at(cx, "session-messages").unwrap();
    assert_eq!(root.left(), column.left(), "items-start");
    let user = at(cx, "item-msg-c0").unwrap();
    assert_eq!(root.top(), user.bottom() + px(14.0), "gap-3.5");
    // The quote's bar spans its paragraph; the code's surface fills the width.
    let quote = block(cx, "7");
    assert_eq!((quote.left(), quote.size.height), (root.left(), px(24.0)));
    assert_eq!(block(cx, "9").size.width, px(914.0));
    assert_eq!(block(cx, "5-0-0").size.width, px(890.0), "ml-6");
}

#[gpui::test]
fn a_marker_stands_before_its_items_first_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "- a\n- b\n  wrapped\n\n9. nine\n10. ten\n";
    let cx = draw(cx, dir.path(), text);
    let (first, second) = (block(cx, "0-0-0t"), block(cx, "0-1-0t"));
    let (disc, next) = (block(cx, "0-0-marker"), block(cx, "0-1-marker"));
    assert_eq!(disc.size.width, disc.size.height);
    assert!(disc.right() < first.left() && disc.left() > first.left() - px(24.0));
    assert!(disc.top() > first.top() && disc.bottom() < first.bottom());
    assert_eq!(next.origin - second.origin, disc.origin - first.origin);
    // A number ends where the text starts, and a longer one starts further left.
    let (nine, ten) = (block(cx, "1-0-marker"), block(cx, "1-1-marker"));
    let item = block(cx, "1-0-0t");
    assert_eq!(nine.right(), item.left());
    assert_eq!(ten.right(), item.left());
    assert!(ten.left() < nine.left());
    assert_eq!(nine.top(), item.top(), "the item's own line");
    assert_eq!(nine.size.height, px(24.0));
}

const ITEMS: &str = "intro\n\n-\n- ***\n- # Head\n- ```\n  code\n  ```\n- - inner\n- > quoted\n\
- ***\n\n  after rule\n";

#[gpui::test]
fn an_item_with_no_line_of_its_own_keeps_its_marker(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), ITEMS);
    // `logs/review/9.6e-blink-items.log`, its list 32 under the root's top here.
    let paths = [
        "1-0",
        "1-1",
        "1-1-0",
        "1-2-0",
        "1-3-0",
        "1-4-0-0-0t",
        "1-5-0",
        "1-6-0",
        "1-6-1",
    ];
    let expected = [
        (24.0, 32.0, 24.0),
        (24.0, 72.0, 24.0),
        (24.0, 72.0, 1.0),
        (24.0, 112.0, 28.0),
        (24.0, 152.0, 79.0),
        (48.0, 243.0, 24.0),
        (24.0, 275.0, 24.0),
        (24.0, 315.0, 1.0),
        (24.0, 332.0, 24.0),
    ];
    for ((path, found), expected) in paths.iter().zip(placed(cx, &paths)).zip(expected) {
        assert_eq!(found, expected, "{path}");
    }
    let height = root(cx).size.height;
    assert_eq!(height, px(364.0), "the list's my-2 stays inside");
    // Each disc lies the same way from its item's edge, on the line that took it.
    let edge = block(cx, "1-0").left();
    let own = block(cx, "1-0-marker");
    let drop = own.top() - block(cx, "1-0").top();
    for (marker, line) in [
        ("1-1", "1-1"),
        ("1-5", "1-5-0"),
        ("1-6", "1-6-1"),
        ("1-4", "1-4-0-0-0t"),
    ] {
        let disc = block(cx, &format!("{marker}-marker"));
        assert_eq!(disc.left(), own.left(), "{marker}");
        assert_eq!(disc.top() - block(cx, line).top(), drop, "{marker}");
    }
    let inner = block(cx, "1-4-0-0-marker");
    assert_eq!(inner.left(), own.left() + px(24.0));
    assert_eq!(
        inner.top(),
        block(cx, "1-4-marker").top(),
        "both on the inner line"
    );
    // In a heading and in code the disc follows that line's baseline.
    let head = block(cx, "1-2-marker");
    assert!(head.top() - block(cx, "1-2-0").top() > drop);
    let code = block(cx, "1-3-marker");
    assert_eq!(
        code.left(),
        own.left(),
        "from the item's edge, not the code's"
    );
    assert!(code.top() > block(cx, "1-3-0").top() + px(41.0));
    assert!(code.bottom() < block(cx, "1-3-0").top() + px(62.0));
    assert_eq!(edge, root(cx).left() + px(24.0));
}

#[gpui::test]
fn a_pending_marker_takes_an_empty_items_line_alone(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "- -\n\n  after empty inner\n- >\n\n  after empty quote\n- - ***\n  - inner text\n";
    let cx = draw(cx, dir.path(), text);
    // `logs/review/9.6e-blink-pending.log`.
    let paths = ["0-0-0-0", "0-0-1", "0-1-1", "0-2-0-0", "0-2-0-1-0t"];
    let expected = [
        (48.0, 8.0, 24.0),
        (24.0, 40.0, 24.0),
        (24.0, 72.0, 24.0),
        (48.0, 112.0, 24.0),
        (48.0, 152.0, 24.0),
    ];
    for ((path, found), expected) in paths.iter().zip(placed(cx, &paths)).zip(expected) {
        assert_eq!(found, expected, "{path}");
    }
    let from = |cx: &mut VisualTestContext, marker: &str, line: &str| {
        block(cx, &format!("{marker}-marker")).origin - block(cx, line).origin
    };
    // An empty item is a line: the outer disc stands on it, 24px before the inner one.
    let inner = from(cx, "0-0-0-0", "0-0-0-0");
    assert_eq!(
        from(cx, "0-0", "0-0-0-0"),
        inner - gpui::point(px(24.0), px(0.0))
    );
    // An empty quote holds no line, and its inset is given back.
    assert_eq!(from(cx, "0-1", "0-1-1"), inner);
    // An item that holds a rule alone is no line: the outer disc waits for the next one.
    assert_eq!(from(cx, "0-2-0-0", "0-2-0-0"), inner);
    assert_eq!(from(cx, "0-2-0-1", "0-2-0-1-0t"), inner);
    assert_eq!(
        from(cx, "0-2", "0-2-0-1-0t"),
        inner - gpui::point(px(24.0), px(0.0))
    );
}

const TASKS: &str = "before\n\n- [ ] todo\n- [x] done\n  - nested plain\n- [ ] third\n\nafter\n\n\
a[^1]\n\n>\n\nz\n\n[^1]: note\n";

#[gpui::test]
fn a_task_list_loses_cindys_list_classes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), TASKS);
    // `logs/review/9.6e-blink-items.log`: a task list has no indent, gap or marker.
    let paths = [
        "0",
        "1-0-0t",
        "1-1-0t",
        "1-1-0-0-0t",
        "1-2-0t",
        "2",
        "3",
        "5",
        "6-1-0-0",
    ];
    let expected = [
        (0.0, 0.0, 24.0),
        (0.0, 32.0, 24.0),
        (0.0, 56.0, 24.0),
        (24.0, 88.0, 24.0),
        (0.0, 120.0, 24.0),
        (0.0, 152.0, 24.0),
        (0.0, 184.0, 24.0),
        (0.0, 216.0, 24.0),
        (24.0, 248.0, 24.0),
    ];
    for ((path, found), expected) in paths.iter().zip(placed(cx, &paths)).zip(expected) {
        assert_eq!(found, expected, "{path}");
    }
    for absent in ["1-0-marker", "1-1-marker", "4", "6-0"] {
        assert!(at(cx, &format!("msg-c1-md-{absent}")).is_none(), "{absent}");
    }
    assert!(at(cx, "msg-c1-md-1-1-0-0-marker").is_some());
    assert!(at(cx, "msg-c1-md-6-1-0-marker").is_some());
    let (line, check) = (block(cx, "1-0-0t"), block(cx, "1-0-0t-check-0"));
    assert_eq!(check.size, gpui::size(px(13.0), px(13.0)));
    assert_eq!(check.left(), line.left());
    assert!(check.top() > line.top() && check.bottom() < line.bottom());
    let done = block(cx, "1-1-0t-check-0");
    assert_eq!(
        done.origin - block(cx, "1-1-0t").origin,
        check.origin - line.origin
    );
}

#[gpui::test]
fn an_object_character_in_the_text_is_no_check(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // `logs/review/9.6e-r1-review-blink.log`: each is one line, and only an `input` is a box.
    let text = "a\u{fffc}b\n\na&#xfffc;b\n\n- [ ] a\u{fffc}b\n- [x] \u{fffc}\n";
    let cx = draw(cx, dir.path(), text);
    let found = placed(cx, &["0", "1", "2-0-0t", "2-1-0t"]);
    assert_eq!(
        found,
        [
            (0.0, 0.0, 24.0),
            (0.0, 32.0, 24.0),
            (0.0, 64.0, 24.0),
            (0.0, 88.0, 24.0)
        ]
    );
    for (line, checks) in [("0", 0), ("1", 0), ("2-0-0t", 1), ("2-1-0t", 1)] {
        for n in 0..2 {
            let check = at(cx, &format!("msg-c1-md-{line}-check-{n}"));
            assert_eq!(check.is_some(), n < checks, "{line} {n}");
        }
    }
    let (line, check) = (block(cx, "2-1-0t"), block(cx, "2-1-0t-check-0"));
    assert_eq!(check.left(), line.left(), "the box, then the character");
}

#[gpui::test]
fn code_wraps_anywhere_and_keeps_its_empty_lines(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "x".repeat(300);
    let text =
        format!("```\n{long}\n```\n\n```rust\na\n\n\n\tb\n```\n\n```\n```\n\n    indented\n");
    let cx = draw(cx, dir.path(), &text);
    let (space, rows) = cx.update(|window, cx| {
        use ely_gpui_component::theme::ActiveTheme as _;
        let mut style = window.text_style();
        style.font_family = cx.theme().mono_family.clone();
        (style.font_size, style.line_height) = (px(14.0).into(), px(21.0).into());
        assert_eq!(text::metrics(&style, window).line, 21.0);
        let space = f32::from(crate::session_view::flow::draw::shape(" ", &style, window).width);
        // A tab counts from its own row's start: after `gh`, not after the whole line.
        let rows = text::code("abcdefgh\tz", &style, px(space * 6.0 + 1.0), window);
        (space, rows)
    });
    let [first, second] = &rows[..] else {
        panic!("two rows: {rows:?}")
    };
    assert_eq!(first, &[Piece::Text("abcdef".into())]);
    let [Piece::Text(before), Piece::Tab(tab), Piece::Text(after)] = &second[..] else {
        panic!("a tab between two texts: {second:?}")
    };
    assert_eq!((before.as_str(), after.as_str()), ("gh", "z"));
    assert!((tab - space * 2.0).abs() < 0.001, "{tab}");
    let per_row = (880.0 / space).floor();
    let lines = (300.0 / per_row).ceil();
    let found = placed(cx, &["0", "1", "2", "3"]);
    assert_eq!(found[0], (0.0, 12.0, 58.0 + 21.0 * lines));
    assert_eq!(
        found[1].2,
        58.0 + 21.0 * 4.0,
        "two empty lines and a tabbed one"
    );
    assert_eq!(found[1].1, found[0].1 + found[0].2 + 12.0);
    assert_eq!(found[2].2, 58.0, "an empty block holds no line");
    assert_eq!(found[3].2, 58.0 + 21.0, "indented code is a block too");
    let end = px(found[3].1 + found[3].2 + 12.0);
    assert_eq!(root(cx).size.height, end, "my-3 stays inside");
}

#[gpui::test]
fn the_row_draws_the_shown_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let verdict = "done\n\n```json\n{\"goal_status\":\"complete\",\"reason\":\"ok\"}\n```";
    let op = r#"{"version":1,"seq":1,"op":"create","id":"a","kind":"text"}"#;
    let rich = &format!("```cindy-ui\n{op}\n```");
    let turns = [verdict, rich, "tail\n"];
    let rows: Vec<_> = turns
        .iter()
        .flat_map(|text| [("user", "hi"), ("assistant", *text)])
        .collect();
    let cx = open_roles(cx, dir.path(), 2400.0, &rows);
    assert_eq!(
        root(cx).size.height,
        px(24.0),
        "the verdict block is not shown"
    );
    assert!(at(cx, "msg-c1-md-1").is_none());
    assert!(at(cx, "item-msg-c2").is_some());
    assert!(
        at(cx, "item-msg-c3").is_none(),
        "a rich answer comes with 9.10"
    );
    assert!(at(cx, "msg-c5-md-0").is_some());
    cx.update(|_, cx| {
        let first = crate::session_view::assistant_row::tree("k", "a", cx).unwrap();
        let again = crate::session_view::assistant_row::tree("k", "a", cx).unwrap();
        assert!(std::rc::Rc::ptr_eq(&first, &again), "parsed once");
        let changed = crate::session_view::assistant_row::tree("k", "b", cx).unwrap();
        assert!(!std::rc::Rc::ptr_eq(&first, &changed));
    });
}
