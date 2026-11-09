# Reviews

Each round: task, round, verdict, findings and their fixes. Reviewer: an Orca worker, Claude Code
on Kimi K3.1 Pro (effort high) from 8.7c1a; Codex GPT-6.1 Sol (xhigh) for 8.6 to 8.7b2; Kimi before. Each round's verdict and
gate logs sit in `logs/review/<task>-r<N>*`.

Chapters 1 to 7 sit in `tasks/reviews-done.md`, chapter 8 in `tasks/reviews-done-8.md`.

## 9.0 Chat view plan
- Round 1: FAIL. 9.18 drew the permission prompt from database rows, which do not exist; 9.15's
  selection quote needed 9.16's composer draft.
- Fixed: 9.18 splits history cards (`ask_user`, `plan_review` rows) from pending interactions,
  which live in the runtime and come with 10.0 (panels drawn against fixtures); the composer is
  9.15 and message actions 9.16, the quote landing in 9.15's draft.
- Round 2: PASS.

## 9.1a History row checks
- Round 1: FAIL. The module docs of `message_rows.rs` and `payload/files.rs` ran to two lines.
- Fixed: each is one line; no other new comment spans two.
- Round 2: PASS.

## 9.1b History view projection
- Round 1: FAIL. The History view bullet pushed `AGENTS.md` to 501 lines.
- Fixed: the bullet is two lines shorter; `AGENTS.md` is 499 lines.
- Round 2: PASS.

## 9.1c History rows read
- Round 1: FAIL. `js_date::iso` negated `i64::MIN`; `TASKS.md` reached 503 lines; `rows.rs`'s module doc ran to two lines.
- Fixed: a range check without negation, with tests at both bounds and `i64::MIN`/`MAX`; the split entries are shorter (499 lines); the doc is one line.
- Round 2: PASS.

## 9.1d History artifacts
- Round 1: PASS. Note: a token can start on a separator (`Out-File -FilePath ;out/`), so `tail.terminator < to` is not an equivalent mutant; the code matches Cindy.

## 9.1e History reader
- Round 1: PASS. The in-flight share is left out: reads run one job at a time on the database worker.
