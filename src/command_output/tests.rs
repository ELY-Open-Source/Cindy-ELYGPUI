use std::time::Instant;

use super::candidates;
use crate::rich_answer::js::{self, Js};

pub(crate) fn golden() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/history-artifacts/expected.json"
    ))
    .unwrap()
}

#[test]
fn candidates_match_cindys() {
    let golden = golden();
    let cases = golden.obj().unwrap()["extractCommandOutputPathCandidates"]
        .arr()
        .unwrap();
    assert!(cases.len() > 100);
    for case in cases {
        let command = case.arr().unwrap()[0].arr().unwrap()[0].str().unwrap();
        let got = Js::Arr(candidates(command).into_iter().map(Js::Str).collect());
        assert_eq!(
            js::stringify(&got, ""),
            js::stringify(&case.arr().unwrap()[1], ""),
            "{command:?}"
        );
    }
}

/// `longCommand` from `commandOutputPaths.scan.test.ts`.
fn long_command(kind: &str, count: usize) -> String {
    let paths: Vec<String> = (0..count)
        .map(|i| {
            format!(
                "'/Users/demo/project/data/dir{}/segment-{i}/file-{i}.json'",
                i % 97
            )
        })
        .collect();
    let pad = "x".repeat(120);
    match kind {
        "oneline" => format!(
            "node scripts/collect.js --pad {pad} --inputs {} && echo ok > /work/final-report.txt",
            paths
                .iter()
                .map(|p| format!("{p} --pad {pad}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        "multiline" => format!(
            "cat <<'EOF' | node scripts/collect.js\n{}\nEOF\necho ok > /work/final-report.txt",
            paths
                .iter()
                .map(|p| format!("{p} {pad}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        "powershell-nested-reads" => format!(
            "Set-Content -Value @({}) -Path C:\\work\\final-report.txt",
            paths
                .iter()
                .map(|p| format!("(Get-Content -Path {p} {pad})"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "powershell-per-line-writes" => paths
            .iter()
            .enumerate()
            .map(|(i, p)| format!("Out-File -FilePath 'C:\\work\\out-{i}.txt' {pad} {p}"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => format!(
            "Get-ChildItem {pad} {} | Out-File -FilePath C:\\work\\final-report.txt",
            paths
                .iter()
                .map(|p| format!("{p} {pad}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

#[test]
fn long_commands_stay_linear() {
    for kind in [
        "oneline",
        "multiline",
        "powershell",
        "powershell-nested-reads",
        "powershell-per-line-writes",
    ] {
        let command = long_command(kind, 7000);
        assert!(command.len() > 1_000_000);
        let started = Instant::now();
        let paths = candidates(&command);
        let elapsed = started.elapsed();
        let want: Vec<String> = match kind {
            "powershell-per-line-writes" => (0..7000)
                .map(|i| format!("C:\\work\\out-{i}.txt"))
                .collect(),
            k if k.starts_with("powershell") => vec!["C:\\work\\final-report.txt".into()],
            _ => vec!["/work/final-report.txt".into()],
        };
        assert_eq!(paths, want, "{kind}");
        assert!(elapsed.as_secs_f64() < 20.0, "{kind} took {elapsed:?}");
    }
}
