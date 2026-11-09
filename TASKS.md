# Tasks

Progress truth, in `docs/PRD.md` chapter order. A task is ticked after review passes, then committed
and pushed. Reviews: `tasks/reviews.md`, at most three rounds per task.

## 1. Foundation

- [x] 1.1 Crate, Ely window, env logging, license, notice, `AGENTS.md`, PRD, this file; public repo
  `ELY-Open-Source/Cindy-ELYGPUI`, first push
- [x] 1.2 CI: fmt, clippy, test and build on Linux, macOS, Windows, x86_64 and arm64
- [x] 1.3 CI green on all six targets
- [x] 1.4 CI on self-hosted runners where the org has one: Linux x64 `omarchy-amd64`, macOS arm64
  `macos-arm64`, Windows x64 `windows-amd64` (any idle one of that label)
- [x] 1.5 CI on manual dispatch only; local checks on Omarchy until the end
- [x] 1.6 Reference moves to Cindy `1ac47f3`, which adds the Omarchy live theme; locales re-synced

## 2. Logging and diagnostics

- [x] 2.0 Expand from Cindy's code (`main/logger.ts`, `shared/mainLogRecordFormat.ts`,
  `main/regionUserData.ts`, `maker-shared/brandIdentity.ts`)
- [x] 2.1 Region from `CINDY_AUTH_REGION` at build (`cn`, `global`, `dev`; default `global`; other
  values fail the build) and the userData folder `Cindy`, `CindyGlobal` or `CindyDev` under the OS
  app-data folder, shared with Cindy
- [x] 2.2 Main log: `main-YYYY-MM-DD.log` in `<userData>/logs` (debug builds: `logs/` in the
  checkout), Cindy's line format and continuation escaping, format sentinel on each file open, level
  from `LOG_LEVEL` then `log-config.json`, daily switch, 30-day retention, terminal mirror in debug
  builds
- Agent and session `ndjson` streams move to chapter 10; log upload and redaction to chapter 13.

## 3. Configuration and storage

- [x] 3.0 Expand from Cindy's code into `docs/storage.md`: every store, its Cindy owner, its chapter.
  Store code lands with its first caller, so the database admission moves to chapter 13.

## 4. i18n

- [x] 4.0 Expand from Cindy's code (`shared/locale.ts`, `renderer/i18n/index.ts`, `locales/`)
- [x] 4.1 Cindy's locale files at the PRD's commit in `locales/`, copied by
  `scripts/sync-locales.sh`; 4.3 compiles them in
- [x] 4.2 Locale choice: Cindy's 12 locales, OS locale matched as `resolvePreferredSystemLocale`
  does (zh by script then region, pt by region), the saved `language` preference read from the
  port's preferences file; writing it lands with the language setting (chapter 12)
- [x] 4.3 Translator with i18next's semantics as Cindy uses them: namespaces `common` (default) and
  `aiRename`; nested keys; `{{name}}` with defaults `appName`, `pronoun` and
  `reservedGhostIdPrefixes`; plural suffixes picked by each locale's CLDR category (`_one`, `_many`,
  `_other` and any other the catalogs hold); `$t()` nesting; fallback classical to modern Chinese to
  English; a missing `ipcError.*` or `chat.remoteError.*` key (not `ipcError.INTERNAL`) shows
  `ipcError.INTERNAL`; `DEV_WRITE_BLOCKED`; Ely's `I18n` set to the nearest Ely locale for its
  components

## 5. Themes and appearance

- [x] 5.0 Expand from Cindy's code (`renderer/themes/`, `hooks/useTheme.ts`, `hooks/useFontSettings.ts`,
  `main/appearance-settings-store.ts`, `main/local-themes/`, `shared/theme-import/`)
- [x] 5.1 Cindy's 576 color tokens (566 direct registrations plus the `effort-tier-*` and
  `price-tier-*` loops) and 11 built-in themes at the PRD's commit in `themes/`, written as JSON by
  `scripts/sync-themes.mjs`, which runs Cindy's own `colors.ts` and `builtin/` and reads the registry
- [x] 5.2 Token resolution as `theme-service.ts` does it (theme value, else registry default for the
  theme's type, else for a dark theme the light default; `var(--x)` aliases, `-hsl` triplets); built-in families as `families.ts`; `theme`
  (light, dark, system) and `theme.familyId` read from the port's preferences, with Cindy's
  single-variant fallback; the system mode follows the OS
- [x] 5.3 Cindy tokens drive the window: Ely's `Theme::set_palette` gets a palette mapped from the
  resolved tokens for each mode, and the resolved tokens stay a global for Cindy's own surfaces
- [x] 5.4 `shared/theme-import/` color, palette, protected-token and VS Code conversion as pure
  functions, with the cases of Cindy's tests for them (moved ahead of local themes, which normalize
  through it; Obsidian conversion goes to chapter 12 with the import button, its only caller)
- [x] 5.5 Local themes from `~/.cindy/themes/` as `main/local-themes/loader.ts` reads them (legacy
  `~/.xdmaker/themes` move, `-local` ids, `family` grouping, `normalizeLocalThemeColors`, diagnostics)
- [x] 5.6 The Omarchy live theme as `main/local-themes/omarchy.ts` does it: family `omarchy` from the
  current Omarchy theme through 5.4's VS Code converter, retinted on each switch
- [x] 5.7 `appearance-settings.json` read as `main/appearance-settings-store.ts` does, and its UI size
  and UI and code families applied to Ely's font scale and families as `applyFontSettings` does;
  code size goes with code blocks (9), terminal fonts with the terminal (18), zoom, wallpaper and
  app icon with the window shell (6)

## 6. Window shell and layout

- [x] 6.0 Expand from Cindy's code (`main/bootstrap-electron.ts` window creation, `shared/layoutTree.ts`,
  `main/layout/LayoutStore.ts`, `renderer/panels/`, `components/layout/`, `components/sidebar/`,
  `docs/dev-rules/architecture-invariants.md` §3, `DESIGN.md`)
- [x] 6.1 Main window: app id per region (`com.xd.cindy`, `com.xd.cindycn`, `com.xd.cindydev`) and
  title; `window-state.json` as `electron-window-state` and `readPersistedWindowMaximized` keep it
  (bounds saved only in the normal state, maximized and fullscreen saved on close, off-screen bounds
  reset to defaults while the raw maximized flag still restores; macOS fullscreen restored after the
  window shows); 800x600 minimum; `swallowActivationClick` from `window-behavior-settings.json`. The
  first-close prompt and close behavior go to chapter 22 with the tray they hide to. The
  `window-theme-mode.json` mirror is not ported: gpui paints the first frame from the resolved theme,
  and writing the mirror would push the port's theme into Cindy's own Windows startup
  (Cindy's Windows maximize recovery after display changes works around Electron; whether gpui
  needs it is checked on the Windows runner in task 23.last)
- [x] 6.2 Layout tree as `shared/layoutTree.ts` (validate, coerce, every operation), matching Cindy's
  own functions on golden fixtures; `layout.v1.json` read and written as `LayoutStore.ts` does
- [x] 6.3a Layout geometry as `LayoutRoot.tsx` computes it: active share ledger, minimum widths and
  heights, root pixel widths with preferred widths, `normalizeSubMinFractions`, divider room, the chat
  shrink relay plan, `alignExpandedPaneFractions`, divider commit (with the `right-tabs` preferred
  width) and double-click equalize, column dividers
- [x] 6.3b The tree on screen: sidebar column slot, content splits, dividers dragged live and saved on
  release, double-click to equalize, self-heal after a resize settles, unknown kinds hidden, parked
  panes kept mounted at zero size, every window following `layout:changed`
- [x] 6.3c Toasts as `lib/toast.ts` keeps them (at most 3 shown, the rest queued in order, 1.2s for
  info and success, 8s for warning and error, 0 forever, hover pauses, 300ms exit, source and
  action) and `components/ui/toast/Toast.tsx` and `ToastContainer.tsx` draw them, in every window;
  the file transfer toasts in the same container go to chapter 18
- [x] 6.3d Drag a panel to swap root regions as `PanelDragController.tsx` does (long press, drop
  zones, swap, saved or failed toast)
- [x] 6.4 App shortcuts: the registry and matching of `shared/appShortcuts.ts`, the user's bindings
  read as `app-shortcuts/AppShortcutStore.ts` keeps them, dispatch as `useAppShortcut.ts` and
  `lib/appShortcutStore.ts` do; rebinding UI and writes go to chapter 12
- [x] 6.5a Left sidebar frame: width from `sidebar-width` with resize, limits, rail below 120px at
  78px and the card-mode snap as `useSidebarResize.ts` and `useHorizontalResize.ts` keep them,
  double-click reset; hidden state kept in the tree as `collapsePrefs.ts` does for `session-list`;
  rail kept in `sidebar-rail`, card snap read from `sidebar.cardMode`; ⌘B through the
  `toggle-sidebar` shortcut; the content area takes the rest of the window
- [x] 6.6a Window chrome: frameless per platform as `bootstrap-electron.ts` opens it (macOS hidden
  title bar with the traffic lights at 12,16; Windows and Linux without a frame, Linux resizing from
  its edges); `WindowControls` on Windows and Linux (minimize, maximize or restore, close; the close
  behavior prompt goes to chapter 22, the in-turn close confirm to chapter 10); the sidebar's 46px
  top row and `ContentHeader`'s 46px band drag the window, with the traffic-light inset and border
- [x] 6.6b `ChromeActions`: placed by the mac traffic lights and fullscreen, the sidebar toggle with
  its tooltip (`DESIGN.md` §14.6) and hover peek and pin as `useSidebarPeek.ts` does; the
  `ContentHeader` spacer that makes room for the cluster
- [x] 6.9a Reduced motion from the system, as Chromium's `prefers-reduced-motion` reads it (Linux
  `enable-animations`, macOS reduce motion, Windows client-area animation), applied as `globals.css`
  does: motion tokens at 0, spinners still, `motion-reduce:` transitions off; other features' rules
  land with them
- [x] 6.9b Window zoom from `appearance-settings.json` (`windowZoom`, clamped and stepped as
  `appearanceSettings.ts` does), scaling everything as Chromium's zoom factor does: Cindy's CSS
  pixels drawn as rems, the rem size 16 × zoom, layout math in CSS pixels; Chromium's per-URL zoom
  memory, which `restoreWindowZoom` undoes, has no gpui counterpart
- [x] 6.9c The app icon (`appIcon`) in the macOS Dock as `app-icon.ts` sets it, light and dark
- [x] 6.9d The window material per family as `vibrancyConfig.ts` resolves it (Windows acrylic and
  mica, macOS vibrancy)
- Wallpaper as Cindy shows it; the settings UI, the custom picker and its write IPC go to chapter 12:
  - [x] 6.9e1 The static wallpaper at startup: `wallpaperId`, `wallpaperOverlay`,
    `wallpaperVisibility` and `wallpaperBlur` normalized as
    `appearanceSettings.ts` does (null meaning, the legacy overlay-to-visibility veil), the custom
    URL from `custom-wallpaper.json` read from `cindy-media/client-wallpaper` with
    `assertBlobPathContained`'s checks (`custom` without one falls back to `none`), painted as
    `useWallpaperBackground` and its CSS do: bundled previews for official ids, cover image, veil,
    blur with overscan, the page surfaces it clears, and the coverage that turns 6.9d's native
    material off. The change-time crossfade and blur preview come with the settings page (12.0);
    the glass fills come with the surfaces that carry them
  - [x] 6.9e2 The official original image from Cindy's durable cache, read and verified as
    `wallpaper-video.ts` does before any network (`readBoundedFileNoFollow`, size and sha256),
    shown once decoded in place of the bundled preview; the download itself needs
    `cdnBaseUrl` and goes to 13.0
  - Dynamic motion (`wallpaperMotion`): cached official videos and custom videos played as
    `WallpaperVideo.tsx` does, paused when hidden or static, none under reduced motion; GPUI has no
    video element, so each platform decodes with its own system media framework:
    - [x] 6.9e3a The playback and its layer, decoded on Linux with GStreamer
    - [x] 6.9e3b Decoded on macOS with AVFoundation
    - [x] 6.9e3c Decoded on Windows with Media Foundation

## 7. Mock mode

- [x] 7.0 Expand from Cindy's code (`docs/dev-rules/ui-mock-data.md`, `main/index.ts` startup,
  `main/devCliFlags.ts`, `main/localDb/uiMockSeed.ts`, `maker-shared/uiMock.ts`,
  `uiMockShowcase.ts`). Cindy's mock mode is a source run in an isolated sandbox
  (`XDT_UI_MOCK=1` with `--isolated=ui-mock`, set by its restart script). A profile with no saved
  mode starts account-free (`startFreshProfileInLocalMode`); any saved choice wins. Each time the
  local database is ready for the account-free owner (`LOCAL_DATA_OWNER_ID`) with the gate on, the
  seed runs and never blocks startup (`uiMockSeed.ts`): demo project files under
  `ui-mock/projects/` are written only where absent, missing catalog tasks are inserted
  (`onConflictDoNothing`), so a deleted one returns and renamed or archived ones stay, messages go
  only to new tasks and to catalog tasks with no message row at all (an interrupted seed), so a
  deleted message's tombstone stays deleted. That needs chapter 8's database and chapter 13's
  local mode, so the gate, the local start and the seed go to 13.0; the restart script is Cindy's
  dev tooling, not the app. Mobile's mock is out of scope
- [x] 7.1 Dev profiles as `devCliFlags.ts` resolves them, in debug builds only (a source run;
  release builds keep the region folder): `--isolated` / `--isolated=<name>` and `XDT_ISOLATED` /
  `XDT_ISOLATED_NAME` (names `[A-Za-z0-9_-]{1,32}`; a bad name warns and uses the default
  sandbox), the `<userData>-dev2[-<name>]` sandbox, `XDT_USER_DATA_DIR` with
  `XDT_USER_DATA_DIR_EPOCH`, the profile kind (production-shared, isolated-sandbox, custom) by
  canonical path, case-insensitive volumes included, and the refusal to run `--isolated` on an
  official profile; the keychain identity marker, device id override and `--endpoints-cdn` go to
  13.0, `--passive` to 15.0, the profile-kind flags to the stores that read them (8.0 database,
  13.0 auth, 21.0 IM)

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
- [ ] 8.7 The top nav as `SidebarTopNav.tsx` draws it: New, Search with its inline results over the
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
    - [ ] 8.7c The search results in place of the list (`messages_fts` through `cjk_seg`, sorted
      as `conversationSearchSort` sorts), with the search row sticky over them and the clip
      below it while a query runs; the options' Projects submenu, which lists every known
      project, archived tasks' included
    - [ ] 8.7d More (`SidebarNavigationMoreMenu` opened on hover by `useHoverOpenMenu`) with its
      hidden entries and Customize (`SidebarNavigationCustomize.tsx`) over
      `sidebarNavigationPrefs.ts`; without a data owner the defaults apply and nothing is saved,
      as Cindy does, until 13.0's owner-scoped storage
    - [ ] 8.7e The rail (`SidebarRailNavigation`): the tiles in the saved order, Search as
      `ConversationSearchBox`'s popover, and More
- [ ] 8.8 Interrupted tasks after a restart: the candidates as `interruptedResumeCandidates.ts` and
  `listInterruptedPendingRows` find them (the turn's last message and last tool use), and
  `InterruptedTasksResumeDialog.tsx` once per window and owner (load retries, selection, the
  still-interrupted recheck before continuing); the continue itself dispatches a turn, which
  lands with the agent runtime (10.0)
- [ ] 8.9 Back and forward through the session browse history (`sessionBrowseHistory.ts`, at most
  50 in memory, cleared on owner change, ⌘[ / ⌘]) with `SessionHistoryButtons`
- [ ] 8.10 The project header's actions as `ProjectNode.tsx` draws them: the hover More and New
  in this folder buttons and its menu, by More or a right click (Rename, Search in this project
  with the search's locked project chip, Show files, Open in the file manager, Copy deep link,
  Import, Remove from sidebar), and the dialogue group's Search; actions whose feature lands in a
  later chapter come with it
- [ ] 8.11 A right click on the list's blank space opens the filter menu at the pointer, as
  `handleSidebarBlankContextMenu` opens `SidebarFilterPopover`
## 9. Chat view

- [ ] 9.0 Expand from Cindy's code
- [ ] 9.1 Tab groups (from 6.7): `right-tabs` and the edge groups, as `tabGroups.ts` and
  `ensureTabGroupPane` place them; edge groups parked on non-chat routes other than settings; an edge group's temporary
  maximize and restore without changing the tree (`panelMaximize.tsx`, `LayoutRoot.tsx`; its only
  trigger is `TabGroupPanel.tsx`); the pane placement signals panels read (`panePlacement.tsx`:
  at window top, at window left, fill); the panel registry as `panels/registry.ts` keeps it
  (built-in kinds with their collapse memory, tab group kinds registered, unregistered and
  parked); `right-tabs` collapse per session under `right-sidebar-collapsed:<sessionId>`; the window
  grows and shrinks as the right group opens and closes (`sidebarPresentation.ts`, `sidebarExpansion.ts`, anchored
  widths)
- [ ] 9.2 Detached windows (from 6.10): bounds in `right-sidebar-window-state.json` and the shared
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
