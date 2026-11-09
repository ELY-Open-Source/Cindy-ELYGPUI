use serde_json::Value;

use super::display;
use crate::{i18n::Translator, sessions::group::Session};

/// The fixture's `createdAt`, 2026-03-04T05:06:07Z.
const CREATED_MS: i64 = 1_772_600_767_000;

fn session(title: &str, source: &str, working_dir: Option<&str>) -> Session {
    Session {
        id: "s".into(),
        title: title.into(),
        status: "active".into(),
        source: source.into(),
        dialogue: false,
        working_dir: working_dir.map(Into::into),
        project_dir: None,
        remote_host: None,
        user_send_at: None,
        pinned_at: None,
        messages: 1,
        created_at: CREATED_MS,
        agent_kind: "cc".into(),
        orca_lead: false,
        orca_worker: false,
        muted: false,
        tokens: 0,
        cost_usd: 0.0,
        cost_amount: 0.0,
        cost_currency: None,
        activity_at: CREATED_MS,
        updated_at: CREATED_MS,
        preview: None,
        summary: None,
    }
}

#[test]
fn shows_titles_as_get_session_display_title_does() {
    let path = format!(
        "{}/tests/fixtures/display-title.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let golden: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let t = Translator::new("en").unwrap();
    // The fixture's times are UTC; this machine shows its own zone.
    let utc = "03-04 05:06";
    let local = jiff::Timestamp::from_millisecond(CREATED_MS)
        .unwrap()
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%m-%d %H:%M")
        .to_string();
    assert!(golden.len() > 200);
    for case in golden {
        let (title, source) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
        let dir = case[2].as_str();
        let want = case[3].as_str().unwrap().replace(utc, &local);
        assert_eq!(
            display(&session(title, source, dir), &t),
            want,
            "{title:?} {source} {dir:?}"
        );
    }
}

#[test]
fn stores_edited_titles_as_to_stored_session_title_does() {
    let path = format!(
        "{}/tests/fixtures/stored-title.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let golden: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(golden.len() > 200);
    for case in golden {
        let (title, source) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
        let (dir, edited) = (case[2].as_str(), case[3].as_str().unwrap());
        assert_eq!(
            super::stored(&session(title, source, dir), edited),
            case[4].as_str().unwrap(),
            "{title:?} {source} {dir:?} {edited:?}"
        );
    }
}
