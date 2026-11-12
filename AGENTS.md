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
  (`setProjectHidden`). The other actions come with their tasks. `focus.rs` is `pendingProjectFocus.ts`
  with the sidebar's effect on it: once the list has loaded, the folder's key is matched among the
  drawn projects (`findProjectRepresentativeKey`), its project unfolds and `reveal::project` scrolls
  its node into view in the next frame drawn, else a warning toast says the project was not found.
  `src/cross_process_lock.rs` is the advisory tier of Cindy's `crossProcessLock.ts`: the `{pid,
  startedAt, nonce}` file, a 2 s heartbeat, nonce-bound release, and stale takeover under the
  `.reclaim.d` gate, with owner liveness from `kill(pid, 0)` and `ps` (PowerShell, then WMIC, on
  Windows). It blocks, so callers run it on the background executor. `filter.rs` reads and writes the
  filter preferences; `header.rs` draws the scope header and the filter menu on Ely's `Menu`
  (`menu_under`). Ely's menu has no row value, tip or kept-open choice, and lacks some lucide icons;
  the nearest Ely icon stands in. A right click on the list's blank space opens the same menu at the
  pointer (Ely `ContextMenu` around the scroller, `handleSidebarBlankContextMenu`); rows and headers
  stop their own right clicks. `fold.rs` is `SectionCollapse` and the first-N fold. Show all is keyed
  by section and group and resets once its list is no longer drawn, as each `SessionEntryList` holds
  its own. Animations read `background_executor().now()` so tests can advance the clock. `preview.rs`
  is `finalizePlainPreview` (its LRU cache, `stripInternalWebCitations`) and `projection.rs` the list's
  latest-message extract with `scheduleSessionListProjectionBackfill`: after a read, one `UPDATE` fills
  the empty `list_preview` and `list_message_count` columns. `view_mode.rs` is `useSidebarCardMode.ts`
  with `sidebarInstallVintage.ts`: the Pinned mode (`sidebar.cardMode`) and the main list's
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
- Router: `src/router.rs` is `router.tsx`'s hash routes (React Router 7.18's `matchRoutes`, the
  last chat view, `/` and `/cc-agent`). `docs/router.md` holds it with what hangs on it in the
  sidebar: `sessions/open.rs`, `keyboard.rs`, `select.rs`, `reveal.rs`, the conversation search
  (`search.rs` and `search/`) and its lock. Read it before touching any of them.
- Session view: `src/session_view.rs` is `CCAgentSessionView` for a local task (the gate, the
  header, the column, the stream, a user message's bubble, attachments, images, labels, cards,
  `flow.rs`'s line breaking, chips and links). `docs/session-view.md` holds it. Read it before
  touching any of them.
- Previews: `src/markdown_text.rs` is `markdownPreviewText` over the `markdown` crate's mdast (wooorm's
  markdown-rs, the mdast remark-parse and remark-gfm build). The crate is vendored in `vendor/markdown`
  (1.0.0, upstream otherwise) with patches marked `Cindy:` where it parts from micromark and
  mdast-util-from-markdown: ATX heading text, character classes as micromark reads UTF-16 code units,
  laziness at end of file, and those of the Markdown tree below. `src/rich_answer.rs` is
  `richAnswerText`: `cindy-ui` fences split from Markdown, `stream.rs` replays the ops once,
  `values.rs` resolves them, `js.rs` parses them as `JSON.parse` does (`Infinity` apart from `null`, no
  nesting limit: its recursion grows the stack through `stacker`, as V8 nests a million levels).
  Goldens in `tests/fixtures/preview` come from Cindy's functions; `sync-golden.mjs` stubs Electron,
  which `mapper.ts` imports.
- Markdown tree: `src/markdown.rs` is the parse of `MarkdownRenderer.tsx`'s `REMARK_PLUGINS`
  (remark-gfm with `singleTilde: false`, remark-cjk-friendly, remark-math), golden `markdown-
  tree`. `docs/markdown-tree.md` holds its parts (`flanks.rs`, `autolinks.rs`, spread) and each
  patch of the vendored crate; read it before touching the parse.
- Tree transformers: each of Cindy's remark plugins after the parse is a `transform` on the tree,
  golden `markdown-transforms` (`scripts/golden/markdownTransforms.mjs` runs each plugin of
  `REMARK_PLUGINS_PRIVILEGED` alone on the parsed tree; its cases are the inputs of Cindy's tests and
  seeded mixes). `markdown/cjk_urls.rs` is `remarkTruncateCjkUrls`: a literal autolink (one text, its
  value the address, at the link's own place) takes back the query's bracket closers, is cut by
  `url_boundary::clip` with the text before it, and its tail's web addresses become links.
  `html_images.rs` is `remarkHtmlImages` with `htmlImage.ts`: an html node that is one safe `<img>`
  becomes an image at its place; mdast's `data.hProperties` are `Image.width` and `height`, fields
  added to the vendored crate. `session_links.rs` is `remarkSessionLinks`: text outside a link is split
  at its valid deep links. A link a transformer makes has no place and no title. `local_images.rs` is
  `remarkBareSpaceLocalImages`: it takes the text `parse` read, slices each text node's source (a NUL
  counted as the U+FFFD it reads as) and, where `bare_space_images::find` (`bareSpaceLocalImages.ts`)
  finds an image, rebuilds the node; a span that differs from the node's value is read again by
  `markdown::plain`, remark-parse with no extension. For it the vendored crate starts a text at an
  escape's backslash, as mdast-util-from-markdown does. `path_links.rs` is `remarkLocalPathLinks`:
  `path_tokens::find` (`bareFilePathTokens.ts`; its lookarounds and backtracking order by hand, on
  characters) and a link with `bare_path`, the `data-bare-path` mark. `raw_destinations.rs` is
  `remarkPreserveRawLocalDestinations`: `Link.raw` and `Image.raw` hold a local destination as written.
  Golden `local-paths` checks the functions under them, on the inputs of Cindy's tests and seeded
  mixes: `local_path/shape.rs` (`looksLikeFilePath`, `looksLikeDirectoryPath`, `classifyMarkdownHref`,
  `resolveKnownLocalFileHref`, `normalizeXdtFileUrlForRenderer`) and `src/markdown_target.rs`,
  `markdownTarget.ts` whole (`link`, `inline_code`, `split_line`, `sandbox`, `ambiguous`,
  `remote_lit`). An `xdt-file://` link's `new URL` is the `url` crate, which reads the scheme as Node
  does where the golden runs; in Cindy's renderer the scheme is standard, so Chromium refuses hosts
  Node takes.
- Renderer tree: `src/markdown/hast.rs` is what react-markdown 10 hands `MarkdownRenderer.tsx`'s
  renderers, golden `markdown-hast` (`scripts/golden/markdownHast.mjs` calls react-markdown's own
  `Markdown` with Cindy's remark lists, `rehypeSlug`, `rehypeFencedCodeMarker` and each URL transform,
  and reads the tree its `post` step left; an esbuild load hook exports the renderer's private
  transforms and plugin list to the golden's bundle). `markdown::transformed` runs the transformers in
  `REMARK_PLUGINS`' order, the session links only when privileged. `hast/build.rs` is
  mdast-util-to-hast 13 (`wrap`, the text after a break, trim-lines, loose and task list items, a
  table's cells filled to its alignments, `normalizeUri`, remark-math's `code` classes; a reference or
  a footnote call with no definition panics, since micromark builds neither, as does a task item that
  does not start with its paragraph), `footer.rs` its footnote calls and section, `slug.rs`
  github-slugger over `resources/slug-classes.json`, which the sync reads off the slugger's regex.
  `hast.rs`'s `finish` is the rehype plugins and `post` in one walk: a heading's id, `dataFencedCode`
  on a `pre`'s `code`, html dropped (`skipHtml`), `href` and `src` through `urls.rs`
  (`trustedUrlTransform`, `previewSafeUrlTransform`, `allowlistedImageUrlTransform` over
  `defaultUrlTransform`). `remarkStrictInlineMath` and KaTeX come with 9.8, the highlighter with 9.7;
  the golden leaves them out.
- Image targets: `markdown/image_targets.rs` is `markdownImageTargets.ts`: `extract` walks the tree
  after the privileged transformers for image nodes, each address once, and `Cache` is the
  per-message cache the stream's mount holds (`items::Watch`). `render_items` reads it in
  `recordTurnInlineMedia`, so a result's media a turn's answer embeds draws once.
  `src/markdown_media.rs` is `isManagedMarkdownVideoUrl` and `src/remote_media_url.rs`
  `parseRemoteMediaUrl`'s wrapped address (`atob`, `TextDecoder`). Golden `image-targets`, from the
  hast golden's bundle; `normalizeMathDelimiters` joins with 9.8, so no case holds a math delimiter.
- Streaming Markdown: `src/streaming_markdown.rs` is `repairStreamingMarkdown.ts` (`repair`: an open
  fence closed, a half image or link read as its text, an open `**` or `*` on the last line closed)
  with `getStreamingMarkdownThrottleInterval` (`throttle`, the length in UTF-16 units), and
  `chunks.rs` `splitStreamingMarkdownChunks` (`split`: a chunk's `start` is its byte, the golden's a
  UTF-16 unit). JavaScript's `\s`, `.` and multiline `^` are spelled by hand. Golden
  `streaming-markdown`: `scripts/golden/streamingMarkdown.mjs` runs Cindy's functions on every
  prefix of Cindy's test inputs, crafted texts and 240 seeded mixes.
- Word break: `src/word_break.rs` is `Intl.Segmenter`'s word granularity as V8 answers in Cindy's
  renderer (`words`; `split` is `splitWordsUncached`). ICU4X's own segmenter follows UAX 29 and a
  longest-match dictionary, where Chromium's ICU 78 parts from both, so the port spells Chromium's
  `word.txt` by hand as pair rules over ICU4X's properties (a full stop is `$MidNum`, a colon no
  `$MidLetter`, Hangul syllables and Han, kana apart from letters) and `cjk.rs` ports
  `CjkBreakEngine::divideUpDictionaryRange` (the cheapest path over `cjdict`'s costs, read from
  ICU4X's baked data through `icu_provider`; the katakana run costs; NFKC with ICU's map back)
  with `populateDictionary`'s choice of engine per call (a Common-script kana mark goes to the
  unhandled engine and takes the Common characters after it). ICU's sources read for it are in
  `.scratch/9.6j2/`. ICU's Thai, Lao, Khmer and Burmese engines are not ported: ICU4X's dictionary
  segmenter cuts a run of those scripts, alike in about half of the probes
  (`logs/review/9.6j2a-complex.log`). A run that starts with U+30FC before the process met any Han
  or kana is cut as after. ICU4X 2.4 carries Unicode 18 where V8 carries 17: `resources/word-classes.json`
  holds V8's unassigned code points, which get no class, and its pictographs. Golden `word-break`:
  `ELECTRON_RUN_AS_NODE=1 <cindy checkout>/node_modules/.bin/electron scripts/v8-words.js` writes
  that table and V8's cuts of 25,679 texts (locale strings, seeded mixes of every class, CJK text spliced with
  characters NFKC changes).
- Word fade: `src/word_fade.rs` is `rehypeStreamWordFade.ts`: `Fade` is `WordFadeState` (its
  default `createWordFadeState`; `source` a chunk's own state by the chunk's start, on the owner's
  timeline; `candidate` and `whole_candidate` a render's copy, `commit` and `retain_only` its
  layout effect), `schedule` is `scheduleWordFadeSegment` and answers the delay its caller sets,
  and `fade` is the transformer on a renderer tree: a span per word and per inline code, a list
  item's marker on its first segment's key. The clock is its caller's, in milliseconds. A text
  holds no lone surrogate, so a pair cut by a chunk never reaches it. `cache.rs` holds
  `getOrCreateWordFadeState`'s 64 states (`States`) and `splitWords`' cache (500 texts, 2 MiB in
  UTF-16 units). Golden `word-fade`: `scripts/word-fade.js` runs Cindy's module in Cindy's
  Electron, for V8's words, on 36 seeded streams (chunks that grow, are edited, fall back to the
  whole document; renders given up; spans scheduled and settled on a stepped clock) and one of
  marked items come back emptied, and writes
  each run's tree and the state after it.
- Streaming throttle: `src/streaming_throttle.rs` is `useStreamingThrottle` as a state its caller
  drives: `render` is the effect (it runs when the text or the switch changed), `due` when the
  timer fires, `fire` its callback, `shown` the hook's answer; times are `performance.now()`'s
  milliseconds. Golden `streaming-throttle`: `scripts/golden/streamingThrottle.mjs` mounts
  Cindy's hook (through the hast golden's exporter) in jsdom, the clock and `window.setTimeout`
  set by hand, each timer fired at its own time. `src/stream_fade.rs` is
  `useStreamFadePreference.ts`'s read (`enabled`, `chat.streamFadePreference`, the memo in the
  `StreamFade` global; a stored default is removed); the setter comes with its setting.
- Streamed chunks: `src/streamed_chunks.rs` is `MarkdownRendererBase`'s streamed half with
  `StreamingMarkdownChunk`: `Chunks::render` repairs the text when it fades, splits it, and
  parses a chunk anew when its start (a UTF-16 offset, as Cindy keys it), text, `wholeDocument`
  or fade state changed, as `React.memo` and the chunk's `useMemo` do; a parsed chunk gets its
  candidate, the fade, then a place on the timeline for each key whose ref attaches (`mounted`:
  a span's, then its list item's after its children; none under an `a` whose renderer draws a
  card, a chip or the audio player in its children's place, `replaced`), then the commit and,
  for a whole document, the retain. A key is queued once, so a ref that attaches again changes
  nothing. The attached files are read at the parse alone. The chunks' trees stand end to end
  in one tree. gpui draws every frame, so `streamingMarkdownBlocks`' kept React subtrees have
  no counterpart. Golden `streamed-chunks`: `scripts/golden/streamedChunks.mjs` bundles Cindy's
  `StreamingMarkdownChunk` and `streamedChunksRun.mjs` mounts it in jsdom under Cindy's Electron
  as Node (V8's words), a row per chunk as the renderer maps them, with Cindy's own `a`
  renderer (`markdownHast.mjs`'s exporter lifts its source out of the component) under a
  `MemoryRouter`, on 36 seeded texts that grow on a stepped clock; each step holds the DOM's
  faded spans and the timeline. `session_view/streamed.rs` is a streaming message's renderer
  and `fade.rs` its words and markers on their way in (`docs/session-view.md`); a fading word
  is painted over the row's one layout (`markdown/inline/fading.rs`), never shaped alone.
- Markdown blocks: `session_view/assistant_row.rs` is `AssistantMessage` for a stored text: the bare
  bubble at the message width, its text through `src/goal_verdict.rs` (`stripGoalVerdictBlock`,
  golden `goal-verdict`), the tree parsed once per shown text (`Trees`, with the mounted content). A
  text with a rich answer draws nothing until 9.10; the labels, cards and action bar come with 9.11,
  `normalizeMathDelimiters` with 9.8. `session_view/markdown.rs` is `.msg-markdown`: it walks the
  renderer tree and lays the blocks out itself, since gpui has no margin collapsing, list markers or
  inline boxes. Blink's layout of Cindy's classes is measured in Cindy's Electron
  (`logs/review/9.6e-blink-*.log`; the probes are `.scratch/9.6e/probe*.js`). `margins.rs` collapses
  a column's margins: adjoining ones keep the larger, they pass through a container with no padding
  or border (a list, an item, a quote, the footnotes), a block with no height joins those about it,
  and the bubble, a flex item, keeps the first and last inside. A node's own `className` replaces
  the renderer's (`{...props}` comes after it), so a task list (`contains-task-list`) is a bare `ul`
  (no indent, gap or marker) and the footnotes' heading (`sr-only`) is not drawn. A run of inline
  nodes between blocks is a block of its own. `text.rs` reads it (`Inline`): the text with
  `white-space: normal` collapsed, a collapsed space carried through an element that holds none; the
  face each stretch has (`Mark`: `strong` 700 in `--md-strong-fg`, `em` italic, `del` struck, `a`
  underlined, inline `code`); a line feed per `br`; and the chips, a task's check (a 13px atom on
  the baseline, `marker::check`, Chromium's disabled light box: the renderer sets no `color-scheme`)
  and the pill of a code that holds no text. A line's end (`Inline::end`) drops the spaces before
  it, through empty codes (`logs/review/9.6f-blink-trailing.log`). Where a wrap drops a code's
  space, the code closes before it, and a code the space opened keeps its box on that row
  (`Look::stub`). A `sup` is Tailwind's preflight: 75% of
  its face's size on the row's baseline, lifted half an em, its line height none. An image is
  its own bullet. `inline.rs` draws the rows: each face is shaped in its own font, inline code
  (`text-14`, the code font, the block's line ratio) takes `px-[0.4em]` at its two ends, and
  `flow::rows_where` breaks the line: a word with no break overflows, as Cindy's does, and breaks
  only after a link's letters (`overflow-wrap: anywhere`; `logs/review/9.6f-blink-words.log`). A row
  of one plain stretch is a string; else its pieces stand side by side, code over its fill
  (`--msg-md-inline-code-bg`, `py-[0.2em]` about the font's height, 6px corners at the code's own
  ends). A row with code is Blink's line box (`extent`, `grown`: the leading's upper half floored;
  `logs/review/9.6f-blink-inline.log`), so it may be a fraction taller than its line. gpui lays a
  code piece out to the device pixel. `marker.rs` draws a
  list's markers as `list_marker.cc` places them: an item's marker waits for the first line under
  the item, at any depth, and hangs from that line's rows at the item's own edge, by its own
  baseline; an item with no line keeps it and is one line high, and an item with no block at all is
  a line the markers pending above it take too. A code block is `CodeBlockPre`'s surface over
  `pre-wrap` `break-all` rows in the code face, a tab to its row's next stop of four spaces
  (`text::code`, Blink's `TabSize`); what `break-all` leaves whole overflows; every fence draws so
  until 9.7 (highlight, copy, diff) and 9.9 (mermaid). `table.rs` is the GFM table in
  `CopyAsImageBlock`'s `my-3` (its toolbar comes with 9.16): `columns` is Blink 148's
  `DistributeInlineSizeToComputedInlineSizeAuto` for automatic columns in 64ths of a pixel, over
  each column's min and max content (`Look::sizes`; a cell breaks anywhere, so its min is its
  widest atom, a code's box with no text of its own counted with the atom before it, or at a
  line's start with the one after; no row starts at such a box, which hangs with the space before
  it) plus 25px of padding and collapsed border; the Chromium sources read for it are in
  `.scratch/9.6g/chromium`. The table is its container's width, or its least widths' sum when that
  is more, and then its wrapper scrolls it. Column edges snap to device pixels. A cell draws the
  borders after it and the table the first two; text is `text-14`, a `th` medium on
  `--msg-table-header-bg` and always left, a `td` by its `align`, both in the row's middle. A
  pending list marker hangs by the first row's content bottom, Blink's baseline for a row with no
  baseline cell (`logs/review/9.6g-blink-{tables,cells,nested}.log`). The scroller draws no
  scrollbar.
- Path resolver: `src/path_resolver.rs` is `main/pathResolver.ts`, the answer behind
  `fs:resolve-path`: `decode` (`safeDecodeHrefForLookup`), a leading `~`, the absolute stat, the
  join with the task's folder, then `walk.rs`'s breadth-first search for files whose path ends with
  the href (`find`, and `find_many` for a batch's one walk), with Cindy's ignored folders and the
  folders an href names itself. `resolve_all` is `resolveWorkspacePathBatch`. A folder that cannot
  be read is passed by, as Cindy passes it. `cache.rs` is the cache both entries share (30 seconds,
  200 answers, the oldest out first; a refresh or a negative answer asked again reads the folder
  anew) behind one mutex. The calls block: a caller runs them on the background executor. Golden
  `path-resolver`: `scripts/golden/pathResolver.mjs` builds each case's tree in `.scratch/` and runs
  Cindy's functions on it; the test builds the same tree. A folder's entries come in byte order on
  POSIX, as libuv's `scandir` sorts them (`logs/review/9.6h1-r1-electron-order.log`, Cindy's
  Electron main on Linux; not measured on macOS), and in the system's order on Windows.
  `src/node_path.rs` is Node 24's `path` (`join`, `normalize`, `resolve` from a given current
  folder, `basename`, `dirname`, `extname`, `isAbsolute`) for POSIX and Windows, with
  `isWindowsReservedName`'s `slice(0, -1)` and a Windows drive's own folder (`=C:`); golden `node-path` runs both halves in
  Cindy's Electron as Node, so Linux checks the Windows half's strings. No Windows folder is walked
  in a test.
- Smart paths: `src/local_path/smart.rs` is `localPathResolver.ts`'s lookups over the path resolver,
  in the `SmartPaths` global: `resolve_smart` (`resolveLocalPathSmart`, a task, past this cache),
  `peek`, `ask` (`resolveLocalPathSmartCached`) and `recheck` (`revalidateNegativeLocalPathSmart`;
  it answers whether a lookup is on its way). No promise is handed out: an answer lands in the cache
  (500, the oldest out first, no lifetime), `ask`'s `then` runs once the batch of the lookup it
  joined has settled, as a promise's callback, and the windows draw again. Asks of one update leave
  together in `cx.defer`, which stands for the microtask: one resolver call per folder on the
  background executor, rechecks apart. A lookup sent in a boundary's millisecond or before it is
  checked once more when it lands negative. `now` is `Date.now()` on the executor clock, in whole
  milliseconds as Cindy's stamps are. The resolver's allow check is `file_path_policy::allowed`, its
  home the user's. Cindy's main process answers its calls in the order sent; here two lookups in
  flight may land in either order. `src/markdown_target/resolved.rs` is `useResolvedMarkdownTarget`
  for a local task: the `Mounted` global holds a hook per target drawn (`begin` and `end` about the
  rows; a target not drawn unmounts). A hook reads the cache when its target, folder or streaming
  flag changes, asks on a miss, reads once more when the lookup it joined lands (a later run of the
  hook drops that callback, as the effect's cleanup does), and then keeps its state whatever the
  cache holds, as React state does. `lit` is `resolvedLocalFromResult`. A remote task's stat comes
  with its chapter. Golden `smart-paths`: `scripts/golden/smartPaths.mjs` runs Cindy's cache over
  Cindy's main resolver on real trees, each call held at a gate until the step that lands it, on a
  clock it sets; `lit.json` comes from the renderer's private function through the hast golden's
  bundle.
- Workspace media: `local_path::shape::image_src` is `normalizeMarkdownImageSrc` (golden `local-
  paths`). `src/workspace_media.rs` is `useWorkspaceMediaPath`, the address a relative media path
  of a local task loads: the `Mounted` global holds a hook per media drawn (`begin` and `end`
  about the rows) and `streamingMediaKeys`; `path` runs the hook's effects in React's order (the
  lookup on a new path or folder, then once more on a boundary just set: a streamed answer's end,
  or the window's `focus`, which `main_window`'s activation hands to `focused`) and answers the
  source, `pending` and the `revision` that grows when a missing path is found. `smart::join`
  waits on the lookup `recheck` said is on its way. Golden `workspace-media`:
  `scripts/golden/workspaceMedia.mjs` mounts Cindy's hook in jsdom over Cindy's cache and main
  resolver, calls gated as the smart paths' are, and renders the parent after each step, as gpui
  draws every row each frame; a hook Cindy's memoized renderer leaves alone keeps its last answer
  until it renders. After a boundary met a lookup in flight Cindy's promise settles at the second
  recheck and the port's waiter at the first, which shows only when the cache dropped the answer
  between them. Nothing draws with it before 9.6i3.
- Inline images: `session_view/inline_image.rs` is `LightboxImage` in a line, for the addresses
  the media stores read (`xdt-image://`, `cindy-media://`), a local file's (`xdt-file://`, below)
  a `data:` address and a web address (below; a remote task's media comes with its chapter and
  draws the unavailable card). `images.rs`'s `inline` reads a `data:` address with the `data-url` crate, the
  Fetch standard's reader, and decodes it as SVG when its media type is `image/svg+xml`, else by
  its bytes; `tests/fixtures/data-image/blink.json` holds what Blink's `<img>` loads of 45
  addresses, and the port loads the same ones at the same sizes. A failure is unavailable. `read` takes the tree's `img` nodes in document order (`Anchors.images`,
  mounted as `<key>-g<n>`): the raw local source or `src` through `workspace_media::path` (nothing
  while pending) and `image_src`; a managed video draws nothing before `ChatVideoView`'s chapter.
  The load is `images.rs`'s (`held`, `fetch`). Blink's boxes are in `logs/review/9.6i3a-blink-
  image.log` and `-cells.log` (probes in `.scratch/9.6i3/`). A loading or loaded image is one atom
  of its line (`Chip::Image`, `Swap::Image`) with 18px over and under it (`reach`): a loaded
  button's bottom stands on the baseline; the 280 by 180 frame's baseline is its centred spinner's
  bottom, 98px down. `fit` sizes a loaded one: the button is the natural width cut by `max-height`
  (or the `width` attribute), at most the line's width, and the image at its left is at most
  `min(50vw, 480px)` wide, so the button may be wider than its image; a column counts nothing for
  it at least and its own width at most. A failed load is `ImageMissingPlaceholder`, a block among
  the lines as the handoff card is (`Swap::Card`; `handoff::Metrics.margin` is 0 for it):
  `images::failed` in its block form, 280 wide where there is room, a pending list marker by its
  icon's bottom; a column counts its icon, its name, its title's widest character or Retry
  (`retry_width`, Ely's small button about its label) at least. Its metrics say 140 tall (more
  with Retry) at the bubble's 1.6 line ratio; a card squeezed until its title wraps draws taller
  than they say. `Load::Undecoded` is `readable`: the card offers Retry (Ely's small secondary
  `Button` in a 28px row), which starts a load for that image alone, kept while it shows that
  address (`Held.1`, `own`, `mounted`), as each `useImageLoadFailure` keeps its own state;
  `images::focused`, from the window's activation, drops every failed load, for the user's images
  too. No `online` event reaches the port. The click, the Tab stop, the menu and the lightbox come
  with 9.13; `status: 'loading'` with the streamed rows of 9.6j.
- Web images: the app's HTTP client is `gpui-pre-reqwest-client` at gpui's pin, set with
  `with_http_client` in `main.rs`; its user agent (`web_image::user_agent`) is Electron 42's with
  Cindy's product token and this crate's version. `src/web_image.rs` is the fetch Blink's `<img>`
  makes, as measured in Cindy's Electron against a loopback server
  (`logs/review/9.6i3c2-blink-http.log`, probe `.scratch/9.6i3c/http.js`, and the reviewer's
  `logs/review/9.6i3c2-r1-review-blink.log`). The address is made canonical by the `url` crate and
  refused with credentials or on a port of Chromium 148's `kRestrictedPorts`. `fetch` asks each hop
  itself with Blink's image `Accept` and no `Referer`, and reads each answer's fields as Chromium
  does, byte for byte (`logs/review/9.6i3c2-r3-blink-headers.log`, probe `.scratch/9.6i3c/headers.js`):
  two differing `Location` fields or `Content-Disposition` values fail any answer; one whose joined
  `Cross-Origin-Resource-Policy` is `same-origin` or `same-site` is refused; a 301, 302, 303, 307
  or 308 with a `Location` that is not empty goes on, its bytes past ASCII escaped, at most twenty
  times, and fails on a place that does not parse, another scheme, credentials or a restricted
  port; 204, 205 and 304 have no body; any other status gives its body, failed when empty. A
  `Content-Type` splits at commas outside quotes and the last value with a type decides: SVG only
  under `image/svg+xml`, any other body by its bytes. A
  client's error is logged by its root cause, which names no address. The port sends no cookie, no
  `Accept-Language`, no `Sec-Fetch` or client-hint header, asks for no compression and keeps no
  HTTP cache; a proxy comes from the environment, as reqwest reads it. A failure is unavailable
  with no Retry. The fetch's test runs the real client against a loopback server outside gpui's
  scheduler; view tests set gpui's `FakeHttpClient`. Logs name an address by its length.
- Local files: `src/local_file.rs` is `localFileProtocol.ts`'s handler for a `GET` with no `Range`
  (`read`: the file's bytes and its `Content-Type`, else the refusal's status): the `path` of the
  query as `URLSearchParams` decodes it, absolute, `path.resolve` equal to `path.normalize`, the
  extension whitelist, the sensitive-media list on the path and again on its real path, a regular
  file. `new URL` is the `url` crate, to which the scheme is not standard: Chromium refuses hosts
  it takes. A lost current folder refuses every path, where Node asks for it only when a path
  needs it. `file_path_policy/sensitive.rs` is `buildSensitiveMediaBlocklist` over a `Host`
  (platform, home, current folder, environment, `realpathSync`), golden `file-policy`:
  `scripts/golden/filePolicy.mjs` runs Cindy's function in Cindy's Electron as Node on crafted and
  seeded hosts of all three platforms, each with a file system of words the test builds again.
  `media_allowed` checks against the list built once from this host (`dirs::home_dir` for
  `os.homedir`, `canonicalize` without Windows' namespace for `realpathSync.native`). An inline
  image loads it through `images.rs`'s `local`: every failure is unavailable with no Retry, since
  `useImageLoadFailure` probes the stores alone; an `image/svg+xml` file is drawn by
  `src/svg_image.rs`, any other by its bytes: resvg at gpui's version (usvg's size rules, not
  Blink's) at two pixels per CSS pixel with the system's fonts, and an `image` inside it that names
  a file or a web address is not read, as Blink's image reads none (`data:` ones draw); gpui's own
  SVG renderer would read such a file past the sensitive list. `images::revised` is
  `WorkspaceMediaSrc`'s `key={revision}`: an image whose hook revision grew mounts anew, with a
  load of its own (`remount`, `reload`).
- Markdown links: `session_view/target_link.rs` is the `a` renderer with `MarkdownTargetLink` for a
  privileged message of a local task. Before the blocks are laid out `read` takes the tree's `a`
  nodes in document order (`Anchors`, each by its node's address): the raw local destination when
  the tree kept one, `markdown_target::link` with the files the task's user messages attached
  (`stream::attached`, `collectStableLocalFileRefs`), then the node's hook (`resolved::resolved`,
  mounted as `<key>-a<n>`; the stream calls `begin` and `end` about its rows). `Face` is what the
  node draws as: a span (plain text, a code reference, a bare path that has not resolved; the audio
  player comes with 9.13), a resolved file (below), a deep link's chip or words (below), a handoff
  card (its own bullet), a web address, or an inline target. A session or project link (`deep`) is
  `SessionLinkChip` or `ProjectLinkChip`, 9.5j's chip with its reads, clicks, keys and menu
  (`link_chip`; the label is the link's text, trimmed, unless empty or the address itself): the
  reader takes the `a` as one atom whatever it holds (`text::Swap::Chip`, `Chip::Link`),
  `Anchors.links` holds each chip's width, margin box and element, and its row grows about the
  chip's middle, half an x-height over the baseline, as a user message's row does. The label is
  `--text-12` (`text_size`), and an `em` about the link slants a session or project chip
  (`slanted`), not a message link's (`logs/review/9.6h5a-r1-review-blink.log`). A link that names no
  task or folder reads as its words, `label ?? href` (`Swap::Words`). A link is underlined and
  breaks anywhere; each stretch of its text on each row, and each empty code's box it holds, is a
  box (`dress`, named by row and part), the first its Tab stop, its authored `title` gpui's tooltip,
  all of them `links::listen`: the clicks, keys and menu `UserMessageUrlLink` has, on `What::Url`
  for a web address (the open-with menu) and `What::Target` for the rest (`useLinkContextMenu`: Open
  link, then Copy link with the `href` as the renderer handed it on). One menu per message hosts
  them (`links::host` under `<key>-md`). `open` is `openInlineTarget`: an anchor scrolls to its id,
  else the `anchorNotFound` toast; an authored local link asks `resolve_smart` past the cache, then
  toasts `duplicateFiles` or `fileUnavailable`; a found HTML file opens by the link preference
  (`file_menu::page`); what another found file and a stored image open comes with 9.13, so those
  clicks log and open nothing. `session_view/jump.rs` is the anchor's `getElementById` and
  `scrollIntoView({ behavior: 'smooth', block: 'start' })`: a heading, a footnote's item and an `a`
  with an `id` record where they paint (the first in the document keeps the id), and the stream's
  `ScrollHandle` moves along `src/smooth_scroll.rs`, Chromium's programmatic curve that
  `sessions/reveal.rs` shares; reduced motion jumps.
- Selection: `session_view/select.rs` is the browser's text selection over the stream, kept by
  the port (`docs/session-view.md`): each drawn run records its block, bytes and bounds, a press
  and a drag set the two ends, and each run lights its part in `--text-selection-bg`. A link
  chip's label and a handoff card's words are runs too, placed as a flex row's items. Blink's
  rules are measured under Cindy's built CSS: `.scratch/9.6k1/probe.mjs` builds
  `MarkdownRenderer` and Tailwind's output through a copy of `sync-golden.mjs`.
- Handoff card: `deep_link::parse_card` is `parseSessionCardHref` behind the renderer's
  `session-card/` prefix check (`new URL` is the `url` crate; golden `session-card`, from the
  renderer's private function through the hast golden's bundle); a link that parses as no card draws
  as any other link. `src/handoff_card.rs` holds the card's words (`displayTitle`, the badge,
  `formatLastActive` on `js_date` (an empty `last_active` is none, as the component's truthiness
  check has it), `shortSessionId` in UTF-16 units) and `session_view/handoff.rs` draws
  `SessionHandoffCard`. The button is a block inside its paragraph, so the inline reader reads a run
  in parts (`Inline::part`): text past card `part - 1` and up to card `part`, counting cards through
  `em`, `strong` and `del` (`Swap::Card`), and `Walk::runs` lays the parts and cards out as blocks
  whose margins collapse (`my-2`, through the paragraph or heading, an open container). A break
  before a card ends its line; one after it opens a line. Pending list markers hang by the card's
  first baseline, the bottom of the icon in its circle, 46px down (47 with the outlined badge, whose
  border makes the card 76px for 74). Blink's places are in `logs/review/9.6h5b-blink-card.log` and
  `-list.log` (probes in `.scratch/9.6h5b/`). The card keeps its own sizes and takes its second
  line's weight from a `strong` or an `h1` to `h3` about it (`target_link::around`), never an `em`'s
  slant. Its hover eases border, circle, icon and arrow over a fixed 150ms, which reduced motion
  leaves; the keyboard's focus draws `ring-2 ring-foreground` past Tailwind's white `ring-offset-2`.
  gpui has no letter spacing, so the badge's `tracking-[0.4px]` is a box per letter, as wide as it
  is shaped and 0.4px more, laid out to the device pixel. A click (any modifier) is `handleClick`,
  and the keys are a native button's (`press::clicks`), not gpui's key-up click: Enter clicks on key
  down and Space on key up, whatever they hold. A held Space ends with any other activation and with
  the focus leaving; a clean Space clicks through gpui's own key-up click, which gpui withholds when
  the focus moved meanwhile. gpui keeps that focus generation to itself, so a Space mixed with
  another key or a modifier does not see a focus gone and back within one update. The click runs
  `link_chip::act::task`, the chip's route read, with the route's `from` state: `router::Handoff`,
  the shown task's id and title, beside the location (`navigate_from`; a plain `navigate` carries
  none). `session_view/handoff_pill.rs` is `HandoffSourcePill` over the viewport while the route
  holds a handoff and the content has not dismissed it: Return (`act::back`, `resolveSessionRoute`
  then a plain navigate) and Dismiss, both `press::clicks` buttons with `ring-2`. In a table's cell
  the card is a block too (`Walk::parts`, which `runs` shares): the cell is a closed column of lines
  and cards, so `my-2` stays inside its padding and two cards share one margin; a `th` hands its
  medium weight on. The column counts the card's min and max content (`handoff::content`): the
  circle, arrow, gaps, padding and border, 122px, and the wider of its two lines, where the title
  and the last-active words count whole (`nowrap`) and the badge and the short id their widest
  character in the min, since the cell's `overflow-wrap: anywhere` reaches them
  (`logs/review/9.6h5b2-r1-review-blink.log`). The title's and the id's white space collapses.
- Resolved files: `target_link/file.rs` holds the faces of a resolved local target. An `a` whose
  label reads as a file (`code_label`, `shouldRenderCodeReferenceLabel`; golden
  `smart-paths/label.json` from the renderer's private function) is `FileTargetChip`, any other and
  every bare path `ResolvedLocalLink`; an inline `code` whose text resolves (`InlineCodeWithTarget`,
  its hook mounted as `<key>-i<n>`) is the chip too, its title the file's path. `read` hands the
  reader each such node's `FileFace` (`Inline::read`'s `files`): a chip opens a box in
  `Inline.files` (`Mark.file` names the innermost, so a chip may hold a chip) and an icon stands
  before the label as a chip atom (`FileTypeIcon` at 14 with `mr-1`, 0.125em under the baseline;
  none for a folder, none for a link with no label). `markdown/inline/file_box.rs` draws the box:
  text in it is the code face and underlined, `px-1` goes to its first and last atom, `break-all` to
  its bytes, and on each row the parts a box holds stand inside one fragment (`nest`) with `py-0.5`
  about the font's box, rounded where the box starts and ends (`logs/review/9.6h4a-blink-chip.log`).
  A fragment is the chip's `role="button"` (`Chip::fragment`): the first its Tab stop, its hover the
  lit fill over `--motion-fast` (the row under the pointer is kept, so a move between rows stays
  lit), its `title` gpui's tooltip. A chip inside a link is the link's box as a whole (`Held.link`,
  `link_of`): the link's listeners stand about the fragment, not on what it holds, so the chip keeps
  its clicks, right press, title and Tab stop, after the link's. The innermost target takes a press.
  `target_link/file_menu.rs` is `useFileChipContextMenu` for a local task, on the message's one menu
  (`What::File`; a message with chips and no `a` hosts it too). A right click, the menu key or
  Shift+F10 (under the boxes the target drew) opens its rows: for a page (`isBrowserOpenablePath`)
  Open in sidebar, Open in browser and View source; else Locate in file list (with a task folder)
  and Open with default app (a file); then Show in folder (`show_item`), Copy (`copy_media`, no
  page), Copy path and, where the reference named a line, Copy Location. `activate` is `onActivate`:
  a separate activation (Shift, Mod, Ctrl or the middle button) opens a page in the system browser
  (`browser_file`, a refusal's code the toast's key) and any other target through `open_path`; a
  plain one opens an HTML file as `openHtmlFileByPreference` does (`page`: the local link
  preference, the sidebar by default) and reaches `onOpen` for the rest, which opens nothing before
  9.13, as View source does. Every box of a target hears Enter and Space and lets them bubble, so a
  key on a chip inside a link activates both, as Cindy's keydown does; a click and the menu key stop
  at the chip. The sidebar browser comes with 17.0 and the sidebar file browser with 18.0, so those
  rows fail with Cindy's toasts. Ely's `Folder` and `Clipboard` stand in for lucide's `folder-tree`
  and `clipboard-copy`. `src/file_location.rs` is `formatFileLocation` over `toWorkdirRel`
  (`shared/workdirPath.ts`: a Windows folder compares without case and slices in UTF-16 units),
  golden `file-location` on the inputs of Cindy's test and `cases.json`.
- Dev profiles (`src/dev_profile.rs`, Cindy's `devCliFlags.ts`): debug builds stand for Cindy's
  source run. `--isolated[=<name>]` or `XDT_ISOLATED=1` (+ `XDT_ISOLATED_NAME`) runs in
  `<userData>-dev2[-<name>]`; `XDT_USER_DATA_DIR` picks a folder (a sandbox only with
  `XDT_USER_DATA_DIR_EPOCH=1`); `--isolated` on an official profile stops startup. Paths compare
  through `dev_profile::canonical::path`. Release builds always use the region folder.
- i18n: `src/i18n.rs` ports Cindy's i18next setup over `locales/`, embedded in the binary. Plurals and
  number formats come from ICU4X (`icu_plurals`, `icu_decimal`, `icu_locale_core`), as `Intl` gives
  them to Cindy. `Arg::Js` is a JavaScript Number: it picks its plural form and reads as `String(n)`,
  so a count past 2^53 reads as Cindy's does. Ely's `I18n` global is set to the nearest Ely locale for
  Ely's own text.
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
  `codeSize` (`clampAppearanceCodeSize`) is the `CodeSize` global, `--app-code-font-size`;
  `code_size(delta, cx)` is its `calc()`.
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
  the sensitive-media list is under Local files. `src/open_path.rs` is the `shell:open-path`
  handler: `resolveShellOpenPathTarget` for paths (`cindy-media://` comes with its store), the
  blocklist, then Electron's `shell.openPath`. Linux starts `xdg-open` and reaps it unwaited, as
  Electron's Linux open never settles, so no error reaches a toast; Electron's XDG activation
  token is not passed. macOS calls `NSWorkspace openURL`; Windows `ShellExecuteEx` on a COM
  thread (Chromium's `SHParseDisplayName` id list, `explore` for a folder). The project menu's
  Open in file manager toasts the handler's error. Native code is type-checked in a scratch crate.
- Show in folder and copy file: `src/show_item.rs` is the `shell:show-item-in-folder` handler for a
  file path (absolute, `path.normalize`, the blocklist, `fs.existsSync`), then Electron 42's
  `shell.showItemInFolder`, which answers nothing. Linux (`portal.rs`, `ShowItemHelper` on a thread
  of its own): the portal's `OpenURI.OpenDirectory` with the file's descriptor, heard out to its
  `Response`, when `org.freedesktop.portal.Desktop` runs or the bus can start it, else
  `FileManager1.ShowItems`, the choice made once; a failure opens the parent folder as Electron's
  `OpenFolder` does, `xdg-open .` from that folder's own parent. No activation token is passed.
  macOS (`finder.rs`) calls `selectFile:inFileViewerRootedAtPath:`; Windows (`shell.rs`)
  `SHOpenFolderAndSelectItems` on a COM thread, through `SHParseDisplayName`. `src/copy_media.rs`
  is `media:copy-to-clipboard` for a file path (no normalize). Windows and macOS run
  `fileClipboard.ts`'s PowerShell and `osascript` through `exec.rs`, Node's `execFile`: ten
  seconds then `SIGTERM`, 64K bytes of decoded output, `error.message` as Node words it. Linux
  writes a bitmap, as Cindy does: `bitmap.rs` is `nativeImage.createFromPath` (the bytes as a
  PNG, else a JPEG, a 16-bit sample cut to its high byte; a path through `..` made real;
  the `@Nx` files beside it by `base::FilePath`'s own string rules, the density nearest 1 from above) with
  Skia's premultiply and its PNG encoder's unpremultiply
  (`logs/review/9.6h4b-electron-unpremul.log`); colour profiles are not applied. gpui 0.3.11
  writes text alone to a Linux clipboard, so `arboard` writes the image (Wayland data control,
  else X11) and one `Clipboard` is kept, since the content lives in its writer. In tests both
  handlers record the path and touch no desktop. Native code is cross-linted in a scratch crate
  (`logs/review/9.6h4b-r1-native-*.log`).
- Open in browser: `src/browser_file.rs` is `openFileInBrowser.ts`, the
  `shell:open-file-in-browser` handler, for an absolute path: `resolveBrowserFileTarget`
  (`convert.rs` is Node 24's `pathToFileURL` for POSIX and Windows: `node_path`'s `resolve`,
  where a rooted Windows path with no device takes the current folder's drive, share or `\\?`,
  `EncodePathChars`, the kept last separator, then ada's part by hand, since the `url` crate
  drops a `file:` path's empty segments and guards a drive letter otherwise: the controls at the
  end cut, the path set escaped, dot segments folded (`segments`), and a UNC path's host as
  `set_hostname` reads it through `url::Host`, none when empty or `localhost`), the blocklist,
  `isBrowserOpenablePath`
  (`openable`), `fs.existsSync`, each refusal with its `BROWSER_FILE_*` code and Cindy's message.
  The URL goes to Electron's `shell.openExternal` (`open_external::hand`); when that fails the
  path goes to `shell.openPath` (`open_path::hand`), whose error is the answer. Golden
  `browser-file`: `scripts/golden/browserFile.mjs` runs Cindy's functions in Cindy's Electron as
  Node, once as it is and once with `path` and `pathToFileURL` set to their Windows forms, each
  Windows case in a current folder of its own. A
  `file://` URL is refused as no target until 17.0: the built-in browser sends one, and the `url`
  crate reads a `file:` URL's empty segments and `^` otherwise than Node's ada. In tests
  `open_path` records the path, as `open_external` does, and starts no program.
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
- `ELECTRON_RUN_AS_NODE=1 <cindy checkout>/node_modules/.bin/electron scripts/word-fade.js <cindy
  checkout> <commit>` writes `tests/fixtures/word-fade/expected.json`.
- `sync-golden.mjs` opens an offscreen Electron window for golden `stream-fade`, whose steps run on
  the real clock: it takes about a minute, and the fixture's times change with each run.
- `<cindy checkout>/node_modules/.bin/electron --no-sandbox scripts/blink-data-images.js` asks Blink
  which `data:` addresses an image loads and writes `tests/fixtures/data-image/blink.json`.
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
- No file over 1000 lines (vendored upstream code and the data under `tests/fixtures` apart).
  Comments only where needed, one short line.
- `TASKS.md` is progress truth; finished chapters move to `tasks/done.md`, their reviews to
  `tasks/reviews-done*.md`. One task, one review pass, one commit, one push.
- Update this file when the stack or direction changes.
