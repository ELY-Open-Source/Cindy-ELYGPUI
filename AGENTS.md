# AGENTS.md

Cindy-ELYGPUI: Cindy's desktop client in Rust, on GPUI and Ely GPUI Components. `docs/PRD.md`
defines scope and what 1:1 means.

## Stack

- Rust 1.98, edition 2024, pinned in `rust-toolchain.toml`. One crate, `cindy`, a binary.
- `ely-gpui-component` from GitHub, pinned by `rev` in `Cargo.toml`.
- gpui and gpui_platform as `gpui-pre` and `gpui-pre-platform` `=0.3.11`, Ely's pin. gpui_platform
  takes `font-kit`, `wayland` and `x11`. Ely's default feature turns on `runtime_shaders`.
- Region is set at build by `CINDY_AUTH_REGION` (`cn`, `global`, `dev`; default `global`) through
  `build.rs`. The app shares Cindy's userData folder for that region (user decision).
- Data owner (`src/session_owner.rs`, Cindy's `appSessionState.ts` read only): `app-session.json`'s
  `activeMode`; `local` is the `local-v1` owner with `owners/<first 20 hex of sha256(id)>/`.
  Signed-out and cloud owners need chapter 13. An unparsable file is removed, as Cindy's store does.
- Local database (`src/localdb.rs`): `cindy-local-v1.db` opens as Cindy's passive instances open
  a shared database, never created or migrated: the reader lease (`lease.rs`, Cindy's
  `schemaMigrationLease.ts`; records and the recovery observation stay byte-compatible), the
  pragmas, `cjk_seg` with its TEMP triggers (`cjk.rs`; character classes in
  `resources/cjk-classes.json` come from V8 through `scripts/sync-golden.mjs`), sqlite-vec 0.1.9,
  then `admission.rs` against `resources/migration-manifest.json` (Cindy's 149 migrations, also
  from `sync-golden.mjs`). The connection and its lease live on one thread (`worker.rs`). Any
  refusal shows `fatal.rs`, Cindy's `LocalDbFatalScreen`, above everything.
  `localdb/search.rs` is `conversationSearch.pure.ts` (golden `tests/fixtures/search`):
  `fuzzy.rs` in UTF-16 positions, `visible.rs` the role's visible text (`Js` objects keep
  `Object.values` order), `snippet.rs` the preview and keyword snippet in code points, each code
  point folded alone, `forwarded.rs` `forwardedRequestDisplayText`, `merge.rs` the merged order
  (ids on ICU's root collation). `fts.rs` is `chatHistorySearch.pure.ts` (tokens, the OR match
  through `cjk_seg`, the visible-token check, RRF) and `history.rs` `prepareChatHistorySearch` as
  conversation search calls it: the FTS arm with its filter, the pool cap, pages of hit rows read
  as `messageToCamel` reads them, App rows through `mcp_app.rs` (`projectMcpAppHistory`; the
  caller resolves the trust labels). The vector arm reports Cindy's disabled skip reason until 11.0.
  `conversations.rs` is `searchConversations` for the owner's own search (no host scope, so no
  source filter or remote aliases): candidates with their companions, title matches on the shown
  title, up to three content pages, then merged results with live message counts.
- Task list (`src/sessions.rs`, Cindy's `sessionsStore.ts` for the filter's status bucket): the first
  read sweeps provisional leftovers once per database open (`sweep.rs`), then `query.rs` reads
  `ordinaryTaskListFilter`'s rows. Cindy's push events never reach the port, so a watcher on the
  database, its WAL and `owners/<key>/sidebar-settings.json` (`settings.rs`) reloads, single-flight.
  `lists.rs` builds both lists from one bucket as `CCAgentSidebarUpper.tsx` does: `pinned.rs` draws the
  Pinned section, `main_list.rs` the main list in `project` or `flat` mode, from `group.rs`
  (`groupSessions`, `buildMainListEntries`) and `paths.rs` (project keys). All of it is checked against
  `tests/fixtures/grouping` from `sync-golden.mjs`. `row.rs` is the task row, with `title.rs`
  (`getSessionDisplayTitle`, golden `display-title.json`) and `meta.rs` (`SessionInfoMeta`, golden
  `meta.json`; task info read once, as Cindy caches it). `task_menu.rs` is the row's `SessionTaskMenu`
  (Ely `ContextMenu` on a right click, `menu_under` on the hover More), `rename.rs` the
  `SessionRenameInput` on Ely's `TextInput`, and `write.rs` `patchMeta`: the row patched first, the
  `UPDATE` on the database worker, rolled back with a toast on failure. The port writes Cindy's
  database the way Cindy's own main process does. `pin.rs` is `handleTogglePin`: `pinnedAt` through
  `write.rs`, and a pin promoted in `sidebar-settings.json`'s `pinnedOrder` by `settings::promote`
  (`savePinnedOrder` and `updateAtomic`: under the lock, an unreadable file refused, tmp then rename).
  `project.rs` is `ProjectNode`'s `ProjectHeader`: a Tab stop that folds on a click, Enter or Space,
  its hover More (`ProjectAction`), and one menu (Ely `ContextMenu::manual`) from More, a right click
  or the menu key: Rename, Pin or Unpin (`pin::project`, `project:<key>` promoted or removed), Copy
  deep link (`buildProjectDeepLink`). `project/rename.rs` is the alias field on a double click or
  Rename, `project/alias.rs` `useProjectAliases.updateAlias` and `upsertProjectAlias` on the database
  worker. `project/remove.rs` is Remove from sidebar: a confirm, then `settings::hide`
  (`setProjectHidden`). The other actions come with their tasks. `src/cross_process_lock.rs` is the
  advisory tier of Cindy's `crossProcessLock.ts`: the `{pid, startedAt, nonce}` file, a 2 s heartbeat,
  nonce-bound release, and stale takeover under the `.reclaim.d` gate, with owner liveness from
  `kill(pid, 0)` and `ps` (PowerShell, then WMIC, on Windows). It blocks, so callers run it on the
  background executor. `filter.rs` reads and writes the filter preferences; `header.rs` draws the scope
  header and the filter menu on Ely's `Menu` (`menu_under`). Ely's menu has no row value, tip or
  kept-open choice, and lacks some lucide icons; the nearest Ely icon stands in. A right click on the
  list's blank space opens the same menu at the pointer (Ely `ContextMenu` around the scroller,
  `handleSidebarBlankContextMenu`); rows and headers stop their own right clicks. `fold.rs` is
  `SectionCollapse` and the first-N fold. Show all is keyed by section and group and resets once its
  list is no longer drawn, as each `SessionEntryList` holds its own. Animations read
  `background_executor().now()` so tests can advance the clock. `preview.rs` is `finalizePlainPreview`
  (its LRU cache, `stripInternalWebCitations`) and `projection.rs` the list's latest-message extract
  with `scheduleSessionListProjectionBackfill`: after a read, one `UPDATE` fills the empty
  `list_preview` and `list_message_count` columns. `view_mode.rs` is `useSidebarCardMode.ts` with
  `sidebarInstallVintage.ts`: the Pinned mode (`sidebar.cardMode`) and the main list's
  (`sidebar.mainListMode`), kept in memory as Cindy's module memo, chosen from the filter menu's
  Display row and the Pinned header's view style menu. `list_row.rs` is `SessionCard`'s `list` variant
  (dividers, title line, one preview line from `sessionToCamel`'s `preview`), drawn in list mode;
  groups inset it `pl-2`. `card.rs` is the `card` variant (title, `resolveSessionCardBody`, meta row)
  and `masonry.rs` `CardMasonry`: one to three round-robin columns from the width measured in prepaint,
  with `card-col-fade`; card mode's fold height is that measured height. `marquee.rs` is
  `SidebarTitleMarquee` in the text and list rows: the row's `on_hover` picks one title, prepaint
  canvases measure it and its track, and the track scrolls on the executor clock. The hover ignores key
  presses, as CSS `:hover` does. An unmounted row lets go at the next list render; a title back from
  rename starts again. Tests drive frames with `simulate_next_frame`. `sortable.rs` is `SortableList`'s
  native drag for the Pinned text and list modes: gpui's `on_drag` per entry, the entry under the
  pointer takes the dragged one's place (`swapThreshold: 1`), displaced entries slide 150ms, a drop on
  the list saves; Escape, a drop elsewhere or a lost window put the order back. Cards drag the same way
  across `DraggableCardColumns`' round-robin columns (`sortable/order.rs`): into another column an
  entry goes after a target it sat above, else before; a column's empty tail appends; the drop saves
  `reorderByDropSlot`'s `row * N + column`. Slides move in x and y. `reorder.rs` is
  `handlePinnedReorder` (`buildManualPinnedReorder`, golden `pinned-reorder`) and `settings::save`
  takes `savePinnedOrder`'s `reorder`, rebased on the file as `rebasePinnedReorder` does. A press on a
  group's tasks does not drag the group.
- Interrupted tasks: `src/sessions/interrupted.rs` is `listInterruptedResumeCandidateRows`: active
  tasks whose turn started before this process (`cross_process_lock::started_ms`, Cindy's `bootAtMs`)
  and never ended, companion tasks through the send path's guards and the owner's
  `companion-cloud/shadowed.json` (read with `atomic_file::read`, `readAtomicFileSync`), each with its
  turn's last message time and its last `tool_use` as `activity`. `load` runs once per owner when the
  database is ready. `interrupted/tasks.rs` is `loadInterruptedTasks` in the same read: `getMany`'s
  rows (`query::by_ids`), companion names when a companion task is there, newest first. A Cindy beside
  the port, mid-turn, looks interrupted to it, as to a passive join. `interrupted/dialog.rs` is
  `InterruptedTasksResumeDialog` on `ConfirmDialog`, drawn from the main window's root: it opens when
  an owner's load with tasks lands and the window has not handled that owner; closing marks it. Rows
  reuse `lists::labels` and `row::vendor`. `dialog/check.rs` draws the native box (Space, not Enter;
  focus on press) in `confirm-btn-primary-bg`, and its `label`: a click focuses the box; box and label
  set one target state, so a press toggles once. The row's hover ignores keys, as CSS `:hover` does.
  Continue rechecks the candidates; with no send before 10.0 each picked task still interrupted fails
  with the `continueFailed` toast. The release notes that suspend it come with their chapter.
- Command intent: `src/command_intent.rs` is `commandIntent.ts` (`from_command`, `from_actions`,
  `intents_from_actions`), actions named as Cindy spells them; `shell.rs` its lexer (JS `\s`,
  `trim`, lengths in UTF-16 units). `src/command_display.rs` is `normalizeDisplayCommand`.
  Golden `command-intent` holds the literal inputs of Cindy's tests, run through Cindy's code.
  `src/tool_use.rs` is `describeToolUse` (`parseToolName`, `truncateToolText`, `humanizeToolToken`)
  kept to the fields rows and history read; golden `tool-use` projects Cindy's descriptor to them.
- History rows: `src/message_rows.rs` holds the row checks `projectHistoryView` reads, on rows as
  `messageToCamel` reads them (`parseMessageToolUse` with its gateway unwrap,
  `messageContentToPreview`, `isDeliveryProseText` with JS `\s` and line terminators,
  `isContinuationMessage`, `hasVisibleHistoryResult`); `src/payload.rs` is `payloadSummary.ts`'s tool
  result readers, `payload/files.rs` the file references (a source-literal lexer on UTF-8 bytes, ASCII
  compares; `searchParams` is the `url` crate). `js::stringify` is `JSON.stringify`, `js::entries` the
  property order. Golden `history-rows`: `record.mjs` records the arguments of Cindy's tests (vitest
  stubbed), adds `cases.json`; read with `js::parse`. `src/tool_row.rs` is `formatIslandToolDetail`
  over `createLocalizedToolRowWording`: its `activity` checked in every locale (`activity.json`).
  `src/plan_todos.rs` is `messageRender.ts`'s plan timeline (`tasks.rs` the `Task*` list, `window.rs`
  what a paged window proves); golden `plan-timeline`. Golden `turn-yields`: `src/tool_media.rs` is
  `extractToolResultMedia`, `src/generated_files.rs` `collectGeneratedFiles`, `src/local_path.rs`
  `resolveToolFilePath`. `src/render_items.rs` is `buildRenderItems` (golden `render-items`),
  `src/work_groups.rs` `messageWorkGroups.ts` on `Rc` items (golden `work-groups`); the recorder's clock
  and random ids are fixed. These readers take the store's rows (`merge::Fields`), so no row is copied;
  `render_items::prop` reads a field's own object. `tool_media::extract` keeps
  `getToolResultMetadata`'s LRU (512 results, 32M UTF-16 units; a result that names no media is not
  kept, since its check costs less than its copy) and `generated_files/cache.rs` is
  `collectCachedGeneratedFiles`, weak on its rows. `src/history_render.rs` is `renderHistoryView` as
  `buildMessageStreamRenderProjection` calls it for a ready view (`bind.rs` its `bind` with the
  stream's `structure`; golden `history-render`, run through Cindy's own projection):
  `render_items::Deferred` holds a group's ranges and its actions take the view.
  `session_view/items.rs` holds the routed task's projection and renders again after each change of its
  view. A local read (`transport.rs`) leaves its caller's stack before it borrows the app, so a click
  may call the view.
- History view: `src/work_runs.rs` is `groupWorkRuns` behind an `Adapter` (golden `work-runs`, toy
  adapter `scripts/golden/workRuns.mjs`). `src/history_view.rs` is `projectHistoryView` (subagent bodies
  as a `deferred` range), `historySubagentScopes`, `completesHistoryTurn`; `source.rs`
  `projectHistorySourceView` (`foldAnswerTail: false`), `json.rs` Cindy's key order. `rows.rs` is
  `readMessagesList` and the reader's anchor, hydrate, validate and revision, rows as
  `messageToCamelWithRowid`; `outline.rs` the outline SQL; `artifacts.rs` `withHistoryArtifacts` (its
  LRU of 2048). `reader.rs` is `createHistoryViewReader` over `Deps` (`db.rs` the database's; turns and
  live rows come with 10.0): `page`, `details`, the detail cache. Reads queue on the database worker.
  `controller.rs` is `HistoryViewController` (network, cache, intents: 20.0) in JS promise order.
  `src/chat_message.rs` is `mapServerMessages` (`Out` keeps `undefined` keys; golden `chat-message`):
  throws are `Err` with V8's text (`js::try_string` is a throwing `String()`). `merge.rs` is
  `mergeMessages` and `confirmRemoteUsers` on `Rc` rows (`===` is `Rc::ptr_eq`, `None` a key set to
  `undefined`; golden `merge-messages`). `transport.rs` is the view's local `page` and `details` (a
  `Reader` per database open). `src/chat_store.rs` is the store's local slice: `ensureInitialMessages`
  and `loadOlderMessages` through the view, an add-only mirror (borrowed thinking and read bodies
  retire), counted visits, the forced resume read; the watcher invalidates views. `raw.rs` is
  `listMessagesFor` and `runRemoteReconcile`, `older.rs` raw pages, `window.rs` timeline and islands,
  `memory.rs` the LRU cap of 20, the trim and the soft eviction. `src/js_sort.rs` is V8's sort
  (`array-sort.tq`'s TimSort). `src/js_date.rs` is V8's `Date.parse` (`parser.rs` ports `dateparser*`;
  host-zone local times via jiff, as ICU's `FORMER`) and `toISOString`. Goldens `js-sort`, `js-date`
  (Cindy's Electron, three zones). `src/command_output.rs` is `commandOutputPaths.ts` (`js_re`,
  lookarounds by hand); offsets in bytes, 240/80 windows in UTF-16 units (a cut pair's half is U+FFFD).
  Goldens `history-view` (Cindy's tests less a lease and `import.meta.env`), `history-artifacts`.
- User message: `src/url_boundary.rs` is `urlTextBoundary.ts` (`clip` is `clipBareHttpAutolink`,
  `bare_http` the scanner's `BARE_HTTP_URL_RE_SOURCE`, `host.rs` the prose boundary with the IDN dots a
  hostname keeps): it works in UTF-16 units and answers in bytes. `\p{P}`, `\p{Z}` and `\p{L}` are
  V8's, read from `resources/url-classes.json` (`sync-golden.mjs` writes it; `v8_classes::within` looks
  a character up), since the `regex` crate's tables trail V8's Unicode. `new URL` is the `url` crate,
  which follows the URL standard on Unicode 17 data. Cindy's two engines each part from it: Blink,
  where the message is drawn, takes an ASCII host's broken `xn--` label unchecked, and Node, where the
  golden runs, reads a host through ada's older IDNA tables (`logs/review/9.5a-r1-url-engines.log`).
  Golden `url-boundary` holds the inputs of Cindy's tests and `cases.json`. `src/js_re.rs` turns a
  JavaScript regex source without the `u` flag into a Rust regex. It refuses a bare dot: a caller
  spells the units JavaScript's dot takes. `src/deep_link.rs` is `deepLinkSchemes.ts` and `deepLink.ts`
  for links read from text (the `cindy` and `xdt-maker` schemes, `parse_session`, `parse_project`,
  `label_start`; a project link's lookahead is `project_ends`). `src/linkify.rs` is
  `findLinkifyMatches`, ranges in bytes: no two kinds of link start at one place, so its comparator is
  a sort by start. Golden `linkify`. `src/user_message.rs` is `userMessageDisplayText.ts` (`orca`,
  `hook_group_context` with `captureImContext`'s group envelope, `display_text` over 8.7's
  `forwarded::display_text`; the hook source is the stored `Js`) and `collapse.rs`
  `mayExceedVisualLineThreshold` in UTF-16 units. Golden `user-message`. `src/chat_quotes.rs` is
  maker-shared `chatQuotes.ts` (`format`, `segments`, `join_text`, `strip_marker_lines`, `source_label`
  for `quoteSourceDisplayLabel`). `src/user_message/sent.rs` is the sent text of `UserMessage.tsx`:
  `tokens` (`buildSentInlineTokens`, its ranges sorted by `js_sort`, since the comparator is no total
  order), `inline_plain` with `label` (`sentAgentReferenceDisplayLabel`), `slash_label`
  (`slashCommandDisplayLabel`), `Span::read`, `Pasted::read` and `Reference::read` over the stored
  ranges, `text_starts`, `within` (`projectSentRanges`) and `parts`, the command, mention and path
  split of `renderContentWithoutPastedText`. Golden `sent-text`: `scripts/golden/sentText.mjs` reads
  `parts` off Cindy's own `renderContent`, so the golden bundle builds with `jsx: 'automatic'`.
  `src/link_open.rs` is `useLinkOpenPreference.ts` (`is_local`, golden `link-open`; `preference` with
  its memo per kind and the legacy key's one migration, in the `LinkOpen` global) and
  `linkInteraction.ts`'s `separate`. `src/open_external.rs` is the `shell:open-external` handler for
  `http:` and `https:`, then Electron 42's `shell.openExternal`: Linux starts `xdg-open` with
  `MM_NOTTTY=1` and answers once it started; macOS (`workspace.rs`) asks `NSWorkspace` for an
  application, then `openURL:configuration:completionHandler:` with `activates`; Windows (`shell.rs`)
  runs `ShellExecuteEx` `open` on a COM thread with the quoted `escape`, Chromium's
  `EscapeExternalHandlerValue`. Native code is checked in a scratch crate
  (`logs/review/9.5f-r1-native-*.log`). `src/file_presentation.rs` is maker-shared
  `filePresentation.ts` (`kind` is `getFileVisualKind`, `glyph` `FILE_VISUAL_GLYPHS`; golden
  `file-presentation`).
- Router: `src/router.rs` is `router.tsx`'s hash routes for chapter 8: one pathname in the `Router`
  global, starting at `/`. `/` goes to `/cc-agent` (`MainEntryRedirect`; the remembered entry needs
  13.0's owner-scoped storage); `/cc-agent` waits for the task list, then opens the last chat view
  (`lastViewStore`'s chat slot, kept by `navigate` as `useTrackLastView` does), the first task, or
  `/cc-agent/new`. The chat pane shows its loading line meanwhile. `sessions/open.rs` is
  `handleSessionClick`: a plain click on a row, list row or card opens `/cc-agent/<id>` unless it is
  shown; a Shift, Mod or Ctrl click waits for 8.6d's selection. The active task
  (`router::active_session`) draws as Cindy's: its pill and ring (an absolute bordered overlay stands
  in for the inset shadow), foreground and tints. The task menu wraps a renamed row too, so a press
  that commits a rename lands its click; the field stops right presses. `sessions/keyboard.rs` hosts
  that menu (Ely `ContextMenu::manual`) and makes each row, list row and card a Tab stop: Enter or
  Space on the row itself opens it; the menu key (key up on Windows) or Shift+F10 (not macOS) opens the
  menu at the focused row's or More's centre, as Chromium's `ShowNonLocatedContextMenu` does. Prepaint
  probes record those visible bounds. `select.rs` is the bulk selection of `handleSessionClick`: the
  row probes also list the rows painted (the `Drawn` global, pinned card slots marked with
  `rowMajorRank`), which stands in for `getVisibleSidebarSessionIds`; a probe at the list's end prunes
  the selection, as the MutationObserver does. The bulk bar has Clear; Archive and Delete come with
  18.0. `reveal.rs` is `scrollIntoNearestView`: a row that turns active, or mounts active, is scrolled
  to CSSOM's `nearest` on the list's `ScrollHandle`, along Chromium 148's desktop programmatic curve
  (`cubic-bezier(0.4,0,0,1)`, `min(sqrt(|delta|), 90) / 60` s in device pixels); reduced motion jumps,
  and a scroll by the user or a reveal that needs no move ends it. The active record is keyed by look
  (text, list, card), since Cindy mounts a new component per look. `search.rs` is the conversation
  search: two `useConversationSearch` instances (`Which`), the inline one `conversationSearchContext`
  shares and the rail popover's, each with its own Ely `TextInput`, filters and run, one sort on
  `Search` for both; and the top nav's Search row (`SidebarInlineSearch`) at the top of the list's
  scroller. Focus goes straight to the field, which stands for `forceOpen`; the placeholder is drawn
  over the field in `text-placeholder`, since Ely draws its own in `fg_subtle`. A press that does not
  hit the row (gpui's hit test, so a clipped row misses) clears a running query; hover ignores key
  presses. `search/options.rs` is `SearchFilterMenu` on Ely's `Menu` (`menu_under`) in the open row:
  sort (saved under `cc-agent.search.sortBy`), status, projects, agent and last activity, reset and the
  reminder dot. Focus inside the menu stands for `menuOpen`; a press past the open menu clears the
  query once the press is done, unless a row took it (Ely chooses on mouse down). `search/projects.rs`
  is the Projects filter: `allKnownProjects` from a second read of every status (Orca workers, pins and
  hidden projects left out), `nextProjectSelection`, and the selection reconciled after each read. Its
  tasks' ids scope the search, so a choice that adds no task keeps the page; the list's scroll keys on
  the selection. `group.rs` sorts projects by their latest task or `lastUsedAt`. Ely's rows have no
  value slot, so a project's task count joins its label. `search/run.rs` is `useConversationSearch`'s
  effect: a change of the trimmed query or an option starts over (dropped tasks clear the timers); the
  keyword pass at 250ms and the hybrid pass at 900ms run `searchConversations` on the database worker,
  only the latest publishes, the hybrid page wins once it lands, and an empty page waits for both.
  `fanout` is the renderer's `mergeConversationSearchFanout` (Cindy Make rows dropped, `max(userSendAt,
  updatedAt)`). `search/results.rs` is `SearchResultsBody` with its rows, drawn while a query runs; the
  list stays built under `display: none`, so gpui skips its prepaint and its rows leave the selection,
  as Cindy's hidden rows do. Its hitbox joins the search surface. `marks.rs` draws title and keyword
  marks as text highlights, which have no padding or radius. A click on a result or hit is
  `handleSelect`: its task opens and the query stays; the hit's `searchJump` lands with the chat view
  (9.14). Results, hits and More are Tab stops with the button role; gpui clicks them on Enter or Space
  key up. `search/scroll.rs` is the sidebar's layout effect: the list's offset is kept while it shows,
  each search starts at the top, and a clear puts the offset back once the pointer lifts.
  `search/stick.rs` is `pinSearch`: while a query runs the row's prepaint offsets it to the scroller's
  top (gpui has no `position: sticky`), and the content after it paints and hits under a content mask
  that starts at the row's bottom, as the clip-path effect clips. `search/lock.rs` is
  `requestConversationSearch` with the lock it brings. The project menu's Search and the dialogues'
  (`sessions/dialogue.rs`, `SessionGroupNode`'s header: a Tab stop, its menu on a right click, the menu
  key or Shift+F10 under its left edge) lock the inline search, and the rail's while its view shows
  (`isCollapsed`), to a project (`[key]` selected, its own ids while none is known) or a task group
  (sorted ids, counted as a filter). The field takes focus; the inline row shows the chip
  (`search/chip.rs`), whose X is `clearLock`, as the rail tile's click is. The options disable the
  project rows and Reset keeps the locked project. A project hidden or not known lets the lock go.
  While a menu's search opens with no query, `scroll.rs` freezes the list at the top until a press
  outside the search lifts (a press past an open menu counts unless the menu chose) or a Tab stop
  outside it takes focus (`focusin`; the root's focus is not a Tab stop, as `body` is not).
- Session view: `src/session_view.rs` is `CCAgentSessionView` for a local task: `CindyMakeSessionGate`
  (a task any held bucket lists opens at once, else `get` reads it; a miss or failure draws
  `SessionEntryError`; a Cindy Make source goes back), the task the view holds (the latest read, else
  the `active` bucket's row; each watcher reload reads it again), a deleted task purged and left, the
  store's `enterView`, `ensureInitialMessages` and `leaveView` with the route. `header.rs` is
  `SessionContentHeader` in the header slot (which shows on macOS while it holds one): the pin, the
  title (its press followed over the whole window, as pointer capture, with a 4px dead zone before it
  moves the window; a double click renames with `rename::header_field`, whose edit outlives a task
  switch, commits to the task shown and goes with the header), More (a Tab stop) with `task_menu` under
  `Scope::Header`, whose pin writes `pinnedAt` alone. `width.rs` is `useProportionalWidth(914)` and
  `column.rs` the view's section: a canvas measures the container and lays the section out in the same
  frame (`prepaint_as_root`), the message column at `messageWidth`, the dock (fade, `pb-5` zone, the
  composer's row at `inputWidth`) at `inputPad`. `stream.rs` is `MessageStream`'s list: the scroller,
  the `pt-7` column at `messageWidth` and `msg-stream-items`, a row per render item; only an ordinary
  user message draws so far. `user_row.rs` is `UserMessage`'s bubble: the display text (quote text
  joined), `ChatBubble`'s user face, and the long message's fold. `flow.rs` breaks a line into rows
  itself, since gpui's text holds no element: Blink's break opportunities (`breaks` is
  `NextBreakablePosition`: a break after a run of spaces, the pair table
  `resources/line-break-pairs.txt` for U+0021 to U+00FF, which `scripts/blink-line-breaks.js` reads off
  Cindy's Electron, ICU's UAX 14 for the rest and for a hyphen before a letter past ASCII, with LB20a,
  which ICU4X lacks) over the shaped glyphs' advances (`atoms`, read in x order, so right-to-left text
  keeps its advances), a chip a fixed-width atom at U+FFFC, Blink's own stand-in; a word too long
  breaks where it overflows (`overflow-wrap: anywhere`), and spaces, tabs and U+3000 at a wrap hang
  unseen (`logs/review/9.5e-r1-blink-lines.log`, `-blink-spaces.log`). `flow/draw.rs` draws
  `renderContentWithoutPastedText`'s nodes (`sent::parts`, each text piece apart) row by row, each row
  a flex row of words, links and chips: no CR, a final line feed ends its line, a line that wraps fills
  the bubble. A row's parts stand left to right; Blink's reordering of mixed directions across a chip
  is not ported. The measure shapes with the window's text style, which no element above the text
  changes. The fold counts the plain text's rows while the row is built, so Cindy's estimate and
  remembered results are not ported; a folded message draws its first rows and `clamp` ends the last
  with the ellipsis. A row with a chip grows as Blink's line box does: each chip's margin box (24px, a
  quote island's 26px) centres half an x-height over the baseline
  (`logs/review/9.5i-r1-blink-chip.log`). `chip.rs` is `InlineReferenceChip`'s inert face for the
  legacy command, agent, folder and file chips (`FileTypeIcon` through `file_presentation`; lucide
  0.468's `FileChartColumn` and `FileVideo` live in `lucide.rs`), sized by layout up to
  `max-w-[min(240px,55vw)]`, with `Tip` after 300ms (`tip::mono` for a path). A label's white space
  collapses to single spaces, as `truncate`'s `nowrap` draws it. `UserFileChip`'s click and menu come
  with 9.13. `renderContent`'s stored ranges (`user_row::ranges`, `projectSentRanges` from where the
  shown text starts in the content) cut the text first (`sent::tokens`): a slash range draws the
  command chip, a pasted range its words with `tip::framed` (the text in a frame 320 wide that scrolls
  past 256, hoverable), a reference `SentAgentReferenceChip`'s inert face. The fold's measure reads
  those chips as their words (`inline_plain`). A `quotesEncoded` message is a list of `draw::Island`s:
  each text island takes the ranges that lie in it (`sent::text_starts`, then `projectSentRanges` from
  the island's start in the content), and a quote between two draws `QuoteChip` (`chip::quote`: `mx-1`,
  no lift, the label through JavaScript's white space, the tip the quote in its marks over its source's
  `chat_quotes::source_label`). A folded message reference draws that chip in line. `link_chip.rs` is
  the face of `SessionLinkChip` and `ProjectLinkChip`, for a deep link in an unfolded message's text
  and for its session, project and message references: the author's label, else the task's title
  (`get`, the draft title as the unnamed label; a task of another device keeps `A session` until the
  device link) or the folder's name; a message link is a quote chip, underlined and not lifted, of the
  message's text (`resolveSessionMessageText` through `rows::anchor`, `sessionMessageDisplayText`). The
  reads live with the mounted content (`Found`), asked once each after the frame that lacked them. A
  chip that acts is underlined and lights to `surface-hover` under the pointer. A link that names no
  task or folder stays its words. An image path is underlined and breaks as `word-break: break-all`
  (`flow/break_all.rs`, Blink's `kBreakAllLineBreakClassTable`;
  `logs/review/9.5l-r1-blink-breakall.log`). The message link's native `title` has no port: gpui holds
  one tooltip. What the link chips do comes with 9.5m, an image path's click with 9.13. gpui snaps a
  line's height to the device pixel, so a compact line is 22.5px for Cindy's 22.4. The unfolded
  messages live with the mounted content, as the component's state does. `links.rs` is
  `UserMessageUrlLink` with `useOpenWithMenu`: an unfolded message's text pieces are each read for web
  addresses (`linkify`), underlined by text highlights, and a canvas after a piece lays a hit box over
  each stretch of a link's glyphs, ordered by x since the shaper lists right-to-left text from the
  right (`prepaint_as_root` at the laid-out text's positions). A link's boxes on every row share its
  Tab stop (the first box), the menu and the record of boxes the menu key reads. A click is
  `activateMessageUrl` (a modifier or the middle button opens the system browser, else the preference;
  the sidebar browser comes with 17.0, so that open fails with Cindy's toast), Enter and Space activate
  on key down with their modifiers, a right click, the menu key or Shift+F10 opens the menu (Ely
  `ContextMenu::manual`; Ely's `Link` for `link-2`). gpui places an underline itself, so
  `underline-offset-2` has no port. In tests `open_external` records the address and starts no program.
- Previews: `src/markdown_text.rs` is `markdownPreviewText` over the `markdown` crate's mdast
  (wooorm's markdown-rs, the mdast remark-parse and remark-gfm build). The crate is vendored in
  `vendor/markdown` (1.0.0, upstream otherwise) with three patches marked `Cindy:` where it parts
  from micromark: ATX heading text, character classes as micromark reads UTF-16 code units, and
  laziness at end of file. `src/rich_answer.rs` is `richAnswerText`: `cindy-ui` fences split from
  Markdown, `stream.rs` replays the ops once, `values.rs` resolves them, `js.rs` parses them as
  `JSON.parse` does (`Infinity` apart from `null`, no nesting limit: its recursion grows the stack
  through `stacker`, as V8 nests a million levels). Goldens in `tests/fixtures/preview` come from
  Cindy's functions; `sync-golden.mjs` stubs Electron, which `mapper.ts` imports.
- Dev profiles (`src/dev_profile.rs`, Cindy's `devCliFlags.ts`): debug builds stand for Cindy's
  source run. `--isolated[=<name>]` or `XDT_ISOLATED=1` (+ `XDT_ISOLATED_NAME`) runs in
  `<userData>-dev2[-<name>]`; `XDT_USER_DATA_DIR` picks a folder (a sandbox only with
  `XDT_USER_DATA_DIR_EPOCH=1`); `--isolated` on an official profile stops startup. Paths compare
  through `dev_profile::canonical::path`. Release builds always use the region folder.
- i18n: `src/i18n.rs` ports Cindy's i18next setup over `locales/`, embedded in the binary. Plurals
  and number formats come from ICU4X (`icu_plurals`, `icu_decimal`, `icu_locale_core`), as `Intl`
  gives them to Cindy. Ely's `I18n` global is set to the nearest Ely locale for Ely's own text.
- Themes: `src/theme.rs` resolves Cindy's tokens from `themes/` as `theme-service.ts` does;
  `src/theme/css.rs` evaluates their CSS colors (hex, rgb, hsl, `color-mix`, relative `oklch`). On
  Linux, `src/os_appearance.rs` reads the portal's color scheme before the window opens, since gpui
  learns it only later; gpui's reports count once they agree with it.
- Local themes: `src/theme/local.rs` reads `~/.cindy/themes/` as Cindy's loader does and
  `src/theme/normalize.rs` applies `normalizeLocalThemeColors`; local families follow built-ins.
- Omarchy: `src/theme/omarchy.rs` reads `~/.local/state/omarchy/current/theme/vscode-theme.json`
  through the VS Code converter as family `omarchy`, never writing Omarchy files. An inotify
  watcher (`notify`) reloads every theme on each finished `theme.name` write; a read inside
  Omarchy's folder swap keeps the last good theme.
- Fonts: `src/appearance_settings.rs` reads Cindy's shared `appearance-settings.json` (fonts only so
  far) and sets Ely's font scale (`uiSize / 14`) and families; gpui takes the first of the list.
- Main window: `src/main_window.rs` opens it as Cindy's `createWindow` does (app id per region from
  `src/region.rs`, title, 800x600 minimum) and keeps `window-state.json` with `electron-window-state`'s
  rules (`src/window_state.rs`). `src/activation_click.rs` swallows the activating click per platform
  as Cindy does. Theme glue for windows lives in `src/theme/shell.rs`. Hyprland 0.56 takes Lua for
  `hyprctl dispatch` (`hl.dsp.window.close({ window = "address:..." })`).
- Layout: `src/layout_tree.rs` ports `shared/layoutTree.ts` (tree, `validateLayout`, `coerceLayout`,
  the operations in `ops.rs`); `store.rs` keeps `layout.v1.json` as `LayoutStore.ts` does, healed on
  read, strict on write. Unknown fields stay as written. `cindy_json::roundtrip` reads JSON as
  `JSON.parse` then `JSON.stringify` would. Cindy's dev-console `__cindyLayout` is not ported.
- Layout on screen: `src/layout_root/` ports `LayoutRoot.tsx`. `geometry.rs` holds its sizing math
  (visible-share ledger, floors, root widths, self-heal, divider room and chat relay); `divider.rs` the
  divider gestures; `view.rs` the `LayoutRoot` view. The view lays panes out itself, so the widths it
  paints stand in for Cindy's DOM measurements. Its width is the window less the sidebar column, so the
  first frame is already right. A divider drag listens on the whole window and writes the store once on
  release; self-heal runs 120ms after width or tree settle, never mid-drag. The view follows
  `LayoutStore` through `observe_global`, which stands in for `layout:changed`.
- App shortcuts: `src/app_shortcuts.rs` ports `shared/appShortcuts.ts` (registry, normalize, effective
  map, scope overlap); `store.rs` reads `app-shortcuts.v1.json` at startup into the `AppShortcuts`
  global, normalized from `JSON.parse`-shaped fields (an overflowing flag is `Infinity`, so truthy);
  `keys.rs` turns a gpui keystroke back into Cindy's `KeyboardEvent.code`. gpui reports the typed key,
  not the physical one, and drops Shift for a shifted symbol on all three platforms, so a US-layout
  table restores the code: on AZERTY or Dvorak a shortcut follows the letter. Keypad digits, decimal
  and Enter reach the app as their main-keyboard twins (`Digit1`, not `Numpad1`): gpui's
  `keystroke_from_xkb` strips `kp_` and `KeyDownEvent` carries no scancode. Only a gpui change that
  exposes the physical key closes this; keypad `+` and `-` survive. A shortcut handler sits on the
  window's root element in `capture_key_down`. The root holds focus while nothing else does
  (`on_focus_lost` hands it back), so every key's dispatch path runs through it; with nothing focused
  gpui dispatches to the tree's own root only. Ely's `FocusScope` wraps the root on its own handle, so
  Tab and Shift-Tab walk gpui's tab stops.
- Sidebar frame: `src/sidebar.rs` ports `Sidebar.tsx` and the width, rail and hidden state
  `MainLayout.tsx` keeps; `sidebar/resize.rs` ports `useHorizontalResize.ts` with the sidebar's limits.
  Width and rail live in the preferences file (`sidebar-width`, `sidebar-rail`), the hidden state in
  the layout tree. The content area's width is the window less the sidebar's painted width, read every
  frame of its 250ms width transition. `sidebar/top_nav.rs` is `SidebarTopNav.tsx`'s fixed section: the
  New row, off the rail. `top_nav/more.rs` is the More row after Search with
  `SidebarNavigationMoreMenu`, drawn by the port (Ely's menus cannot close on leave and take focus) on
  Ely's `surface`: right of the row, flipped left without room, below the 58px titlebar strip.
  `sidebar/hover_menu.rs` is `useHoverOpenMenu` on the executor clock: hover ignores key presses, a
  press on the open trigger takes no focus, and a close returns focus to the trigger unless the open
  was a hover or the close a press outside. Escape closes it through `intercept_keystrokes`, wherever
  focus is; a press on the menu counts as inside the search, as Radix's popper wrapper does. With no
  data owner nothing is hidden, so it holds Customize alone. `top_nav/customize.rs` is
  `SidebarNavigationCustomize` in place of Search and More: the draft lives in `More.customize`, its
  rows drag through `sessions::sortable` as a list of its own (a drag belongs to its list; others pass
  it by), a press on the checkbox focuses it and neither drags nor reaches the row, and the grip moves
  on Up and Down. `search::listen` keeps the search's outside press while its row is gone. Done stores
  nothing, as `setSidebarNavigationPrefs(null)` does, until 13.0's data owner. Ely's Subtle and Ghost
  buttons stand in for Cindy's primary and quiet ones; Puzzle for `plug`.
- Rail: `sessions/rail.rs` is `CCAgentSidebarUpper`'s two views. Both stay drawn; the sidebar's `upper`
  tween fades `CollapsedView` in over 200ms `ease-in-out` while it is a rail or hidden and not peeking,
  and the shown view paints last with `occlude`, standing in for `pointer-events-none`. `CollapsedView`
  holds the New tile (`SidebarIconButton`, its face inset 1px on press), Search, the divider and
  `RailNav`. `rail/search.rs` is `ConversationSearchBox`: the tile toggles a popover at its right over
  the rail's search, enabled only while open; Escape (`intercept_keystrokes`), a press outside (unless
  the open options menu keeps it), focus leaving the panel or the tile close it and reset the query,
  focus going back to the tile unless the close came from outside. A chosen result closes it first. The
  panel occludes what lies under it and loops Tab in Ely's trapping `FocusScope`. The inline search
  counts the panel and its open menu as poppers. `rail/pinned.rs` is `RailNav`'s Pinned tiles after the
  divider: pinned tasks (archived and Orca workers left out) by `manualPinnedOrder` rank,
  `pinnedTileLabel` in UTF-16 units, a click as `handlePinnedClick`, a drag through `sortable` as its
  own list (bounds, slides and columns keyed by list, so the drawn-but-hidden Pinned section keeps its
  own) into `reorder::reorder` (`handleRailPinnedReorder` is `handlePinnedReorder`). The hover card,
  `SessionPreviewCard`, sits at the tile's right from the bounds its prepaint recorded and has no
  hitbox; running and unread faces come with 10.0. `rail/sections.rs` is the Projects and Dialogues
  tiles over `railPanelStore` (the `Panels` global): a hover opens the section at the tile's bounds and
  hides the pin preview, leaving closes it after 120ms unless a key opened it, a click opens it
  (`is_keyboard` for the key flag), a pin click closes all. `rail/panels.rs` is `RailPanels`:
  `RailPanelShell` deferred at the tile, the Dialogues panel (`compareDialogueSessions` by recency,
  `sidebar.collapse.dialogueLimit` 10, its own Show all) and the closes: a pointer move off the panel
  and tiles schedules one, a press outside or Escape (`intercept_keystrokes`) closes, so do the rail's
  end and a tile out of view (its prepaint against the content mask stands in for
  `IntersectionObserver`). A row's field or menu holds the panel: Ely's menus focus a handle outside
  the tab order, which stands in for Radix's modal overlay; a choice in such a menu marks the press as
  inside. A keyboard open moves focus in (`focus_next` from the panel; with no stop inside, focus
  stays) and Escape gives it back to the tile. The tiles' keep-alive is a hit test on their hitboxes;
  Show all is `fold::ShowAll`, a keyboard button the main list shares. `row::Scope` gives the panel's
  rows their own rename, marquee and selectors; they neither reveal nor join the selection, and a click
  only navigates (`handlePanelSessionClick`). `panels/projects.rs` is the Projects panel:
  `Lists.unclassified` (5, then Show all), then `Lists.rail_projects` (`visibleRailProjectsWithVendor`,
  pinned projects kept, unpinned tasks only, `sortProjectsForSidebar`; 20, then Show all), each row a
  tab stop. The Cindy Make group is absent: `CINDY_MAKE_ENABLED` is off, so no such task loads.
  `panels/level.rs` is the project level (`openProject`, `scheduleProjectClose`): a row's hover or
  click opens its tasks 8px right of the row in a second shell; its grace runs whenever the pointer is
  off the level and every project row (both hitboxes, recorded in prepaint), so either hover order
  holds it. A keyboard open on either level takes no grace and moves focus in. The lamps come with
  10.0.
- Preferences: `src/preferences.rs` is a global; `set` writes the file atomically, as
  `localStorage.setItem` stores strings. `cindy_json::js_to_number` is JavaScript's `Number`.
- Window chrome: frameless as Cindy opens it. macOS hides the title bar and keeps the traffic
  lights at 12,16; Windows hides the caption (`appears_transparent`); Linux asks for client
  decorations. `src/chrome.rs` draws Cindy's `WindowControls` on Windows and Linux with gpui's
  `WindowControlArea`, as Ely's caption buttons do; their close runs the same window-state save as a
  system close. Linux resizes from bare edge grips (Ely's `window_inset`, no drawn frame); on
  Hyprland a floating window's border belongs to the compositor (`resize_on_border`). The sidebar's
  top row and the chat column's `ContentHeader` (`layout_root/view/header.rs`) drag the window
  through Ely's `drag_region`.
- Chrome cluster: `src/chrome/actions.rs` ports `ChromeActions.tsx` (x 78 by the mac traffic
  lights, 8 elsewhere; drawn deferred over the sidebar and header). Its sidebar toggle drives the
  peek: `src/sidebar/peek.rs` is `useSidebarPeek.ts` as a clock-driven state machine (deadlines,
  `tick`), `peek_view.rs` wakes on its next timer, decides drawer and toggle hits from the pointer
  position, and draws the drawer deferred over the content. `src/tip.rs` draws Cindy's `Tip`
  through gpui's tooltip (500ms, by the pointer, not under the button); `tip::rich` takes any content.
  `src/browse_history.rs` is `sessionBrowseHistory.ts` with `useSessionBrowseHistory`: one `History`
  global (50 task ids, emptied when the data owner changes) that follows the `Router`; a hop waits
  for its route before it moves the cursor. `chrome/history.rs` is `SessionHistoryButtons` on
  `ChromeIconButton`, after the toggle; the window root's `capture_key_down` runs the shortcuts and
  stops a key only when it hopped. `src/press.rs` is a native button's `:hover` and `:active` (hover
  ignores keys, Space presses while focused) and `src/transition.rs` the CSS transition that
  `transition::keyed` keeps per element. gpui's div has no `aria-disabled`. Forgetting deleted tasks
  comes with 18.0; the `.xterm` and `.cm-editor` exclusion with their editors. Lucide icons at
  Cindy's sizes come from `IconName::svg()` through gpui `svg()`; `lucide::icon` also swaps in
  Cindy's `strokeWidth`. An `svg` takes no inherited color: set `text_color` on it.
- Reduced motion: `os_appearance::reduced_motion` reads the system setting Chromium reads for
  `prefers-reduced-motion` (Linux portal `org.gnome.desktop.interface enable-animations`, macOS
  `accessibilityDisplayShouldReduceMotion`, Windows `SPI_GETCLIENTAREAANIMATION`), at start and on
  every window activation. `motion::ReducedMotion` zeroes Cindy's motion tokens and
  `motion-reduce:` transitions through `motion::length`; Ely's `Theme.reduced_motion` stills its
  spinners. gpui's own `reduce_motion` stays off: Cindy keeps fixed-duration transitions (toasts).
  The macOS and Windows reads are type-checked in a scratch crate (`cargo check --target`); the
  full crate's C build scripts need those systems.
- Units and zoom: Cindy's sizes are CSS pixels, which Chromium's zoom scales. `units::css(n)` draws
  them as rems at 16 per rem, and each window's rem size is 16 × `windowZoom` (read and clamped from
  `appearance-settings.json`), as Ely's `ZoomControl` zooms. Layout math (`layout_root`, sidebar
  resize) runs in CSS pixels: viewport and pointer positions go through `units::to_css`; checks
  against painted bounds use `units::window_px`. Native window sizes (bounds, minimum size, traffic
  lights) stay in window pixels, as Electron's are. New UI code writes lengths with `css`, never
  `px`, unless the value is a painted bound or a native window measure. gpui's `border_*_1()`
  helpers are fixed pixels: write `border_b(css(1.0))`. `Appearance::shadow_token(id, cx)` scales
  shadow lengths by the zoom.
- Dock icon: `src/dock_icon.rs` ports `app-icon.ts`. `appIcon` from `appearance-settings.json`
  picks artwork under `resources/` (Cindy's files) for the active theme's light or dark, set
  through `NSApplication.setApplicationIconImage` at startup and on each theme apply. A bundle
  keeps its own icon for `default` until another replaced it. Other platforms do nothing, as
  Cindy. The full crate does not cross-compile from Linux; macOS code is checked in a scratch
  crate and by the macOS CI test `every_artwork_is_embedded`.
- Window material: `src/window_material.rs` ports `vibrancyConfig.ts` (`resolve`, with Cindy's
  test cases) and the glass CSS (`root`, `sidebar`, `overlay`, `cindy_theme`). `native.rs` ports
  Electron: macOS `setVibrancy` (an `NSVisualEffectView` behind the content), Windows 11 22H2
  `setBackgroundMaterial` (frame margins and `DWMWA_SYSTEMBACKDROP_TYPE`), with gpui painting the
  backing color. It applies when a window opens and on each theme apply. Wallpaper coverage
  (`covered`) is the wallpaper's `active`. Native code is cross-linted in a scratch crate.
- Wallpaper: `src/wallpaper.rs` normalizes the wallpaper keys of `appearance-settings.json` and reads
  `custom-wallpaper.json` (golden cases in `tests/fixtures/appearance`). `canvas.rs` paints
  `body::before` behind the main window: surface, cover image, veil; with a blur it blurs off the main
  thread at source resolution (`media::blurred`) and stretches it over the overscanned box. `clear`
  empties page surfaces while a wallpaper is active. Custom media is read only through
  `media::custom_path`, which keeps `assertBlobPathContained`'s checks. Logs name the kind of
  wallpaper, never the custom URL. An official wallpaper shows its bundled preview, then the original
  from Cindy's shared cache when `media::verified` matches the manifest
  (`resources/wallpaper-video-manifest.json`) in size and sha256. Downloads come with 13.0. `glass` is
  the fill of an in-page raised surface under a wallpaper (`--wallpaper-glass-opacity` with its floor),
  for Cindy's own themes.
- Wallpaper video: `wallpaper/video.rs` ports `WallpaperVideo.tsx` (`wanted` decides; `sync`
  opens, pauses or drops one decoder; `layer` paints it over the veil and drops retired frames).
  The decoder plays the bytes that were read and checked, never a path reopened later; errors
  log GStreamer's domain and code, not its text. Each platform decodes with its system
  framework: Linux GStreamer (`video/gst.rs`, from a sealed memfd, needs `gst-plugins-good` and
  `gst-libav` at run time); macOS AVFoundation (`video/avf.rs`, the checked bytes in a private temp
  file) and Windows Media Foundation (`video/mf.rs`, a memory stream) are `paced::Decoder`s:
  `paced::Player` runs one on its own thread, paces frames by their times, and restarts it to loop
  or refit. A hidden window pauses through gpui's `WindowVisibility`; Hyprland 0.56 sends no
  `suspended` state for a window on another workspace, so there it keeps playing. Decoder threads
  wake gpui from outside, so tests drive the real decoder only outside the gpui test scheduler.
- Open path: `src/file_path_policy.rs` is `filePathPolicy.ts`'s system blocklist and
  `isPathAllowedAgainst` on this host's paths (`canonical::resolve` stands for `path.resolve`);
  the sensitive-media list comes with `xdt-file://`. `src/open_path.rs` is the `shell:open-path`
  handler: `resolveShellOpenPathTarget` for paths (`cindy-media://` comes with its store), the
  blocklist, then Electron's `shell.openPath`. Linux starts `xdg-open` and reaps it unwaited, as
  Electron's Linux open never settles, so no error reaches a toast; Electron's XDG activation
  token is not passed. macOS calls `NSWorkspace openURL`; Windows `ShellExecuteEx` on a COM
  thread (Chromium's `SHParseDisplayName` id list, `explore` for a folder). The project menu's
  Open in file manager toasts the handler's error. Native code is type-checked in a scratch crate.
- Untrusted files: read with `bounded_read::read`, Cindy's `readBoundedFileNoFollow` (no-follow
  open, regular file, size limit, stable while read).
- Toasts: `src/toast.rs` ports `lib/toast.ts` as one `Toasts` global with a board per window, as
  each Cindy renderer keeps its own; timers run on gpui's executor clock. `toast/view.rs` draws
  `Toast.tsx` and `ToastContainer.tsx`; its `Slide` element does the FLIP in prepaint, so the
  slide starts in the frame the new tops are known. `motion::ease_out` is CSS `ease-out`.
  The container is deferred over every dialog, as its `z-[10100]` is.
- Confirm dialog: `src/confirm_dialog.rs` is `ConfirmDialog`'s `standard` presentation on Ely's
  `take_focus`, `give_back` and trapping `FocusScope`: a deferred scrim (`drag_region`, occluding,
  a press closes nothing), the centered panel (`maxWidth`, 85vh, the body scrolls between title
  and buttons), Escape through `intercept_keystrokes`. Radix's `Presence` is `Phase`: it fades in
  over `--motion-enter` and out over `--motion-exit` on the executor clock, inert while it fades
  out; gpui has no transform, so the panel rises without its 0.96 scale, and no backdrop blur.
  `confirm_dialog/button.rs` draws Cindy's `Button` (`lg`, `confirmation` palette) itself, since
  Ely's tones come from Ely's palette: the face on the border box, pressed in to the padding box
  by mouse or held Space (`press.rs`); the `transition.rs` transitions carry
  fill over `--motion-fast` (Oklab beside an `oklch()` token, else sRGB), the inset over
  `--motion-instant`. A disabled one keeps the arrow cursor, as `globals.css` does.
- Confirm host: `src/confirm.rs` is `ConfirmDialogProvider`'s `confirm`: a queue per window, one
  dialog at a time, the next 200ms after a close; `ask` answers on a oneshot, cancel unless
  confirmed. Only the options its callers use are ported.
- Cindy's `components/ui` primitives are drawn with Ely's (the toast action is Ely's secondary
  small `Button`); Cindy's `--text-N` is `appearance_settings::text_px`.
- Panel swap: `layout_root/view/panel_drag.rs` ports `PanelDragController.tsx`. Each pane and split
  root child records its painted bounds in prepaint (Cindy reads the DOM); a 600ms still press on
  a pane body starts the drag, Escape cancels through `intercept_keystrokes`, release swaps and
  toasts. Panel content that must not start a drag stops the mouse-down from bubbling.
- Box shadows come from tokens through `Appearance::shadow_token`; Cindy's motion tokens are in
  `motion.rs` (`FAST`, `motion_ease_out`).
- Channel tokens (`0 0% 64%`) are read through `Appearance::hsl_token`, as Tailwind's `hslVar`
  wraps them in `hsl()`; `token` takes full colors.
- View tests use gpui's `test-support` (a dev-dependency) and drive real mouse events
  (`src/layout_root/screen_tests.rs`). gpui's test scheduler fails a wake from another thread, so
  tests serve the database through `localdb::serve` (`Db::inline` runs each job where it is called). Live checks on Hyprland: `hl.dsp.cursor.move({ x, y })`,
  `hl.dsp.window.float/resize/move({ window = "address:..", x, y })`; a click needs a uinput device.
- userData is created at startup, as Electron does.
- Linux builds need `libxkbcommon-x11-dev` and `libfontconfig-dev`.
- Local builds run on huacnlee/cargo (`~/.local/bin/cargo`, ahead of the rustup proxy): build
  output lives in `~/.cargo/targets/` under a size cap, binaries in `./target`. Remove it to go back.
- Logging: `log` facade, written by `src/logger.rs` in Cindy's main-log format to
  `main-YYYY-MM-DD.log` (debug builds: `logs/` in the checkout; release: `<userData>/logs`).
  Level: `LOG_LEVEL`, then `log-config.json`, else `trace` in debug and `info` in release; other
  crates log at `warn` and above. A log's `target` is its Cindy scope.

## Commands

- `cargo run` opens the app.
- Review verdicts and gate logs go to `logs/review/<task>-r<N>.md` and `-{fmt,clippy,test}.log`.
  Every file a task makes stays inside this checkout. The sync scripts work in `.scratch/` (ignored).
- `scripts/sync-locales.sh <cindy checkout> <commit>` copies Cindy's locale files into `locales/`
  and records the commit in `locales/COMMIT`. Never edit `locales/` by hand.
- `node scripts/sync-themes.mjs <cindy checkout with node_modules> <commit>` runs Cindy's
  `colors.ts` and built-in themes through Cindy's esbuild and writes `themes/` (registry, built-ins,
  logos, `COMMIT`). Never edit `themes/` by hand.
- `node scripts/sync-golden.mjs <cindy checkout> <commit>` runs Cindy's own `convertVsCodeTheme`,
  `normalizeLocalThemeColors` and `layoutTree` on the inputs under `tests/fixtures/` and writes the
  expected outputs; the Rust ports must match them.
- `<cindy checkout>/node_modules/.bin/electron --no-sandbox scripts/blink-line-breaks.js` asks Blink
  where it breaks each pair of U+0021 to U+00FF and writes `resources/line-break-pairs.txt`.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. CI
  (`.github/workflows/ci.yml`) runs them and a release build on Linux, macOS and Windows,
  x86_64 and arm64. Linux x64, macOS arm64 and Windows x64 run on the org's self-hosted runners
  (`omarchy-amd64`, `macos-arm64`, `windows-amd64`); the rest on GitHub-hosted ones.
  CI runs only on manual dispatch (`gh workflow run CI`): day-to-day checks run on the local
  Omarchy machine; all six targets run once the port is finished.

## Rules

- Read Cindy's code and rule docs for an area before porting it. Cindy at the PRD's commit is the
  reference; do not guess its behavior.
- Keep Cindy's persisted formats and wire protocols. Change internal structure freely.
  `docs/storage.md` maps each store to its chapter; never migrate Cindy's database.
- Colors, sizes and radii come from Ely's `cx.theme()`. Reuse Ely components before writing new UI.
- Errors surface: no fallback that hides a failure. Log each key step.
- No file over 1000 lines (vendored upstream code apart). Comments only where needed, one short line.
- `TASKS.md` is progress truth; finished chapters move to `tasks/done.md`, their reviews to
  `tasks/reviews-done*.md`. One task, one review pass, one commit, one push.
- Update this file when the stack or direction changes.
