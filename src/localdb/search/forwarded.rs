//! `forwardedRequestDisplayText`: a forwarded request's quoted words shown as plain text.

use crate::rich_answer::js::{self, Js};

/// Source, user, agent, task and task id prefixes, in every spelling Cindy has stored.
const DIALECTS: [[&str; 5]; 11] = [
    [
        "[Source] ",
        "User’s original words:",
        "Agent supplement:",
        "[Task source] ",
        "Task ID:",
    ],
    [
        "[来源] ",
        "用户原话：",
        "Agent补充：",
        "[任务来源] ",
        "任务 ID：",
    ],
    [
        "[来源] ",
        "用户原话：",
        "agent补充：",
        "[任务来源] ",
        "任务 ID：",
    ],
    [
        "[來源] ",
        "使用者原話：",
        "Agent補充：",
        "[任務來源] ",
        "任務 ID：",
    ],
    [
        "[出典] ",
        "ユーザーの原文：",
        "Agent の補足：",
        "[タスクの出典] ",
        "タスク ID：",
    ],
    [
        "[출처] ",
        "사용자 원문:",
        "Agent 보충:",
        "[작업 출처] ",
        "작업 ID:",
    ],
    [
        "[Fuente] ",
        "Palabras originales del usuario:",
        "Suplemento del Agent:",
        "[Origen de la tarea] ",
        "ID de la tarea:",
    ],
    [
        "[Source] ",
        "Mots d'origine de l'utilisateur :",
        "Complément de l'Agent :",
        "[Source de la tâche] ",
        "ID de la tâche :",
    ],
    [
        "[Quelle] ",
        "Ursprünglicher Wortlaut des Nutzers:",
        "Agent-Ergänzung:",
        "[Aufgabenquelle] ",
        "Aufgaben-ID:",
    ],
    [
        "[Origem] ",
        "Palavras originais do utilizador:",
        "Suplemento do Agent:",
        "[Origem da tarefa] ",
        "ID da tarefa:",
    ],
    [
        "[Origem] ",
        "Palavras originais do usuário:",
        "Complemento do Agent:",
        "[Origem da tarefa] ",
        "ID da tarefa:",
    ],
];

/// A JSON string that `JSON.stringify` writes back byte for byte.
fn exact_string(quoted: &str) -> Option<String> {
    match js::parse(quoted) {
        Ok(Js::Str(ref s)) if js::quote(s) == quoted => Some(s.clone()),
        _ => None,
    }
}

/// One whole envelope decodes; anything else stays verbatim.
pub fn display_text(body: &str) -> String {
    if !body.starts_with('[') {
        return body.to_owned();
    }
    let lines: Vec<&str> = body.split('\n').collect();
    for [source, user, agent, task, task_id] in DIALECTS {
        // Background tasks put two host lines first, each followed by a blank line.
        let offset = if lines.len() == 7
            && lines[0].starts_with(task)
            && lines[1].is_empty()
            && lines[2].starts_with(&format!("{task_id} "))
            && lines[3].is_empty()
        {
            4
        } else {
            0
        };
        if lines.len() != offset + 3
            || !lines[offset].starts_with(source)
            || !lines[offset + 1].starts_with(user)
            || !lines[offset + 2].starts_with(agent)
        {
            continue;
        }
        let (Some(said), Some(added)) = (
            exact_string(&lines[offset + 1][user.len()..]),
            exact_string(&lines[offset + 2][agent.len()..]),
        ) else {
            continue;
        };
        let mut out: Vec<&str> = lines[..=offset].to_vec();
        out.extend(["", user, "", &said, "", agent, "", &added]);
        return out.join("\n");
    }
    body.to_owned()
}
