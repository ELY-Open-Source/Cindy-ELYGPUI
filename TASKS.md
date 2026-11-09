# Tasks

Progress truth, in `docs/PRD.md` chapter order. A task is ticked after review passes, then committed
and pushed. Reviews: `tasks/reviews.md`, at most three rounds per task. Chapters 1 to 7 are done,
listed in `tasks/done.md`.

## 8. Sessions

- [x] 8.0 Expand from Cindy's code, with the title bar's back and forward through the task browse
  history (`useSessionBrowseHistory`, `SessionHistoryButtons`); the sidebar top nav as
  `SidebarTopNav.tsx` draws it (New, Search, More and its customize view, order and top-level or
  More placement as `sidebarNavigationPrefs.ts` keeps them, rail tiles) and the Pinned and All
  sessions sections; the router as `router.tsx` declares it (hash history with back and forward,
  `MainEntryRedirect`, `CCAgentIndexRedirect`, `useActiveMainView`); the Automations row lands
  with chapter 15, the Plugins row and plugin entries
  with chapter 16, owner-scoped storage (`sidebarOwnerStorage.ts`) with chapter 13; the database
  migration's official shared-profile flag (from 7.1)
  - 8.0 plan, from `localDb/` (`index.ts`, `betterSqliteFactory.ts`, `migrationRunner.ts`,
    `schemaMigrationLease.ts`, `registerCjkSeg.ts`, `cjkSeg.ts`, `schema.ts`, `sessionQueries.ts`,
    `ipc/sessions.ts`), `appSessionState.ts`, `sidebarSettingsStore.ts`, `lib/sessionsStore.ts`,
    `lib/sessionService.ts`, `features/cc-agent/CCAgentSidebarUpper.tsx` and `sidebar/`,
    `components/sidebar/`, `router.tsx`. The session list's first caller is this chapter, so the
    database admission and opening move here from 13.0 for the owner that needs no sign-in: the
    account-free `local-v1` owner of `app-session.json`'s `local` mode (`cindy-local-v1.db`);
    `cloud` owners (`cindy-<userId>.db`) need chapter 13's account and open with it. The port never
    runs Cindy's 149 migrations, so a profile without that database file shows why (Cindy creates
    it) instead of a list
- [x] 8.1 The session owner, read only: `app-session.json`'s `activeMode` normalized as
  `appSessionState.ts` does, `local` giving the `local-v1` owner and its `owners/<key>/` folder,
  `<key>` the first 20 lowercase hex characters of the owner id's SHA-256 (`dataOwnerStorageKey`); `signed-out` and `cloud` wait for chapter 13 and say so
- [x] 8.2 The database for that owner, as `docs/storage.md` admits it: `<userData>/cindy-local-v1.db`,
  the reader lease, `checkMigrationCompatibility` against the reference commit's migrations, the
  pragmas `openWithPragmas` sets, `cjk_seg` and its TEMP triggers (`registerCjkSeg.ts`,
  `cjkSeg.ts`), file mode `0600`; queries off the main thread, as Cindy's worker does; a refused or
  missing database is shown, never repaired
- [x] 8.3 The list as `local-db:sessions:list` serves it: first, once per owner, the sweep of
  leftover provisional tasks (`sweepLeftoverProvisionalSessions` with `provisional-sessions.json`:
  hard-delete only unsent rows with no messages, keep other owners' entries, remove only an empty
  Cindy-managed dialogue folder; creating and marking provisional tasks lands with chapter 9);
  then `selectSessionListRows` (`updated_at DESC, id DESC`, `ordinaryTaskListFilter`; the
  `before {updatedAt, id}` keyset waits for a caller, since the sidebar store reads its first 1000),
  the `active`, `archived` and `all` buckets of `sessionsStore.ts`, and
  rows written by Cindy while both run, picked up as its push events would, shown
  in the main list's `flat` group-by mode (`sidebarFilterCore.ts`) with a row's title and time, so
  the data has its first reader here; the list projection backfill and pinned-summary backfill are
  Cindy's write-side caches and stay Cindy's
- The Pinned section and the project-grouped main list, as `CCAgentSidebarUpper.tsx` builds them:
  - [x] 8.4a The main list in its default `project` group-by mode: project keys as
    `projectKeys.ts` and `workingDir.ts` form them (`projectDir ?? workingDir`, storage
    normalization, worktree folders collapsed, `local:`, `remote:` and `device:` keys, Windows
    comparison keys), `groupSessions`' dialogues and unclassified tasks, display names
    (`project_aliases`, then unique basenames), `recent_workdirs` projects kept without tasks,
    the one ranking of `mainListModel.ts` under `sortBy` and `projectOrder` (`activity`, `custom`
    with `manualProjectOrder`), `ProjectNode.tsx` headers and indented rows, the dialogue group,
    the per-project limit (5) and top-level limit (20) with Show all; fold state persists with
    13.0's owner-scoped storage, as does `manualProjectOrder` (until then `custom` keeps
    the list's order), companion groups land with chapter 14
  - [x] 8.4b The Pinned section in its text mode as `PinnedSection.tsx` draws it: pinned tasks
    (active first, then `pinned_at`) and pinned projects, ordered by `sidebar-settings.json`'s
    `pinnedOrder`, its collapse preference, and the same file's `hiddenProjectKeys` (their tasks
    read as Chat tasks); the `flat` mode's dialogue group; card and list modes and the view style
    menu come with 8.5's card; a `pinnedOrder` Cindy has not yet moved out of its renderer
    storage is not readable here
  - [x] 8.4c The sidebar filter as `sidebarFilterCore.ts` keeps it and `MainListScopeHeader.tsx`
    and `SidebarFilterPopover` edit it: status, vendor, last activity, group by, dialogue grouping,
    sort and project order, with the harness and last-activity windows applied as
    `CCAgentSidebarUpper.tsx` applies them (pins exempt, `recent_workdirs` kinds); built on Ely's
    `Menu`, so rows carry no value summary or tip and a choice closes the menu; the machine
    switcher and host project orders land with chapter 19, the project and creator filters and
    the fold-all button with 13.0's owner-scoped storage, the display mode and task info with 8.5
- A session row, its actions and its card, as the sidebar shows them:
  - [x] 8.5a The row as `SessionItem.tsx` and `SessionInfoMeta` draw it in text mode: the
    status icon at rest (`SessionStatusIcon`: archive box, Orca lead, harness mark), title, the
    chosen task info fields (`cc-agent.sidebar.filter.taskInfo`: time, tokens, cost; PR and
    worktree pieces come with chapter 18), and the task info submenu of the filter menu. The
    right status (error, awaiting, running, done dots) goes to 10.0: its only database source,
    a pending interruption, needs the owning host's boot time (`getSessionInterruptionBootAt`)
  - [x] 8.5b1 The row's `SessionTaskMenu` (right click and the hover More button) with Rename,
    Mute notifications and Copy task link; rename by double click in `SessionRenameInput`;
    `title` and `notificationsMuted` written as `updateSessionInDb` patches them (no
    `updatedAt` bump; `toStoredSessionTitle` keeps the `[Schedule] ` prefix). The input's AI
    rename button needs a model and goes to 11.0. Archive, Unarchive, Delete and the two-step
    archive button go to 18.0: their status write takes the worktree locks and recycle records
    (`withStatusWriteLock`). Tags, Move, Share, Export and Open in new window go with the
    chapters that own them
  - [x] 8.5b2 Pin and Unpin as `handleTogglePin` does: `pinnedAt` written, the summary cleared
    on unpin, and a pin promoted in `pinnedOrder` through `sidebarSettingsStore.ts`'s
    `savePinnedOrder`, `override-settings-file.ts`'s `updateAtomic` and the advisory tier of
    `device-link/crossProcessLock.ts` (moved here from chapters 10 and 20, their first writer).
    The task summary a pin starts (`maybeGenerateSessionTaskSummary`) needs a model and goes
    to 11.0; the input-device task slots go with their chapter. Pins still in Cindy's renderer
    storage stay unread, as 8.4b records, so a pin here makes the scoped order authoritative
  - [x] 8.5c1 The task preview text and the list's projection backfill: `finalizePlainPreview`
    (`isSyntheticTriggerText`, `stripInternalWebCitations`, `markdownPreviewText` over remark's
    mdast, here the `markdown` crate) and `richAnswerText` (`packages/maker-shared/src/richAnswer.ts`:
    `cindy-ui` fences split from Markdown, the op stream replayed once, values resolved, each node
    written as text; the chat view's components stay with chapter 9). The list reads
    `latest_message_extract` and `latest_message_role` as `sessionReadSelection` does, and
    `scheduleSessionListProjectionBackfill` writes `SESSION_LIST_PROJECTION_BACKFILL_SQL` after
    it. Checked against Cindy's own functions through `sync-golden.mjs`
  - [x] 8.5c1b The `markdown` crate parses as micromark does where they part, vendored in
    `vendor/markdown` with the fixes only: an ATX heading keeps hashes before its first text
    (`# # heading` is `# heading`), characters are classed as micromark classes UTF-16 code units
    (JavaScript's `\s`; U+FEFF ends a GFM autolink, U+0085 does not; an emoji is not
    punctuation), and a lazy last line that starts an incomplete tag continues the paragraph
    (`>a\\` then `<a` is `a <a`). Golden cases from round 2 of 8.5c1's review
  - [x] 8.5c2a The display modes as `useSidebarCardMode.ts` keeps them: the Pinned section's
    `text`, `card` or `list` in `sidebar.cardMode` (old `true`/`false` read as card/text), the
    main list's `text` or `list` in `sidebar.mainListMode` (migrated from the pinned value, else
    `sidebarInstallVintage.ts`'s fixed `legacy` or `fresh`), chosen from `SidebarFilterPopover`'s
    display rows and `PinnedSection`'s `ViewStyleMenu`; Settings → Appearance with chapter 12,
    `setPinnedCardSummaries` with 11.0
  - [x] 8.5c2b The list row as `SessionCard.tsx` draws its `list` variant in the main list and the
    Pinned section's list mode (dividers, the title line with the info slot that gives way to
    More, one preview line), with `sessionToCamel`'s `preview`; the active ring with 8.6, live
    preview text with 10.0
  - [x] 8.5c2c The card as `SessionCard.tsx` draws its `card` variant in the Pinned section's card
    mode, laid out by `CardMasonry` (one column, two from 246px, three from 390px) and
    `DraggableCardColumns`' round-robin columns, with `sessionToCamel`'s `summary` and
    `resolveSessionCardBody`; dragging comes with 8.5c2e
  - [x] 8.5c2d `SidebarTitleMarquee`: an overflowing title scrolls once while its row is hovered,
    in the text and list rows
  - Drag to reorder pinned entries; project drag under the `custom` order writes
    `manualProjectOrder`, so it comes with 13.0:
    - [x] 8.5c2e1 The text and list modes as `PinnedSection`'s `SortableList` drags them (native
      drag: the dragged entry's ghost at 0.3, siblings making way in 150ms, a drop outside the
      list or a lost window saving nothing), saved as `handlePinnedReorder` does:
      `buildManualPinnedReorder`, then `savePinnedOrder`'s `reorder` rebased on the file
    - [x] 8.5c2e2 Card mode: one column as the list above, two or three as
      `DraggableCardColumns` lays a drag across its round-robin columns
- [x] 8.6 The router and the row's way into a task
    - [x] 8.6a The router as `router.tsx` declares it for the routes this chapter owns: `/` with
      `MainEntryRedirect` (to `/cc-agent` until 13.0's owner-scoped storage brings the remembered
      entry and `useRememberMainEntry`), `/cc-agent` with `CCAgentIndexRedirect` and
      `useTrackLastView`'s last chat view, `/cc-agent/new` and `/cc-agent/:sessionId` as the chat
      view's place until chapter 9
    - [x] 8.6b A row click to its session as `handleSessionClick` and `resolveSessionRoute` do
      (the priority hold and attention clears come with 10.0); the active task, from
      `/cc-agent/:sessionId`, drawn as `SessionItem`, `SessionCard`'s list and card variants and
      `SessionStatusIcon` draw it
    - [x] 8.6c The row a Tab stop (`tabIndex=0`) that Enter and Space open, with its task menu on
      the keyboard's context-menu key
    - [x] 8.6d Bulk selection: Shift and Mod clicks as `handleSessionClick` select (range over
      `getVisibleSidebarSessionIds`, anchor, toggle), the selected rows' look, and the bulk bar
      (`ccAgent.sidebar.bulkSelection`) with Clear; its Archive and Delete go to 18.0
    - [x] 8.6e The active row scrolled into view as `scrollIntoNearestView` does (`block:
      nearest`, smooth unless motion is reduced) when it becomes active
- [x] 8.7 The top nav as `SidebarTopNav.tsx` draws it: New, Search with its inline results over the
  list (`messages_fts` through `cjk_seg`), More with Customize (`SidebarNavigationCustomize.tsx`),
  order and visibility as `sidebarNavigationPrefs.ts` keeps them, rail tiles, with
  `useActiveMainView`'s active view and `navigateToView`
    - [x] 8.7a The New row (`handleNew` to `/cc-agent/new`, drawn as `ROW_CLASS`) in the
      sidebar's fixed top section (`section='pinned'`), off the rail and in the peek; the
      route state's computer comes with the New draft (chapter 9), `ROW_ACTIVE_CLASS` with the
      first row that can be active (15.0)
    - [x] 8.7b1 The rows `section='scrollable'` draws at the top of the list's scroller, in the
      saved order, and Search at rest and open as `SidebarInlineSearch` draws it (hover, focus
      within or a query opens the field; the clear button), its query shared with the results
      as `conversationSearchContext` shares it, and a press outside the search surface clearing
      a running query; a search from another view returns to the task view
      (`useActiveMainView`'s `navigateToView`) with the first other main view (16.0)
    - [x] 8.7b2 The search options as `SearchFilterMenu` offers them (sort, status, agent, last
      activity, reset, the active count), the sort remembered as `conversationSearchPrefs`
      keeps it
    - [x] 8.7c The search results in place of the list (`messages_fts` through `cjk_seg`, sorted
      as `conversationSearchSort` sorts), with the search row sticky over them and the clip
      below it while a query runs; the options' Projects submenu, which lists every known
      project, archived tasks' included
        - [x] 8.7c1a `conversationSearch.pure.ts`: title fuzzy match, keyword ranges, snippets,
          message text extraction and the sort, checked against golden `tests/fixtures/search`
        - [x] 8.7c1b `chatHistorySearch`'s FTS pass over `messages_fts` through `cjk_seg`; the
          vector pass is skipped as Cindy skips it while the embedding service is down (11.0)
        - [x] 8.7c1c `searchConversations`: titles and messages merged, filtered by the options
        - [x] 8.7c2 The results list in place of the task list, the search row sticky over it
            - [x] 8.7c2a `useConversationSearch`'s search: the request from the shared query and
              options (24 results, the unnamed label), the keyword pass after 250ms and the hybrid
              pass after 900ms on the database worker, only the latest request publishing, the
              local page merged as `mergeConversationSearchFanout` does (Cindy Make rows, its
              order), searching, done and error
            - [x] 8.7c2b1 `SearchResultsBody`, `SearchResultRow` and `SearchHitRow` in place of the
              list, which stays built and hidden: the searching, failed and no-result lines,
              title marks, the kind chip, the meta line, three hits then More, keyword marks and
              tips; the empty line lands with the rail popover, teammate hits with companions
            - [x] 8.7c2b2 The list's scroll under the search: top on each search, the list's
              place back once the query clears, after the pointer lifts
            - [x] 8.7c2c `handleSelect`: a result or hit opens its task and keeps the query; the
              message jump (`searchJump`) lands with the chat view (chapter 9)
            - [x] 8.7c2d The search row sticky at the scroller's top while a query runs
              (`pinSearch`), the rows after it following it, and the content scrolled under it
              clipped at its bottom edge, as `SidebarTopNav`'s clip-path effect does; the rows
              before Search (Automations, Plugins) come with 15.0 and 16.0
        - [x] 8.7c3 The options' Projects submenu, every known project, archived tasks' included
    - [x] 8.7d More (`SidebarNavigationMoreMenu` opened on hover by `useHoverOpenMenu`) with its
      hidden entries and Customize (`SidebarNavigationCustomize.tsx`) over
      `sidebarNavigationPrefs.ts`; without a data owner the defaults apply and nothing is saved,
      as Cindy does, until 13.0's owner-scoped storage
        - [x] 8.7d1 The More row after Search, opened on hover by `useHoverOpenMenu` (90ms open,
          160ms close, the gap to the menu kept, a click that only opens, no focus taken on
          hover) at its right, with the hidden entries (none under the defaults) and Customize
        - [x] 8.7d2 `SidebarNavigationCustomize` in place of the rows: the draft over the
          built-in entries (round checkbox rows, the grip's drag and arrow moves, Reset), Done
          storing nothing without a data owner, as `setSidebarNavigationPrefs(null)` does
    - [x] 8.7e The rail (`SidebarRailNavigation`): the tiles in the saved order, Search as
      `ConversationSearchBox`'s popover, and More
        - [x] 8.7e1 `CollapsedView` in place of the list while the sidebar is a rail and not
          peeking, both kept drawn and crossfading 200ms `ease-in-out`, the hidden one taking no
          pointer: the New tile (`SidebarIconButton`: 36px round, 18px icon, `titlebar-icon`,
          hover and press face, tip), the nav tiles in the saved order (Automations and Plugins
          with 15.0 and 16.0), More drawing nothing under the defaults (no hidden entries, no
          Customize on the rail), the divider; the Teammates tile with 14.0
        - [x] 8.7e2 The Search tile as `ConversationSearchBox`: its own `useConversationSearch`
          (query, filters, projects, run) searching only while open, the sort shared with the
          inline one as `subscribeSearchSortBy` syncs it; the popover at the tile's right (field,
          clear, the labelled options trigger, `SearchResultsBody` with the empty line in its
          scroller), focus on open; Escape, a press outside, focus leaving or the trigger close
          it and reset the query; a chosen result closes it; the inline search counts it as a
          popper
        - [x] 8.7e3 `RailNav`: the Pinned, Projects and Dialogues tiles with their hover panels
            - [x] 8.7e3a The Pinned tiles: every pinned task (archived ones and Orca workers left
              out) in `manualPinnedOrder`, each a round tile with the status glyph and
              `pinnedTileLabel` under it, the active one in the selected pill; a click opens it,
              a hover shows `SessionPreviewCard`, a drag reorders through
              `handleRailPinnedReorder`; the divider after them
            - [x] 8.7e3b The Projects and Dialogues tiles opening `railPanelStore`'s panels on
              hover or click; their lamps come with 10.0
            - [x] 8.7e3c `RailPanels`: the panels' content as `ExpandedView` renders it
                - [x] 8.7e3c1 `RailPanelShell` and the Dialogues panel (its head and count, the
                  dialogue rows as the expanded group sorts and folds them), with both panels'
                  closes (pointer keep-alive, Escape, a press outside, the rail ending, the
                  trigger hidden) and the keyboard contract (focus in, focus back)
                - [x] 8.7e3c2 The Projects panel
                    - [x] 8.7e3c2a Its list: the unclassified tasks (5, then Show all; the
                      project filter that hides them comes with 13.0), then every project with
                      its pinned ones as `visibleRailProjectsWithVendor` lists them and
                      `sortProjectsForSidebar` orders them (20, then Show all), each row with
                      its folder, name, count and chevron. The Cindy Make group never shows:
                      `CINDY_MAKE_ENABLED` is off, so the list query has no such tasks
                    - [x] 8.7e3c2b The project level: a row's hover or click opens its tasks
                      beside it, with its own grace, keep-alive and keyboard contract
- [x] 8.8 Interrupted tasks after a restart: the candidates as `interruptedResumeCandidates.ts` and
  `listInterruptedPendingRows` find them (the turn's last message and last tool use), and
  `InterruptedTasksResumeDialog.tsx` once per window and owner (load retries, selection, the
  still-interrupted recheck before continuing); the continue itself dispatches a turn, which
  lands with the agent runtime (10.0)
    - [x] 8.8a The candidates on the database worker: `listInterruptedPendingRows` with companion
      tasks (active, a task-list source or a `canonical`, `delegation`, `routine` or `channel`
      companion task, not hidden, started after its last end and its clear and before this
      process booted), kept when `botSessionInputBlockReason` passes and the companion is not
      in the owner's `companion-cloud/shadowed.json` (the live homes come with 14.0); then each
      turn's last message time and last `tool_use` (`parseToolUse`); loaded once per owner when
      the database is ready, 5 tries `2000 * attempt` ms apart
    - [x] 8.8b The activity line: `formatIslandToolDetail` over `describeToolUse` and
      `formatToolRowText` with `createLocalizedToolRowWording`, golden from Cindy's functions
        - [x] 8.8b1 `commandIntent.ts`: `commandIntentFromCommand` and `commandIntentFromActions`
          over `commandDisplay.ts`'s `normalizeDisplayCommand`
        - [x] 8.8b2 `describeToolUse` with `parseToolName`, `truncateToolText`,
          `humanizeToolToken` and `parseMessageToolUse`'s gateway unwrap; the descriptor keeps
          the fields the row label reads (details, paths, diffs and created paths come with
          the chat view, 9.0)
        - [x] 8.8b3 `formatToolRowText`, `formatIslandToolDetail` and the localized wording
          (`agentActionVerbKeys.ts`); the candidates' `activity` as the IPC makes it. The row
          text keeps the labels the island shows (details come with the chat view, 9.0)
    - [x] 8.8c `InterruptedTasksResumeDialog`: once per window and owner after the load, the
      tasks (`getMany`, companion names) newest first with their checkbox, vendor icon,
      title and source label or companion name, activity and time; select all; Continue
      rechecks the still-interrupted set, the dispatch with 10.0
        - [x] 8.8c1 The dialog's tasks in the same load: `getMany`'s rows for the candidates
          (`selectSessionsByIds`, missing rows dropped), companion names as `bots.list` gives
          them (`displayName` per linked task, newest profile first, the last one wins; a
          failed read gives none; default-bot provisioning comes with 14.0), newest first
        - [x] 8.8c2 `ConfirmDialog`'s `standard` presentation on Ely's primitives: the scrim
          that drags the window, the centered panel (`maxWidth`, 85vh, its scroll area),
          title, description, content, cancel first, the confirm's auto focus and disabled
          state; Escape and the buttons close it, a press on the scrim does not
        - [x] 8.8c3 The dialog itself: shown once per window and owner (closing marks the
          owner handled), its rows (checkbox, vendor icon, title, source label or companion
          name, activity, time), select all with its mixed state, the counts and the
          buttons; Continue closes it and rechecks the still-interrupted set; the send comes
          with 10.0, so each picked task fails as Cindy's failed send does (the
          `continueFailed` toast). The release notes that suspend it come with their chapter
- [x] 8.9 Back and forward through the session browse history (`sessionBrowseHistory.ts`, at most
  50 in memory, cleared on owner change, ⌘[ / ⌘]) with `SessionHistoryButtons`
- [x] 8.10 The project header's actions as `ProjectNode.tsx` draws them: the hover More and New
  in this folder buttons and its menu, by More or a right click (Rename, Search in this project
  with the search's locked project chip and the list's scroll freeze on that open, Show files, Open in the file manager, Copy deep link,
  Import, Remove from sidebar), and the dialogue group's Search; actions whose feature lands in a
  later chapter come with it; the rail panels' New buttons and their project row menu with them
  - [x] 8.10a The header as a Tab stop, its hover More and the menu by More, a right click or the
    menu key: Rename (double click too, `project_aliases`), Pin/Unpin, Copy deep link; New in this
    folder comes with the New draft (chapter 9)
  - [x] 8.10a2 Remove from sidebar: the awaitable confirm and `hiddenProjectKeys`' write
  - [x] 8.10b Search in this project with the locked project chip and the scroll freeze, and the
    dialogue group's Search
  - [x] 8.10c Open in the file manager: `filePathPolicy.ts`'s blocklist and an open that reports
    its error
- [x] 8.11 A right click on the list's blank space opens the filter menu at the pointer, as
  `handleSidebarBlankContextMenu` opens `SidebarFilterPopover`
## 9. Chat view

- [x] 9.0 Expand from Cindy's code
  - 9.0 plan, from `features/cc-agent/CCAgentSessionView.tsx`, `SessionContentHeader.tsx` and
    `NewMakerDraftRoute.tsx`; `lib/makerChatStore.ts` (the session slice, `enterView`,
    `loadOlderMessages`), `maker-shared`'s `historyView*.ts`, `messageWindow.ts`,
    `messageRender.ts`, `messageNormalize.ts`, `richAnswer.ts`, `permissionMode.ts` and
    `agentInputProjection.ts`; main's `localDb/ipc/messages.ts` with `historyViewReader.ts` and
    `historyViewOutline.ts`; `components/chat/` and `components/new-chat/`; `docs/rich-answers.md`,
    `design-rules/message-link-clickability.md` and `attachment-preview.md`. A local task reads
    `local-db:messages:view` (`createRemoteHistoryView` takes it whenever the preload offers
    `historyView`), never the flat list. This chapter draws what the database holds and every
    control the user touches; what needs a running agent (a send reaching the engine, stop, steer,
    a prompt's answer, live streaming) is drawn here and wired with 10.0, which says so in each
    case. Banners and chips owned by later chapters come with them: the provider, upgrade and
    spend banners and the context ring with 11.0 and 13.0, teammates and bot views with 14.0,
    schedule banners with 15.0, plugin prompts and Ghost cards with 16.0, MCP App cards with 17.0,
    worktree and turn-change actions with 18.0, remote and shared-task views with 19.0 and 20.0,
    read aloud and voice with 22.0; the Cindy Make gates stay off (`CINDY_MAKE_ENABLED`). Mermaid,
    KaTeX and highlight.js have no gpui counterpart: their tasks choose the renderer from evidence
    that it matches Cindy's output, and record the choice here
- [ ] 9.1 The history read as `local-db:messages:view` and `local-db:messages:work-details` serve
  it: `historyViewReader.ts`'s pages (`HISTORY_VIEW_PAGE_BYTES`, `HISTORY_VIEW_PAGE_ITEMS`, the
  scan limits), `historyViewOutline.ts`'s outline rows, `projectHistoryView`'s items and work
  summaries, the cleared and rewound cuts, `restoreTaskImageRows`, rows read as `messageToCamel`
  reads them; on the database worker, checked against goldens from `sync-golden.mjs`
  - [x] 9.1a The row checks `projectHistoryView` reads: `messageContentToPreview`,
    `parseMessageToolUse` in full, `isOrcaCommunicationTool`, `isAgentPlanToolName`,
    `isAgentTaskToolName`, `isDeliveryProseText` (`hasRichAnswer`), `isSteerUserRow`,
    `isContinuationMessage`, `parseToolResultPayload` with `extractPayloadToolResultMedia`,
    `extractPayloadToolResultFiles` and `extractPayloadToolCardIds`, and `describeToolUse`'s
    `mcp` kind with `createdPath`; golden from Cindy's functions
  - [ ] 9.1b `groupWorkRuns` (`workRunGrouping.ts`), `historySubagentScopes`, `projectHistoryView`
    with and without lazy details, `completesHistoryTurn`; golden from Cindy's functions
  - [ ] 9.1c The reader: `readMessagesList`'s rows (cleared and rewound cuts, cursors) and
    `historyViewOutline.ts`'s outline, `historyViewReader.ts`'s `page` (page boundaries, safe
    starts, the item and byte targets, scan budgets, the sealed-answer context) and `details`, on
    the database worker. The first page's history import comes with 10.0, `restoreTaskImageRows`
    and `recoverGeneratedImagesForHistory` (both write the media store) with 9.13
- [ ] 9.2 The session slice as `makerChatStore.ts` keeps it for a local task: the first page on
  `enterView`, older pages (`loadOlderMessages`), work details on demand, the reload on a database
  change (the watcher of 8.3), the soft eviction of idle sessions and the window trim;
  `mapServerMessages`
- [ ] 9.3 The route `/cc-agent/:sessionId` as `CCAgentSessionView.tsx` lays it out: the content
  header (`SessionContentHeader.tsx`: title and its rename, project chip, vendor, the actions this
  chapter owns), the message column with its width rules, the composer dock; loading, empty, not
  found and archived states; `useTrackLastView` keeps the slot
- [ ] 9.4 Render items: `buildRenderItems` and `messageWorkGroups.ts` (messages, tool segments,
  work groups, agent plans and tasks, fork origin, turn changes, generated files, tool media),
  subagent rows left out, `collectTurnFinalAssistantClientIds`; golden from Cindy's functions
- [ ] 9.5 The user message (`UserMessage.tsx`): `userMessageDisplayText.ts`, linkify and URL links
  (`message-link-clickability.md`), the collapse (`userMessageCollapse.ts`), attachment chips,
  quote, inline reference and session link chips, the forwarded-request text of 8.7
- [ ] 9.6 Markdown (`MarkdownRenderer.tsx`) on the vendored mdast: GFM tables, lists, task
  lists, block quotes, links and images (`remarkLocalPathLinks`, `remarkSessionLinks`,
  `remarkHtmlImages`, `remarkTruncateCjkUrls`, `remark-cjk-friendly`), headings with slugs, the
  copy and selection behaviour; `repairStreamingMarkdown` and the streaming chunks
  (`streamingMarkdownChunks.ts`) for rows still being written
- [ ] 9.7 Code blocks: `rehype-highlight`'s highlight.js grammars and classes mapped to the
  theme's syntax tokens, the language label and copy, `DiffView.tsx` and `MarkdownDiffBlock.tsx`
- [ ] 9.8 Math: `remark-math` with `remarkStrictInlineMath` and `math-markdown`, drawn as KaTeX
  draws it, inline and block (`rehypeMathBlockMarker`)
- [ ] 9.9 Diagrams: `MarkdownMermaidBlock.tsx` with `mermaid-autofix`, the source toggle, error
  state and `MermaidLightbox.tsx`
- [ ] 9.10 Rich answers (`RichAnswer.tsx`, `docs/rich-answers.md`): the `cindy-ui` component tree
  from 8.7's `stream.rs` and `values.rs` drawn on Ely (layout, text, stat, chart, svg, media,
  controls, tabs, details), local control state per message, `send` buttons queued for 10.0
- [ ] 9.11 The assistant turn: `AssistantMessage.tsx`, `ThinkingCard.tsx` and `ThinkingText.tsx`,
  `AgentActionRow.tsx` and `AgentActionsBlock.tsx` with the full `describeToolUse` (details,
  paths, diffs, created paths, from 8.8b2), `WorkGroupBlock.tsx` folded and expanded,
  `ToolCallCard.tsx`, `ToolPayloadLightbox.tsx`, `TodoListCard.tsx`, `AgentTaskCard.tsx`
- [ ] 9.12 System rows: `SystemCard.tsx` (agent switch, compaction, notices), `ErrorMessageCard.tsx`
  with `errorReasonI18n.ts` and `outputLimitI18n.ts`, `HistoryClearedMarker`, `ForkOriginMarker`,
  `InterruptedTurnBanner.tsx`
- [ ] 9.13 Media and artifacts: `ChatImageView.tsx`, `ChatVideoView.tsx`, `ChatAudioCard.tsx`,
  `GeneratedFilesCard.tsx` (`generatedFilesProjection.ts`), `AttachmentCollection.tsx`, the image,
  video, text and file lightboxes, missing-media placeholders; files through `cindy-media://` and
  the open path of 8.10c (`attachment-preview.md`)
- [ ] 9.14 The viewport (`MessageViewport.tsx`, `useMessageViewport.ts`): the render window
  (`RENDER_WINDOW_INITIAL_ITEMS`, first-paint budget), scroll anchoring and restore per session,
  auto follow, older pages on scroll, `JumpToBottomChip`, `NewMessageIndicator`,
  `PrevMessageJumpChip`, `MessageNavRail.tsx`, navigation keys; the search hit's `searchJump`
  lands here (from 8.7, `searchJumpTargeting.ts`)
- [ ] 9.15 The composer (`ChatInput.tsx`, `composerDocState.ts`): the document as Tiptap keeps it
  (paragraphs, hard breaks, list and quote nodes, mention, pasted-text and slash chips), history,
  placeholder, IME and CJK punctuation, the paste pipeline, `SlashCommandPalette.tsx`,
  `AtMentionPanel.tsx`, drafts as `composerDraftStore.ts` keeps them; the New draft route
  (`NewMakerDraftRoute.tsx`) with its folder and computer, the project header's New in this
  folder (from 8.10a) and provisional task creation (from 8.3)
- [ ] 9.16 Message actions (`MessageActionBar.tsx`): copy, copy as image, the selection quote
  button (`SelectionQuoteButton.tsx`, `appendQuoteToDraft` into 9.15's draft), delete
  (`useDeleteMessage.ts`) and fork (`useForkAtMessage.ts`) on the database; edit last and rewind
  preview drawn, resent with 10.0
- [ ] 9.17 Attachments: `ComposerAttachments.tsx`, the local picker, drop (`composerDrop.ts`,
  `globalDropIntercept.ts`), pasted images and paths, staging and its cleanup
  (`chatAttachmentStageCleanup.ts`), `attachment-preview.md`
- [ ] 9.18 Composer controls: `PermissionSelector.tsx` with `permissionMode.ts` and
  `FullAccessConfirmContent.tsx`, plan mode, effort, fast and thinking toggles, the agent select,
  the send button's states (`SendButton.tsx`), the model selector's face (its catalog with 11.0);
  the `ask_user` and `plan_review` rows as history cards (`AskUserQuestionBubble.tsx`,
  `PlanReviewBubble.tsx`, the plan cards). Pending interactions live only in the runtime
  (`getPendingInteractionsForSession` reads `pendingInteractionResolvers`; a permission makes no
  row), so `PermissionPrompt.tsx`, `AskUserQuestionPrompt.tsx` and the plan action panel are drawn
  here against fixtures, and their input, snapshot rebuild and answers come with 10.0
- [ ] 9.19 Queue and interjection, stop and continue: `PendingQueuePanel.tsx` over
  `agentInputQueueSnapshots.ts` and `agentInputProjection.ts` (rows, edit, reorder, steer), the
  stop face, the interrupted-turn continue; drawn from the database, sent with 10.0
- [ ] 9.20 Tab groups (from 6.7): `right-tabs` and the edge groups, as `tabGroups.ts` and
  `ensureTabGroupPane` place them; edge groups parked on non-chat routes other than settings; an edge group's temporary
  maximize and restore without changing the tree (`panelMaximize.tsx`, `LayoutRoot.tsx`; its only
  trigger is `TabGroupPanel.tsx`); the pane placement signals panels read (`panePlacement.tsx`:
  at window top, at window left, fill); the panel registry as `panels/registry.ts` keeps it
  (built-in kinds with their collapse memory, tab group kinds registered, unregistered and
  parked); `right-tabs` collapse per session under `right-sidebar-collapsed:<sessionId>`; the window
  grows and shrinks as the right group opens and closes (`sidebarPresentation.ts`, `sidebarExpansion.ts`, anchored
  widths)
- [ ] 9.21 Detached windows (from 6.10): bounds in `right-sidebar-window-state.json` and the shared
  `sidebar-tab-window-state.json`, restored without maximize or fullscreen; which tabs are detached
  lives only for the process, so restart or crash returns them to the main window; legacy
  `right-sidebar-window-settings.json` deleted; detach, merge back, close and multi-window cascade as
  `sidebar-tab-window/controller.ts` does; `swallowActivationClick` for these windows too

## 10. Agent runtime

- [ ] 10.0 Expand from Cindy's code, with the agent and session `ndjson` log streams and the
  interrupted-task continue from 8.8, and the task row's running icon, draft pencil and right
  status (`SidebarRightStatusIndicator`, from 8.5a)

## 11. Models and providers

- [ ] 11.0 Expand from Cindy's code, with the rename input's AI rename button (from 8.5b1), the task summary a pin starts (from 8.5b2) and `setPinnedCardSummaries` (from 8.5c2a)

## 12. Settings

- [ ] 12.0 Expand from Cindy's code, with the settings route and its takeover (sidebar, chrome
  cluster, peek and non-chat panels hidden without changing the tree, restored on exit), the title
  bar's menu button (`MenuButton.tsx`), Obsidian theme conversion and the theme import button,
  and shortcut rebinding (`AppShortcutStore` writes, `findAppShortcutConflict`, bindable and
  global checks, Electron accelerators, the recording flag)

## 13. Account and region

- [ ] 13.0 Expand from Cindy's code, with official wallpaper downloads (`wallpaper-video.ts`, from
  6.9e2), the mock mode's gate, account-free start and seed (from 7.0), the dev keychain identity
  marker, `cloud` owners' database (8.2 admits only `local-v1`), isolated device id and `--endpoints-cdn` (from 7.1), the sidebar's owner-scoped storage and its legacy claims
  (with `manualProjectOrder` and project drag from 8.5c2e),
  log upload and redaction, and the database admission in `docs/storage.md` (reader lease,
  read-only migration check, `cjk_seg`)

## 14. Teammates

- [ ] 14.0 Expand from Cindy's code, with the sidebar's Teammates section

## 15. Automations

- [ ] 15.0 Expand from Cindy's code, with the sidebar's Automations row and the dev `--passive`
  scheduler with its shared-profile flag (from 7.1)

## 16. Plugins

- [ ] 16.0 Expand from Cindy's code, with the sidebar's Plugins row, plugin entries and arrivals

## 17. Tools and integrations

- [ ] 17.0 Expand from Cindy's code

## 18. Files and git

- [ ] 18.0 Expand from Cindy's code, with the task row's Archive, Unarchive and Delete and the
  two-step archive button (from 8.5b1), and the bulk bar's Archive and Delete (from 8.6d)

## 19. Remote

- [ ] 19.0 Expand from Cindy's code

## 20. Device link

- [ ] 20.0 Expand from Cindy's code

## 21. IM channels

- [ ] 21.0 Expand from Cindy's code, with the passive dev profile's IM announcement gate (from 7.1)

## 22. Desktop extras

- [ ] 22.0 Expand from Cindy's code, with the Windows and Linux first-close prompt and close behavior
  (`mainWindowCloseBehavior.ts`, `windowBehavior.ts`), which hide to the tray

## 23. Distribution

- [ ] 23.0 Expand from Cindy's code, with the local database refusal screen's update views and
  Check for Updates button (from 8.2), the Linux packages' GStreamer runtime dependencies
  (`gst-plugins-good`, `gst-libav`) for wallpaper video
- [ ] 23.last Dispatch CI and get all six targets green; run the app on the Windows runner and check
  that maximized windows survive display and DPI changes without Cindy's maximize recovery
