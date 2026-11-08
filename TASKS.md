# Tasks

Progress truth, in `docs/PRD.md` chapter order. A task is ticked after review passes, then committed
and pushed. Reviews: `tasks/reviews.md`, at most five rounds per task.

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
- [ ] 6.5a Left sidebar frame: width from `sidebar-width` with resize, limits, rail below 120px at
  78px and the card-mode snap as `useSidebarResize.ts` and `useHorizontalResize.ts` keep them,
  double-click reset; hidden state kept in the tree as `collapsePrefs.ts` does for `session-list`,
  with the legacy `sidebar-collapsed` migration; hover peek and pin as `useSidebarPeek.ts` does
- [ ] 6.5b Sidebar top nav: New, Automations, Plugins, Search, More with order, top-level or More
  placement and plugin entries per owner as `SidebarTopNav.tsx`, `sidebarNavigationPrefs.ts` and
  `SidebarNavigationCustomize.tsx` keep them
- [ ] 6.5c Sidebar sections: Teammates, Pinned and All sessions as frames that chapters 8 and 14
  fill (`sidebarSettingsStore.ts` lands with the pinned and hidden data it owns)
- [ ] 6.6 Title bar: `ContentHeader` drag band, `WindowControls` per platform, `ChromeActions` placed
  by the mac traffic lights and fullscreen, the sidebar toggle with its peek trigger, the menu
  button whose items land with their targets (`DESIGN.md` §14.6); the pane placement signals
  panels read (`panePlacement.tsx`: at window top, at window left, fill)
- [ ] 6.7 Tab groups: `right-tabs` and the edge groups, as `tabGroups.ts` and `ensureTabGroupPane`
  place them; edge groups parked on non-chat routes other than settings; an edge group's temporary
  maximize and restore without changing the tree (`panelMaximize.tsx`, `LayoutRoot.tsx`; its only
  trigger is `TabGroupPanel.tsx`); the panel registry as `panels/registry.ts` keeps it (built-in
  kinds with their collapse memory, tab group kinds registered, unregistered and parked); `right-tabs`
  collapse per session under `right-sidebar-collapsed:<sessionId>`; the window grows and shrinks
  as the right group opens and closes (`sidebarPresentation.ts`, `sidebarExpansion.ts`, anchored
  widths)
- [ ] 6.8 Routes: chat and settings with back and forward history; settings takes over the window,
  hiding the sidebar and non-chat panels and unregistering edge groups without changing the tree,
  and restores them on exit
- [ ] 6.9 Reduced motion zeroing every motion token, as `globals.css` does; window zoom and app icon
  from `appearance-settings.json`; the window material per family as
  `vibrancyConfig.ts` resolves it (Windows acrylic and mica, macOS vibrancy); wallpaper as Cindy shows it:
  `wallpaperId` with the custom URL from `custom-wallpaper.json` (`custom` without one falls back to
  `none`), official images and videos downloaded and cached as `wallpaper-video.ts` does, static and
  dynamic motion with reduced motion; `wallpaperOverlay`, `wallpaperVisibility` and `wallpaperBlur`
  with their normalization, null meaning and the legacy overlay-to-visibility derivation; zoom read,
  applied and restored across navigation and focus as `appearance-settings-ipc.ts` does; the settings
  UI and its write IPC go to chapter 12
- [ ] 6.10 Detached windows: bounds in `right-sidebar-window-state.json` and the shared
  `sidebar-tab-window-state.json`, restored without maximize or fullscreen; which tabs are detached
  lives only for the process, so restart or crash returns them to the main window; legacy
  `right-sidebar-window-settings.json` deleted; detach, merge back, close and multi-window cascade as
  `sidebar-tab-window/controller.ts` does; `swallowActivationClick` for these windows too

## 7. Mock mode

- [ ] 7.0 Expand from Cindy's code

## 8. Sessions

- [ ] 8.0 Expand from Cindy's code, with the title bar's back and forward through the task browse
  history (`useSessionBrowseHistory`, `SessionHistoryButtons`)

## 9. Chat view

- [ ] 9.0 Expand from Cindy's code

## 10. Agent runtime

- [ ] 10.0 Expand from Cindy's code, with the agent and session `ndjson` log streams

## 11. Models and providers

- [ ] 11.0 Expand from Cindy's code

## 12. Settings

- [ ] 12.0 Expand from Cindy's code, with Obsidian theme conversion and the theme import button

## 13. Account and region

- [ ] 13.0 Expand from Cindy's code, with log upload and redaction, and the database admission in
  `docs/storage.md` (reader lease, read-only migration check, `cjk_seg`)

## 14. Teammates

- [ ] 14.0 Expand from Cindy's code

## 15. Automations

- [ ] 15.0 Expand from Cindy's code

## 16. Plugins

- [ ] 16.0 Expand from Cindy's code

## 17. Tools and integrations

- [ ] 17.0 Expand from Cindy's code

## 18. Files and git

- [ ] 18.0 Expand from Cindy's code

## 19. Remote

- [ ] 19.0 Expand from Cindy's code

## 20. Device link

- [ ] 20.0 Expand from Cindy's code

## 21. IM channels

- [ ] 21.0 Expand from Cindy's code

## 22. Desktop extras

- [ ] 22.0 Expand from Cindy's code, with the Windows and Linux first-close prompt and close behavior
  (`mainWindowCloseBehavior.ts`, `windowBehavior.ts`), which hide to the tray

## 23. Distribution

- [ ] 23.0 Expand from Cindy's code
- [ ] 23.last Dispatch CI and get all six targets green; run the app on the Windows runner and check
  that maximized windows survive display and DPI changes without Cindy's maximize recovery
