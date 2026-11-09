# Reviews

Each round: task, round, verdict, findings and their fixes. Reviewer: an Orca worker, Claude Code
on Kimi K3.1 Pro (effort high) from 8.7c1a; Codex GPT-6.1 Sol (xhigh) for 8.6 to 8.7b2; Kimi before. Each round's verdict and
gate logs sit in `logs/review/<task>-r<N>*`.

Chapters 1 to 7 sit in `tasks/reviews-done.md`.

## 8.0 Sessions plan

- Round 1: FAIL. Missing interrupted-task resume and the provisional sweep; paging lacked the
  id tie-break; owner key truncation unstated.
- Round 2: PASS.

## 8.1 Session owner

- Round 1: FAIL. serde_json rejected numbers JSON.parse accepts, so a valid file was removed.
- Round 2: PASS.

## 8.2 Local database admission

- Round 1: FAIL. Lease owners were parsed stricter than JSON.parse; the init-failed view lacked
  the copy action.
- Round 2: PASS.

## 8.3 Task list

- Round 1: PASS.

## 8.4a Project-grouped main list

- Round 1: FAIL. Dialogue-fold keys were parsed with serde, which drops keys Cindy keeps. Fixed with `cindy_json::roundtrip` and a regression test.
- Round 2: PASS.

## 8.4b Pinned section

- Round 1: FAIL. Hidden keys skipped non-strings; Show all kept across unmounts; a fold turned after a failed save. Fixed with JS `String()`, drawn-list pruning and save-first folds, each with a regression.
- Round 2: FAIL. Pinned projects kept Show all when Pinned closed. Fixed by merging inner keys only while the body is drawn.
- Round 3: PASS.

## 8.4c Sidebar filter

- Round 1: FAIL. The scope header waited for the first load. Fixed: it always draws; regression with nothing loaded.
- Round 2: PASS.

## 8.5a Task row

- Round 1: FAIL. toFixed panicked on an overflowed cost and dropped the sign of negatives; the
  row skipped `getSessionDisplayTitle`; task info read failures went unlogged.
- Round 2: PASS.

## 8.5b1 Task menu and rename

- Round 1: FAIL. An edit whose task left the list blocked every later rename; title trims used
  Rust's whitespace (U+0085); More took no keyboard focus. Fixed: the read ends a stale edit and
  a new rename replaces it; JavaScript's whitespace class; More is a Tab stop under Ely's
  `FocusScope` around the window root.
- Round 2: PASS.

## 8.5b2 Pin and Unpin

- Round 1: FAIL. Lock records parsed with serde (a `1e400` field made a live lock look
  malformed); gate listing dropped entry errors; Windows stat opened files for reading;
  overlapping failed pins rolled back to an unsaved order; the owner folder the first pin made
  was never watched. Fixed: `cindy_json` parsing, an entry error fails closed, attribute-only
  stat, a durable baseline and a serial save queue, `watch_owner` after each save.
- Round 2: PASS.

## 8.5c1 The task preview text and the list's projection backfill

- Round 1: FAIL. The image regexes folded Unicode case (`httpſ://` passed); serde_json refused
  JSON nested past 128 levels, which V8 parses. Fixed: ASCII letter classes, and a `JSON.parse`
  port with no nesting limit whose recursion grows the stack through `stacker`.
- Round 2: FAIL. NUL stayed in previews; markdown-rs dropped a heading's leading hashes, read
  Unicode whitespace as Rust does, and ended a lazy block-quote line before an incomplete tag.
  Fixed: NUL reads as U+FFFD as micromark's preprocess does. The parser defects moved to 8.5c1b.
- Round 3: PASS.

## 8.5c1b The `markdown` crate parses as micromark does

- Round 1: PASS.

## 8.5c2a The display modes

- Round 1: FAIL. The Pinned view style trigger took no Tab or key; a claim nested past 128
  levels read as malformed. Fixed: a keyed tab stop with a button role; claims parse through
  the `JSON.parse` port.
- Round 2: PASS.

## 8.5c2b The list row

- Round 1: FAIL. Loose main-list tasks had no source label; the task menu opened over a rename;
  a cut label had no full text. Fixed in both rows: `dialogueSourceLabelMap` for loose rows, no
  menu while editing, the label's tooltip.
- Round 2: PASS.

## 8.5c2c The card

- Round 1: FAIL. An optimistic unpin kept the in-memory summary; two module docs were stale.
  Fixed: an unpin clears the summary and a failed write restores it; the docs name card mode
  and the fade's restart.
- Round 2: PASS.

## 8.5c2d The title marquee

- Round 1: FAIL. A key press dropped the hover; a title back from rename under a hovered row
  never restarted; the memo ignored reduced motion; the tests never advanced the clock. The
  track's accessibility duplicate was answered: plain string children carry no a11y node.
  Fixed: the hover ignores the keyboard, the row and the title are tracked apart, the motion
  length joins the memo, and the tests drive frames and read painted offsets.
- Round 2: PASS.

## 8.5c2e1 Pinned drag in text and list modes

- Round 1: FAIL. A move mid-slide restarted from the layout top; stale bounds and tracks were
  never pruned. A claimed drag loss on leaving the window was answered: gpui promotes a drag to
  the platform only with an external payload, which this drag never sets.
  Fixed: a new slide starts from the painted offset; each list render keeps only painted entries.
- Round 2: PASS.

## 8.5c2e2 Pinned card drag across columns

- Round 1: PASS.

## 8.6a The router

- Round 1: PASS.

## 8.6b Row click and the active task

- Round 1: PASS.

## 8.6c The row on the keyboard

- Round 1: PASS.

## 8.6d Bulk selection

- Round 1: FAIL. Selection missing from the accessibility tree (`aria-pressed`, `aria-selected`);
  Clear had no `focus-visible` ring.
- Round 2: PASS.

## 8.6e The active row scrolled into view

- Round 1: FAIL. A reveal that needed no move left the earlier scroll running; a change of look
  kept the row's active record, so the new look did not reveal.
- Round 2: PASS.

## 8.7a The New row

- Round 1: PASS.

## 8.7b1 The Search row

- Round 1: FAIL. The outside press used the row's unclipped bounds; key presses dropped the
  hover; the drawn placeholder could wrap.
- Round 2: PASS.

## 8.7b2 The search options

- Round 1: PASS.

## 8.7c1a The conversation search core

- Round 1: PASS. No findings. Before the verdict, the content argument moved from the raw column
  to `JSON.parse`'s value, as `messageToCamel` hands it.

## 8.7c1b The FTS pass

- Round 1: FAIL. `messageToCamel` lost `projectMcpAppHistory`, so App rows exposed private card
  fields to search. Fixed: `mcp_app.rs` ports the projection, trust title from the caller's labels.
- Round 2: PASS.

## 8.7c1c searchConversations

- Round 1: PASS. No findings.

## 8.7c2a useConversationSearch search

- Round 1: PASS. No findings.

## 8.7c2b1 SearchResultsBody

- Round 1: PASS. No findings.

## 8.7c2b2 the list scroll under the search

- Round 1: PASS. No findings.

## 8.7c2c handleSelect

- Round 1: FAIL. Results and hits were not Tab stops, so Enter or Space could not open them.
  Fixed: each is a focusable button; hits moved to `results/hits.rs`.
- Round 2: PASS.

## 8.7c3 Projects submenu

- Round 1: FAIL. Projects kept first-seen order, not `compareProjectsByLatestSessionDesc`; a
  choice that left the scope's ids unchanged restarted the search; two comments ran two lines.
  Fixed: `Recent` keeps `lastUsedAt` and `group` sorts by the latest of it and task activity;
  the request keys on the session ids, the scroll on the selection; comments cut to one line.
- Round 2: PASS.

## 8.7c2d The search row sticky over the results

- Round 1: PASS.

## 8.7d1 The More row and its hover menu

- Round 1: FAIL. Key presses read as a leave; Escape needed focus in the menu; no arrow-key
  open; a press on the menu cleared the search; the open trigger took focus on press; a timed
  close returned no focus. Fixed: modality-independent hover, Escape through
  `intercept_keystrokes`, Down opens onto Customize, the menu's hitbox joins the search surface,
  `prevent_default` on the open trigger, `HoverMenu` owns the return focus.
- Round 2: FAIL. Tab and Shift-Tab left the open menu. Fixed: `capture_action` stops Ely's
  `FocusNext` and `FocusPrev` in the menu.
- Round 3: PASS.

## 8.7d2 The Customize panel

- Round 1: FAIL. The rows did not make way during a drag; the search's outside press went with
  its row, so a query run into Customize stayed; a panel drag could drop on Pinned and save.
  Fixed: the panel lays rows through `sortable::arrange`; `search::listen` keeps the listener
  while the row is unmounted; a drag belongs to its list, and other lists pass it by.
- Round 2: PASS.

## 8.7e1 The rail column
- Round 1: FAIL. The pointer moved at the fade's midpoint, not with the state; the press inset
  ignored reduced motion; the hover fill dropped on a key press. Fixed: the target state picks
  the occluding view; no inset under reduced motion; hover held through `on_hover` in
  modality-independent mode.
- Round 2: PASS.

## 8.7e2 The rail Search popover
- Round 1: FAIL. A press on the panel reached the content beneath; Clear was no Tab stop; Tab
  left the panel and closed it; the rail's options menu was no popper to the inline search.
  Fixed: the panel occludes; Clear is a focusable button that hands focus to the panel; Ely's
  trapping `FocusScope` loops Tab; the inline check takes the rail's open menu, and the press
  flag is read, not taken, by both searches.
- Round 2: PASS.

## 8.7e3a The rail Pinned tiles
- Round 1: FAIL. Sortable bounds, slides and card columns were keyed by task id alone, so the
  hidden expanded Pinned row spent a rail move and the tile slid from the row; `TASKS.md` passed
  500 lines; a two-line doc comment. Fixed: those maps key by list and id; chapters 1 to 7 moved
  to `tasks/done.md`; the comment is one line.
- Round 2: FAIL. `tasks/reviews.md` passed 500 lines. Fixed: chapters 1 to 7 moved to
  `tasks/reviews-done.md`.
- Round 3: PASS.

## 8.7e3b The rail Projects and Dialogues tiles
- Round 1: PASS.

## 8.7e3c1 The rail panel shell and the Dialogues panel
- Round 1: FAIL. The Projects panel had no listeners; the out-of-view close waited for a frame;
  the tile check ignored clipping; an empty panel lost focus; Show all took no keyboard; a
  two-line comment.
- Fixed: the listeners stand alone, the tile's prepaint closes, tiles hit-test, focus goes back
  when nothing inside takes it, Show all is `fold::ShowAll`, one-line comment.
- Round 2: PASS.

## 8.7e3c2a The rail Projects panel's list
- Round 1: FAIL. The panel's scroll list was a flex column, so long lists shrank rows (32 to
  22.5px) and Show all (24 to 19.5px) instead of scrolling.
- Fixed: the rows sit in an inner column that does not shrink; the long-list test checks both
  heights.
- Round 2: PASS.

## 8.7e3c2b The rail project level
- Round 1: FAIL. A Projects tile click that reset a keyboard-opened level left focus on the
  tile; the project row's hover took a key press as a leave and a move back as an enter, so a
  keyboard level turned into a mouse one and closed after the grace.
- Fixed: `openSection` hands focus back to the keyboard row while Projects stays open; the row's
  hover ignores input modality, as `onMouseEnter` does.
- Round 2: PASS.

## 8.8a The interrupted-task candidates
- Round 1: FAIL. The `atomic_file::read` doc comment ran two lines.
- Fixed: one line; no other comment in the task runs past one.
- Round 2: PASS.

## 8.8b1 The command classifier
- Round 1: FAIL. `normalizeDisplayCommand`'s bare backslash took a whole scalar; Cindy's takes one
  UTF-16 unit, so `bash -c \😀'echo hi'` kept its wrapper there and unwrapped here.
- Fixed: an escaped astral character marks the word as having an unquoted literal; five
  surrogate cases join the display golden.
- Round 2: PASS.

## 8.8b2 The tool descriptor
- Round 1: PASS.

## 8.8b3 The activity line
- Round 1: PASS.

## 8.8c1 The dialog's tasks
- Round 1: FAIL. `by_ids` put every id in one `IN`, so 32767 candidates passed SQLite's variable limit and the load gave up; Cindy reads 256 ids a query.
- Fixed: 256-id batches merged by id (hash set and map), a test under a 300-variable limit.
- Round 2: PASS.

## 8.8c2 The confirm dialog
- Round 1: FAIL. Held Space gave no press; the face jumped where Cindy's transitions; a disabled
  confirm showed `not-allowed`; two-line comments.
- Fixed: Space presses the face while focused; `transition.rs` runs the fill and inset; the
  disabled cursor stays default; comments cut to one line.
- Round 2: FAIL. Blur hid a Space press but kept it, so a release elsewhere left the old
  button pressed on return; reduced motion did not stop a running fill.
- Fixed: blur clears the press; a zero duration ends a running transition, as `transition:
  none` does.
- Round 3: PASS.

## 8.8c3 The resume dialog
- Round 1: FAIL. A click on a row's or Select all's text did not focus its box, so Space
  went elsewhere; the row's hover went out on a key press.
- Fixed: `dialog/check.rs` draws the box with its own focus, which the label's click takes;
  the row's hover ignores input modality.
- Round 2: PASS.

## 8.9 Session browse history
- Round 1: FAIL. A disabled history button kept its action name; Cindy names it by the
  unavailable reason.
- Fixed: `history::names` gives a disabled button the reason as name and tip.
- Round 2: PASS.

## 8.10a The project header's menu
- Round 1: FAIL. A key press hid More's slot under a resting pointer, as gpui's hover styles
  follow keyboard modality; five comments ran to two lines.
- Fixed: one hover state that ignores input modality lights the header, its chevron and its
  slot; the comments fit one line.
- Round 2: FAIL. More's own hover face and icon colour still followed keyboard modality.
- Fixed: More keeps its own hover that ignores input modality for its face and icon.
- Round 3: PASS.

## 8.10a2 Remove from sidebar
- Round 1: PASS.

## 8.10b Search in this project and the dialogues' Search
- Round 1: FAIL. A press outside while the options menu held focus counted as inside, so the
  frozen list stayed at the top; the chip X's hover followed keyboard modality; three module
  comments ran to two lines.
- Fixed: a press outside the row and its poppers past an open menu restores unless the menu
  chose; the X keeps a hover that ignores input modality (`search/chip.rs`); the comments fit one line.
- Round 2: PASS.

## 8.10c Open in the file manager
- Round 1: FAIL. The refused-folder view test assumed Unix roots on every platform; the Windows
  open went on after a failed `CoInitializeEx`.
- Fixed: the view test runs on Unix, a Windows test checks `%SystemRoot%`; a failed COM init
  returns its error before any shell call, and a successful one is always paired.
- Round 2: PASS.

## 8.11 The blank space's filter menu
- Round 1: PASS.

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
