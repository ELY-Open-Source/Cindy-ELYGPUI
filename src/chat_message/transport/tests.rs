use gpui::TestAppContext;
use rusqlite::Connection;

use super::*;
use crate::{
    history_view::{Summary, controller::Transport},
    rich_answer::js::Obj,
};

fn put(db: &Connection, n: i64, role: &str, content: &str, tool: Option<&str>) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, tool_use_id, created_at)
         VALUES (?, ?, 's1', ?, ?, ?, ?)",
        (
            n.to_string(),
            format!("c{n}"),
            role,
            content,
            tool,
            1_700_000_000_000 + n,
        ),
    )
    .unwrap();
}

/// A turn: the prompt, one Bash call and its result, the answer.
fn turn() -> Connection {
    let db = crate::history_view::rows::tests::db();
    put(&db, 0, "user", "\"Run it ✓\"", None);
    let call = r#"{"toolName":"Bash","toolUseId":"t1","input":{"command":"ls"}}"#;
    put(&db, 1, "tool_use", call, Some("t1"));
    put(&db, 2, "tool_result", "\"ok\"", Some("t1"));
    put(&db, 3, "assistant", "\"Done\"", None);
    db
}

fn serve(db: Connection, cx: &mut TestAppContext) -> Local {
    cx.update(|cx| {
        crate::localdb::install(None, cx);
        cx.set_global(Translator::new("en").unwrap());
        crate::localdb::serve(db, cx);
    });
    Local::new("s1".into(), cx.to_async())
}

fn keys(m: &Message) -> Vec<&str> {
    m.keys().map(String::as_str).collect()
}

fn work(items: &[Item<Message>]) -> &Summary {
    items
        .iter()
        .find_map(|item| match item {
            Item::Work { summary, .. } => Some(summary),
            _ => None,
        })
        .expect("a work group")
}

/// The reader's epoch, before the database's `open:version:changes`.
fn epoch(summary: &Summary) -> &str {
    let epoch = summary.revision.rsplit(':').nth(3).unwrap();
    assert_eq!(epoch.len(), 36, "{}", summary.revision);
    epoch
}

#[gpui::test]
async fn a_page_and_its_details_come_back_as_store_rows(cx: &mut TestAppContext) {
    let local = serve(turn(), cx);
    let page = local.page(None).await.unwrap();
    let kinds: Vec<_> = page.items.iter().map(Item::key).collect();
    assert_eq!(kinds.len(), 3, "{kinds:?}");
    let Item::Messages { messages, .. } = &page.items[0] else {
        panic!("the prompt first");
    };
    let prompt = &messages[0];
    assert_eq!(controller::Message::id(prompt), "0");
    assert_eq!(controller::Message::client_id(prompt), "c0");
    assert_eq!(
        controller::Message::created_at(prompt),
        "2023-11-14T22:13:20.000Z"
    );
    assert_eq!(keys(prompt).last(), Some(&"rowid"));
    let strings = ["c0", "user", "Run it ✓", "0", "2023-11-14T22:13:20.000Z"];
    let units = strings.concat().encode_utf16().count();
    assert_eq!(controller::Message::characters(prompt), units);
    let details = local.details(work(&page.items).into(), None).await.unwrap();
    let roles: Vec<_> = details.messages.iter().map(|m| text(m, "role")).collect();
    assert_eq!(roles, ["tool_use", "tool_result"]);
    assert_eq!(text(&details.messages[0], "toolName"), "Bash");
    let mut raw = Obj::new();
    raw.insert("clientId".into(), Js::Str("sub".into()));
    raw.insert("role".into(), Js::Str("assistant".into()));
    raw.insert("content".into(), Js::Str("nested".into()));
    let child = Item::Messages {
        key: "m".into(),
        messages: vec![Js::Obj(raw)],
        deferred: None,
    };
    let nested = Item::Work {
        key: "w".into(),
        summary: work(&page.items).clone(),
        children: Some(vec![child]),
    };
    let Item::Work { children, .. } = &items(vec![nested]).unwrap()[0] else {
        panic!("the group stays a group");
    };
    let Some([Item::Messages { messages, .. }]) = children.as_deref() else {
        panic!("its children are mapped");
    };
    assert_eq!(controller::Message::id(&messages[0]), "sub");
}

#[gpui::test]
async fn one_reader_serves_each_open_database(cx: &mut TestAppContext) {
    let local = serve(turn(), cx);
    let first = local.page(None).await.unwrap();
    let again = local.page(None).await.unwrap();
    assert_eq!(epoch(work(&first.items)), epoch(work(&again.items)));
    cx.update(|cx| crate::localdb::serve(turn(), cx));
    let reopened = local.page(None).await.unwrap();
    assert_ne!(epoch(work(&first.items)), epoch(work(&reopened.items)));
}

#[gpui::test]
async fn a_throwing_map_fails_the_read(cx: &mut TestAppContext) {
    let db = crate::history_view::rows::tests::db();
    put(&db, 0, "user", "\"Run it\"", None);
    let call = r#"{"toolName":"constructor","toolUseId":"t1","input":{"a":1}}"#;
    put(&db, 1, "tool_use", call, Some("t1"));
    put(&db, 2, "tool_result", "\"ok\"", Some("t1"));
    put(&db, 3, "assistant", "\"Done\"", None);
    let local = serve(db, cx);
    let page = local.page(None).await.unwrap();
    let r = work(&page.items).into();
    let error = local.details(r, None).await.map(|_| ()).unwrap_err();
    assert_eq!(error, "paramKeys is not iterable");
}

#[gpui::test]
async fn a_closed_database_fails_the_read(cx: &mut TestAppContext) {
    let local = serve(turn(), cx);
    cx.update(|cx| crate::localdb::install(None, cx));
    let error = local.page(None).await.map(|_| ()).unwrap_err();
    assert_eq!(error, "DbClient not ready");
}

#[test]
fn rows_without_an_id_or_time_take_their_client_id_and_an_empty_time() {
    let mut row = Obj::new();
    row.insert("clientId".into(), Js::Str("c1".into()));
    row.insert("role".into(), Js::Str("assistant".into()));
    row.insert("content".into(), Js::Str("hi".into()));
    let mapped = map_rows(&[Js::Obj(row)]).unwrap();
    assert_eq!(
        keys(&mapped[0]),
        [
            "clientId",
            "role",
            "content",
            "sourceGroup",
            "botLearning",
            "isStreaming",
            "id",
            "createdAt"
        ]
    );
    assert_eq!(controller::Message::id(&mapped[0]), "c1");
    let created = mapped[0]["createdAt"].as_deref();
    assert_eq!(created, Some(&Js::Str(String::new())));
}

#[test]
fn characters_count_every_string_inside() {
    let mut fields = Fields::new();
    let list = Js::Arr(vec![Js::Str("é✓".into()), Js::Num(1.0)]);
    fields.insert("list".into(), Some(Rc::new(list)));
    fields.insert("gone".into(), None);
    assert_eq!(controller::Message::characters(&Rc::new(fields)), 2);
}

/// The reviewer's case: a new answer's `undefined` keys clear the stale ones on merge.
#[test]
fn undefined_keys_survive_into_a_merge() {
    let input = js::parse(include_str!(
        "../../../tests/fixtures/chat-message/undefined-keys.json"
    ))
    .unwrap();
    let input = input.obj().unwrap();
    let server = map_rows(input["rows"].arr().unwrap()).unwrap();
    let ask = &server[0];
    let undefined: Vec<&str> = ask
        .iter()
        .filter(|(_, v)| v.is_none())
        .map(|(k, _)| k.as_str())
        .collect();
    let want = [
        "askUserRequestId",
        "askUserQuestions",
        "askUserAnswers",
        "askUserOptions",
        "askUserPageIndicator",
    ];
    assert_eq!(undefined, want);
    let existing = input["existing"].obj().unwrap();
    let existing: Fields = existing
        .iter()
        .map(|(k, v)| (k.clone(), Some(Rc::new(v.clone()))))
        .collect();
    let existing = Rc::new(vec![Rc::new(existing)]);
    let options = super::super::merge::Options::default();
    let order = super::super::merge::RowsOrder::OldestFirst;
    let merged = super::super::merge::merge(&server, &existing, &options, order);
    for key in want {
        assert_eq!(merged[0].get(key), Some(&None), "{key}");
    }
    assert_eq!(text(&merged[0], "content"), "New Q");
}
