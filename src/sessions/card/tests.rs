use super::{body, clamp};
use crate::sessions::{group::Session, masonry};

fn task(pinned: bool, summary: Option<&str>, preview: Option<&str>) -> Session {
    Session {
        id: "s".into(),
        title: "t".into(),
        status: "active".into(),
        source: "desktop".into(),
        dialogue: false,
        working_dir: None,
        project_dir: None,
        remote_host: None,
        user_send_at: None,
        pinned_at: pinned.then_some(1),
        messages: 1,
        created_at: 0,
        agent_kind: "cc".into(),
        orca_lead: false,
        orca_worker: false,
        muted: false,
        tokens: 0,
        cost_usd: 0.0,
        cost_amount: 0.0,
        cost_currency: None,
        activity_at: 0,
        updated_at: 0,
        preview: preview.map(Into::into),
        summary: summary.map(Into::into),
    }
}

#[test]
fn the_body_follows_resolve_session_card_body() {
    let summary = Some("PR 已提交并开启，相关单测通过。");
    assert_eq!(body(&task(true, summary, Some("不是键盘没插上"))), summary);
    assert_eq!(
        body(&task(false, summary, Some("看一下我们现在的开发版"))),
        Some("看一下我们现在的开发版"),
        "not pinned"
    );
    assert_eq!(
        body(&task(true, Some("  "), Some("最近一条消息"))),
        Some("最近一条消息"),
        "a blank summary"
    );
    assert_eq!(body(&task(true, None, Some(" \u{a0} "))), None);
}

#[test]
fn the_clamp_follows_card_preview_line_clamp() {
    assert_eq!(
        clamp(&task(true, Some("  "), None)),
        3,
        "Boolean(summary), untrimmed"
    );
    assert_eq!(clamp(&task(true, None, None)), 2);
    let mut automation = task(true, None, None);
    automation.source = "scheduler".into();
    assert_eq!(clamp(&automation), 1);
    automation.source = "desktop".into();
    automation.title = "[Schedule] nightly".into();
    assert_eq!(clamp(&automation), 1);
}

#[test]
fn columns_follow_card_masonry() {
    assert_eq!(masonry::columns_for(245.9), 1);
    assert_eq!(masonry::columns_for(246.0), 2);
    assert_eq!(masonry::columns_for(389.9), 2);
    assert_eq!(masonry::columns_for(390.0), 3);
}
