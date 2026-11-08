use serde_json::{Value, json};

use super::{Entry, Order, Recent, Session, aliases};
use crate::sessions::{
    lists::{Pinned, build},
    query,
    settings::Settings,
};

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` to ms, as the fixtures write it.
fn ms(v: &Value) -> Option<i64> {
    let s = v.as_str()?;
    let n = |r: std::ops::Range<usize>| s[r].parse::<i64>().unwrap();
    let (y, m, d) = (n(0..4), n(5..7), n(8..10));
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(((days * 24 + n(11..13)) * 60 + n(14..16)) * 60_000 + n(17..19) * 1000 + n(20..23))
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn session(v: &Value) -> Session {
    Session {
        id: text(&v["id"]).unwrap(),
        title: text(&v["title"]).unwrap(),
        status: text(&v["status"]).unwrap(),
        source: text(&v["source"]).unwrap(),
        dialogue: v["workspaceKind"] == "dialogue",
        working_dir: text(&v["workingDir"]),
        project_dir: text(&v["projectDir"]),
        remote_host: text(&v["remoteHostId"]),
        user_send_at: ms(&v["userSendAt"]),
        pinned_at: ms(&v["pinnedAt"]),
        messages: v["_count"]["messages"].as_i64().unwrap(),
        created_at: ms(&v["createdAt"]).unwrap_or(0),
        agent_kind: text(&v["agentKind"]).unwrap_or_else(|| "cc".into()),
        activity_at: ms(&v["lastActivityAt"])
            .or(ms(&v["userSendAt"]))
            .or(ms(&v["updatedAt"]))
            .unwrap_or(0),
    }
}

fn shown(entry: &Entry) -> Value {
    let ids = |list: &[Session]| list.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    match entry {
        Entry::Project(p) => {
            json!({ "kind": "project", "key": p.key, "name": p.name, "sessions": ids(&p.sessions) })
        }
        Entry::Dialogues(list) => json!({ "kind": "dialogue-group", "sessions": ids(list) }),
        Entry::Loose(s) => json!({ "kind": "session", "session": s.id }),
    }
}

#[test]
fn matches_cindys_grouping_and_main_list() {
    let read = |name: &str| -> Vec<Value> {
        let path = format!(
            "{}/tests/fixtures/grouping/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    };
    for (case, want) in read("input.json").iter().zip(read("expected.json")) {
        let sessions: Vec<Session> = case["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(session)
            .collect();
        let platform = case["platform"].as_str().unwrap_or("linux");
        // What `query::recent` reads of the sessions table, joined to each entry's own kinds.
        let tasks: Vec<(Option<String>, String)> = sessions
            .iter()
            .filter(|s| !s.dialogue && s.remote_host.is_none() && s.working_dir.is_some())
            .map(|s| {
                (
                    s.project_dir.clone().or(s.working_dir.clone()),
                    s.agent_kind.clone(),
                )
            })
            .collect();
        let rows = case["recentWorkdirs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let paths = rows.iter().filter_map(|r| text(&r["path"])).collect();
        let recent: Vec<Recent> = query::with_kinds(paths, &tasks, platform)
            .into_iter()
            .zip(&rows)
            .map(|(mut r, row)| {
                let own = row["knownAgentKinds"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                r.kinds.extend(own.iter().filter_map(text));
                r.kinds.sort();
                r.kinds.dedup();
                r
            })
            .collect();
        let rows: Vec<(String, String)> = case["aliases"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), text(v).unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        let manual: Vec<String> = case["manualProjectOrder"]
            .as_array()
            .map(|a| a.iter().filter_map(text).collect())
            .unwrap_or_default();
        let list = |key: &str| -> Vec<String> {
            case[key]
                .as_array()
                .map(|a| a.iter().filter_map(text).collect())
                .unwrap_or_default()
        };
        let settings = Settings {
            pinned_order: list("pinnedOrder"),
            hidden: list("hiddenProjectKeys"),
        };
        let order = Order {
            flat: case["groupBy"] == "flat",
            created: case["sortBy"] == "created",
            custom: case["projectOrder"] == "custom",
            manual: &manual,
            group_dialogue: case["groupDialogue"] != false,
            vendor: case["vendor"].as_str(),
            cutoff: ms(&case["lastActivityCutoff"]),
        };
        let lists = build(
            &sessions,
            &recent,
            &aliases(&rows, platform),
            &settings,
            platform,
            &order,
            "Chat",
        );
        let got: Vec<Value> = lists.main.iter().map(shown).collect();
        let pinned: Vec<Value> = lists
            .pinned
            .iter()
            .map(|p| match p {
                Pinned::Session(s) => {
                    json!({ "kind": "session", "session": s.id, "label": lists.labels.get(&s.id) })
                }
                Pinned::Project(p) => json!({ "kind": "project", "key": p.key, "name": p.name,
                    "sessions": p.sessions.iter().map(|s| &s.id).collect::<Vec<_>>() }),
            })
            .collect();
        assert_eq!(
            Value::Array(pinned),
            want["pinned"],
            "pinned: {}",
            case["name"]
        );
        assert_eq!(Value::Array(got), want["entries"], "{}", case["name"]);
    }
}
