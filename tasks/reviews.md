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

## 9.2a History view controller
- Round 1: FAIL. `characters` recursed into deep tool input and overflowed the stack; the reread on a new revision kept `allow_collapsed`, where Cindy's `loadDetails(latest)` checks expansion.
- Fixed: an explicit stack, as `detailCharacters`; the reread passes `false`. Tests: 50,000 nested arrays, and a recovery read over a new revision.
- Round 2: PASS.

## 9.2b mapServerMessages
- Round 1: FAIL. Rust's `sort_by` met Cindy's non-total timeline order (wrong order, a panic at 24 rows); `finishedAt` read ISO only; Cindy Make card arrays spread into objects; prototype tool names did not throw; `String()` never threw.
- Fixed: `js_sort` ports V8's TimSort (insertion under 8); `js_date::parse` ports V8's `DateParser` with local times through jiff; card `data` kept as is; `paramKeys is not iterable`; `js::try_string` at the four coercions, billing sources read before the early return. Goldens `js-sort` (V8) and `js-date` (Cindy's Electron, three zones); the reviewer's inputs join `chat-message`.
- Round 2: PASS.

## 9.2c mergeMessages
- Round 1: FAIL. `valueReadableSize` counts 0 when `JSON.stringify` throws: V8's slow serializer (a `toJSON` key, index keys, cons strings) overflows past about 5,000 levels; the golden script opened with two comment lines.
- Answered: the overflow depth moves with the caller's stack (5655, 5613, 5246 array levels under 0, 100, 1000 frames; `logs/review/9.2c-r2-depth-bisect.log`), and cons strings are invisible to the port, so the throw is not ported. The `String::kMaxLength` throw needs 512 MiB. Comment joined.
- Round 2: FAIL. The stack bound is accepted as a runtime limit; `String::kMaxLength` is fixed: 90M NULs encode to 540,000,002 units and V8 throws.
- Fixed: a field whose encoding passes `(1 << 29) - 24` counts 0; test on the reviewer's input.
- Round 3: PASS.

## 9.2d History view local transport
- Round 1: FAIL. `mapServerMessages` dropped the keys Cindy's literals set to `undefined` (ask, plan review, `toolUseId`, `sharedAuthor*`, `sourceGroup`, `botLearning`, `clientId`), so a merge kept stale ask fields.
- Fixed: the mapper builds `Out`, whose `None` is such a key, in Cindy's place; store rows carry it. Golden `chat-message` records each row's `undefined` keys (a row without a client id added); the reviewer's ask merge is a test.
- Round 2: PASS.

## 9.2e The store's slice for a local task
- Round 1: FAIL. The forced read took its row baseline at first poll, not at call; a two-line doc comment.
- Fixed: `reconcile` captures the baseline, epoch and page read before it spawns; the comment is one line; a regression test changes a row before the queue runs.
- Round 2: PASS.

## 9.2f The raw reads beside the view
- Round 1: FAIL. The raw older pages took the lock at call time but read only while the caller polled, so a dropped waiter left the task busy for good.
- Fixed: the raw read runs on the foreground executor as a shared future; the caller's copy only waits. A test drops the waiter and checks the page lands and the lock lifts.
- Round 2: PASS.

## 9.2g Memory
- Round 1: FAIL. `set` re-inserted a slice the full cache had just pushed out, even when its update was a no-op, so the cache held 21 slices and the access order 20.
- Fixed: `set` reads through `get_or_create` and writes only a real update, as `setState` returns before `sessions.set`; the reviewer's full-cache case is a test.
- Round 2: PASS.

## 9.3a The route's task
- Round 1: FAIL. One read served both the gate and the content, so a deleted task's first read purged it before the visit loaded it again, a content row kept the gate open after the task left list and database, and an error gate left the visit open; two module comments ran two lines.
- Fixed: the gate reads only while the list does not know the task and opens on its own result; the content mounts and unmounts with the gate (its read, then the visit; unmount leaves the store); the deleted exit runs when the content's read lands; comments are one line; the reviewer's three cases are tests.
- Round 2: FAIL. The listed source arriving did not cancel the gate's read, so its late answer left a `loaded` that later hid a missing task; content reads matched only the id, so a read from an earlier mount of the same task purged its remount.
- Fixed: a change of the listed source cancels the gate's read in flight, as the effect's cleanup does; each content read takes a sequence the mount holds, as `createSessionRefreshSequence` does; both cases are tests.
- Round 3: FAIL. Each new task reset the gate's `loaded`, while Cindy's gate keeps its last successful `{id, source}` across task switches; returning to a task the gate read before closes the gate where Cindy keeps it open.
- Left open: marked `- [!]` in TASKS.md.

## 9.3b The content header
- Round 1: FAIL. The title's press was followed only over the title, so a drag that left it moved nothing and a release outside left a press a later hover turned into a move; More was no Tab stop; a route change hid the header's field, which Cindy keeps with its draft; a test comment ran two lines.
- Fixed: the press is followed over the whole window in the capture phase until it moves the window, lifts, or a move comes with no button held; More holds a Tab stop and a press focuses it; the header draws its field whatever task opened it, commits to the task shown now (its stored or shown title is no change) and drops the edit when the header goes; the comment is one line; the reviewer's four cases are tests.
- Round 2: FAIL. Both rename entries looked the task up in any held bucket first, so a stale archived row prefilled the field over the fresh title the header showed.
- Fixed: the double click and More's Rename start from the task the header shows (`header::task`); rows look theirs up in the list; `session_view::task` is gone; the reviewer's two cases are tests.
- Round 3: PASS.

## 9.3c The column
- Round 1: PASS.

## 9.4a The plan timeline
- Round 1: FAIL. A todo's content went through a `String()` that cannot fail, so an object with an own non-callable `toString` became `[object Object]` where Cindy throws a TypeError.
- Fixed: the conversion is `js::try_string`; `extract_plan_todos`, `insertions` and `latest_state` return the throw as `Err`; the golden records a throw's message, with cases for `TodoWrite`, `update_plan` items and plan, arrays, and a subagent row that never converts.
- Round 2: PASS.

## 9.4b What a turn's rows yield
- Round 1: FAIL. `mcpSourceCandidates` read the input in JSON text order where `Object.values` puts integer keys first, ascending; the rich answer's scheme allowlist folded case by Unicode, so `HTTPſ://` passed as `http` where Cindy's `/i` without `u` keeps the address as written.
- Fixed: the candidates come through `js::entries`; the allowlist matches with `(?i-u)`. The golden takes the tool input as JSON text, so its key order reaches both sides, and holds `ſ`, the Kelvin sign and a ligature in schemes.
- Round 2: PASS.
## 9.4c `buildRenderItems`
- Round 1: FAIL. A history artifact with no `ready` came out unready: the file's `ready` took the truthy read where Cindy copies the field and treats only `false` as unready.
- Fixed: the file is unready only for `ready: false`; the exclude checks keep the truthy read. The golden holds an artifact with `ready` absent, `true`, `false`, `0`, `null` and `''`.
- Round 2: PASS.
## 9.4d Work groups over the render items
- Round 1: FAIL. A sealed answer with no `clientId` entered the final-assistant set as the string
  `undefined`, where Cindy's set holds `undefined`.
- Fixed: the set holds `Option<String>`, `None` for a missing id; golden cases for a missing id
  alone, beside the literal id `undefined`, and as an unsealed last answer.
- Round 2: PASS.
## 9.4e `renderHistoryView` for a ready view
- Round 1: PASS.
## 9.5a The bare URL's end
- Round 1: FAIL. P2: `\p{P}`, `\p{Z}` and `\p{L}` came from the `regex` crate's Unicode 16 tables,
  V8's are Unicode 17 (`https://x/a\u{10ED0}b`, `https://x。\u{088F}国中国`). P2: the `url` crate
  refuses `https://ab\u{088F}`, which Node's `new URL` takes, so `[x]` after it stays.
- Fixed: the three classes are V8's, from `resources/url-classes.json`; the reviewer's inputs and
  more Unicode 17 characters are golden cases. The second finding is Node's parser alone: Blink,
  where the message is drawn, refuses that host as the `url` crate does
  (`logs/review/9.5a-r1-url-engines.log`); AGENTS.md records where each engine parts from the
  standard.
- Round 2: PASS.
## 9.5b Linkify
- Round 1: PASS.
