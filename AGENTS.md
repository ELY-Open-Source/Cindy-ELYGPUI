# AGENTS.md

Cindy-ELYGPUI: Cindy's desktop client in Rust, on GPUI and Ely GPUI Components. `docs/PRD.md`
defines scope and what 1:1 means.

## Stack

- Rust 1.98, edition 2024, pinned in `rust-toolchain.toml`. One crate, `cindy`, a binary.
- `ely-gpui-component` from GitHub, pinned by `rev` in `Cargo.toml`.
- gpui and gpui_platform as `gpui-pre` and `gpui-pre-platform` `=0.3.11`, Ely's pin.
  gpui_platform takes `font-kit`, `wayland` and `x11`. Ely's default feature turns on
  `runtime_shaders`.
- Region is set at build by `CINDY_AUTH_REGION` (`cn`, `global`, `dev`; default `global`) through
  `build.rs`. The app shares Cindy's userData folder for that region (user decision).
- Data owner (`src/session_owner.rs`, Cindy's `appSessionState.ts` read only): `app-session.json`'s
  `activeMode`; `local` is the `local-v1` owner with `owners/<first 20 hex of sha256(id)>/`.
  Signed-out and cloud owners need chapter 13. A file that does not parse is removed, as Cindy's
  store does.
- Local database (`src/localdb.rs`): `cindy-local-v1.db` opens as Cindy's passive instances open
  a shared database, never created or migrated: the reader lease (`lease.rs`, Cindy's
  `schemaMigrationLease.ts`; records and the recovery observation stay byte-compatible), the
  pragmas, `cjk_seg` with its TEMP triggers (`cjk.rs`; character classes in
  `resources/cjk-classes.json` come from V8 through `scripts/sync-golden.mjs`), sqlite-vec 0.1.9,
  then `admission.rs` against `resources/migration-manifest.json` (Cindy's 149 migrations, also
  from `sync-golden.mjs`). The connection and its lease live on one thread (`worker.rs`). Any
  refusal shows `fatal.rs`, Cindy's `LocalDbFatalScreen`, above everything.
- Task list (`src/sessions.rs`, Cindy's `sessionsStore.ts` for the filter's status bucket): the
  first read sweeps provisional leftovers once per database open (`sweep.rs`), then `query.rs`
  reads `ordinaryTaskListFilter`'s rows. Cindy's push events never reach the port, so a watcher
  on the database, its WAL and `owners/<key>/sidebar-settings.json` (`settings.rs`) reloads,
  single-flight. `lists.rs` builds both lists from one bucket as `CCAgentSidebarUpper.tsx` does:
  `pinned.rs` draws the Pinned section, `main_list.rs` the main list in `project` or `flat` mode,
  from `group.rs` (`groupSessions`, `buildMainListEntries`) and `paths.rs` (project keys). All of it
  is checked against `tests/fixtures/grouping` from `sync-golden.mjs`. `row.rs` is the task row,
  with `title.rs` (`getSessionDisplayTitle`, golden `display-title.json`) and `meta.rs`
  (`SessionInfoMeta`, golden `meta.json`; task info read once, as Cindy caches it).
  `task_menu.rs` is the row's `SessionTaskMenu` (Ely `ContextMenu` on a right click, `menu_under`
  on the hover More), `rename.rs` the `SessionRenameInput` on Ely's `TextInput`, and `write.rs`
  `patchMeta`: the row patched first, the `UPDATE` on the database worker, rolled back with a
  toast on failure. The port writes Cindy's database the way Cindy's own main process does.
  `pin.rs` is `handleTogglePin`: `pinnedAt` through `write.rs`, and a pin promoted in
  `sidebar-settings.json`'s `pinnedOrder` by `settings::promote` (`savePinnedOrder` and
  `updateAtomic`: under the lock, an unreadable file refused, tmp then rename).
  `src/cross_process_lock.rs` is the advisory tier of Cindy's `crossProcessLock.ts`: the
  `{pid, startedAt, nonce}` file, a 2 s heartbeat, nonce-bound release, and stale takeover
  under the `.reclaim.d` gate, with owner liveness from `kill(pid, 0)` and `ps` (PowerShell,
  then WMIC, on Windows). It blocks, so callers run it on the background executor.
  `filter.rs` reads and writes the filter preferences; `header.rs` draws the scope header and
  the filter menu on Ely's `Menu` (`menu_under`). Ely's menu has no row value, tip or kept-open
  choice, and lacks some lucide icons; the nearest Ely icon stands in.
  `fold.rs` is `SectionCollapse` and the first-N fold. Show all is keyed by section and group and
  resets once its list is no longer drawn, as each `SessionEntryList` holds its own. Animations read
  `background_executor().now()` so tests can advance the clock.
  `preview.rs` is `finalizePlainPreview` (its LRU cache, `stripInternalWebCitations`) and
  `projection.rs` the list's latest-message extract with `scheduleSessionListProjectionBackfill`:
  after a read, one `UPDATE` fills the empty `list_preview` and `list_message_count` columns.
  `view_mode.rs` is `useSidebarCardMode.ts` with `sidebarInstallVintage.ts`: the Pinned mode
  (`sidebar.cardMode`) and the main list's (`sidebar.mainListMode`), kept in memory as Cindy's
  module memo, chosen from the filter menu's Display row and the Pinned header's view style menu.
  `list_row.rs` is `SessionCard`'s `list` variant (dividers, title line, one preview line from
  `sessionToCamel`'s `preview`), drawn in list mode; groups inset it `pl-2`.
  `card.rs` is the `card` variant (title, `resolveSessionCardBody`, meta row) and `masonry.rs`
  `CardMasonry`: one to three round-robin columns from the width measured in prepaint, with
  `card-col-fade`; card mode's fold height is that measured height.
  `marquee.rs` is `SidebarTitleMarquee` in the text and list rows: the row's `on_hover` picks one
  title, prepaint canvases measure it and its track, and the track scrolls on the executor clock.
  The hover ignores key presses, as CSS `:hover` does. An unmounted row lets go at the next list
  render; a title back from rename starts again. Tests drive frames with `simulate_next_frame`.
  `sortable.rs` is `SortableList`'s native drag for the Pinned text and list modes: gpui's
  `on_drag` per entry, the entry under the pointer takes the dragged one's place (`swapThreshold:
  1`), displaced entries slide 150ms, a drop on the list saves; Escape, a drop elsewhere or a lost
  window put the order back. Cards drag the same way across `DraggableCardColumns`' round-robin
  columns (`sortable/order.rs`): into another column an entry goes after a target it sat above,
  else before; a column's empty tail appends; the drop saves `reorderByDropSlot`'s
  `row * N + column`. Slides move in x and y. `reorder.rs` is `handlePinnedReorder` (`buildManualPinnedReorder`,
  golden `pinned-reorder`) and `settings::save` takes `savePinnedOrder`'s `reorder`, rebased on the
  file as `rebasePinnedReorder` does. A press on a group's tasks does not drag the group.
- Router: `src/router.rs` is `router.tsx`'s hash routes for chapter 8: one pathname in the
  `Router` global, starting at `/`. `/` goes to `/cc-agent` (`MainEntryRedirect`; the remembered
  entry needs 13.0's owner-scoped storage); `/cc-agent` waits for the task list, then opens the
  last chat view (`lastViewStore`'s chat slot, kept by `navigate` as `useTrackLastView` does), the
  first task, or `/cc-agent/new`. The chat pane shows its loading line meanwhile.
  `sessions/open.rs` is `handleSessionClick`: a plain click on a row, list row or card opens
  `/cc-agent/<id>` unless it is shown; a Shift, Mod or Ctrl click waits for 8.6d's selection.
  The active task (`router::active_session`) draws as Cindy's: its pill and ring (an absolute
  bordered overlay stands in for the inset shadow), foreground and tints. The task menu wraps a
  row while it is renamed too, so the press that commits a rename still lands its click; the
  field stops its own right press.
  `sessions/keyboard.rs` hosts that menu (Ely `ContextMenu::manual`) and makes each row, list
  row and card a Tab stop: Enter or Space on the row itself opens it; the menu key (key up on
  Windows) or Shift+F10 (not macOS) opens the menu at the focused row's or More's centre, as
  Chromium's `ShowNonLocatedContextMenu` does. Prepaint probes record those visible bounds.
  `select.rs` is the bulk selection of `handleSessionClick`: the row probes also list the rows
  painted (the `Drawn` global, pinned card slots marked with `rowMajorRank`), which stands in
  for `getVisibleSidebarSessionIds`; a probe at the list's end prunes the selection, as the
  MutationObserver does. The bulk bar has Clear; Archive and Delete come with 18.0.
- Previews: `src/markdown_text.rs` is `markdownPreviewText` over the `markdown` crate's mdast
  (wooorm's markdown-rs, the mdast remark-parse and remark-gfm build). The crate is vendored in
  `vendor/markdown` (1.0.0, upstream otherwise) with three patches marked `Cindy:` where it
  parts from micromark: ATX heading text, character classes as micromark reads UTF-16 code
  units, and laziness at end of file. `src/rich_answer.rs` is
  `richAnswerText`: `cindy-ui` fences split from Markdown, `stream.rs` replays the ops once,
  `values.rs` resolves them, `js.rs` parses them as `JSON.parse` does (`Infinity` apart from
  `null`, no nesting limit: its recursion grows the stack through `stacker`, as V8 nests a
  million levels). Goldens in `tests/fixtures/preview` come from Cindy's functions; `sync-golden.mjs`
  stubs Electron, which `mapper.ts` imports.
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
  `src/theme/normalize.rs` applies `normalizeLocalThemeColors`; local families follow the built-in
  ones.
- Omarchy: `src/theme/omarchy.rs` reads `~/.local/state/omarchy/current/theme/vscode-theme.json`
  through the VS Code converter as family `omarchy`, never writing Omarchy files. An inotify
  watcher (`notify`) reloads every theme on each finished `theme.name` write; a read inside
  Omarchy's folder swap keeps the last good theme.
- Fonts: `src/appearance_settings.rs` reads Cindy's shared `appearance-settings.json` (fonts only so
  far) and sets Ely's font scale (`uiSize / 14`) and families; gpui takes one family, so the first
  of Cindy's CSS font list is used.
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
  (visible-share ledger, floors, root widths, self-heal, divider room and chat relay);
  `divider.rs` the divider gestures; `view.rs` the `LayoutRoot` view. The view lays panes out
  itself, so the widths it paints stand in for Cindy's DOM measurements. Its width is the window
  less the sidebar column, so the first frame is already right. A divider drag listens on the
  whole window and writes the store once on release; self-heal runs 120ms after width or tree
  settle, never mid-drag. The view follows `LayoutStore` through `observe_global`, which stands in
  for `layout:changed`.
- App shortcuts: `src/app_shortcuts.rs` ports `shared/appShortcuts.ts` (registry, normalize,
  effective map, scope overlap); `store.rs` reads `app-shortcuts.v1.json` at startup into the
  `AppShortcuts` global, normalized from `JSON.parse`-shaped fields (an overflowing flag is
  `Infinity`, so truthy); `keys.rs` turns a gpui keystroke back into Cindy's `KeyboardEvent.code`.
  gpui reports the typed key, not the physical one, and drops Shift for a shifted symbol on all
  three platforms, so a US-layout table restores the code: on AZERTY or Dvorak a shortcut follows
  the letter. Keypad digits, decimal and Enter reach the app as their main-keyboard twins
  (`Digit1`, not `Numpad1`): gpui's `keystroke_from_xkb` strips `kp_` and `KeyDownEvent` carries no
  scancode. Only a gpui change that exposes the physical key closes this; keypad `+` and `-`
  survive. A shortcut handler sits on the window's root element in `capture_key_down`. The
  root holds focus while nothing else does (`on_focus_lost` hands it back), so every key's dispatch
  path runs through it; with nothing focused gpui dispatches to the tree's own root only. Ely's
  `FocusScope` wraps the root on its own handle, so Tab and Shift-Tab walk gpui's tab stops.
- Sidebar frame: `src/sidebar.rs` ports `Sidebar.tsx` and the width, rail and hidden state
  `MainLayout.tsx` keeps; `sidebar/resize.rs` ports `useHorizontalResize.ts` with the sidebar's
  limits. Width and rail live in the preferences file (`sidebar-width`, `sidebar-rail`), the
  hidden state in the layout tree. The content area's width is the window less the sidebar's
  painted width, read every frame of its 250ms width transition.
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
  through gpui's tooltip (500ms, as Cindy's; gpui places it by the pointer, not under the button).
  Lucide icons at Cindy's sizes come from `IconName::svg()` through gpui `svg()`; `lucide::icon`
  also swaps in Cindy's `strokeWidth`. An `svg` takes no inherited color: set `text_color` on it.
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
- Wallpaper: `src/wallpaper.rs` normalizes the wallpaper keys of `appearance-settings.json` and
  reads `custom-wallpaper.json` (golden cases in `tests/fixtures/appearance`). `canvas.rs` paints
  `body::before` behind the main window: surface, cover image, veil; with a blur it blurs off the
  main thread at source resolution (`media::blurred`) and stretches it over the overscanned box.
  `clear` empties page surfaces while a wallpaper is active. Custom media is read only through
  `media::custom_path`, which keeps `assertBlobPathContained`'s checks. Logs name the kind of
  wallpaper, never the custom URL. An official wallpaper shows its bundled preview, then the
  original from Cindy's shared cache when `media::verified` matches the manifest
  (`resources/wallpaper-video-manifest.json`) in size and sha256. Downloads come with 13.0.
- Wallpaper video: `wallpaper/video.rs` ports `WallpaperVideo.tsx` (`wanted` decides; `sync`
  opens, pauses or drops one decoder; `layer` paints it over the veil and drops retired frames).
  The decoder plays the bytes that were read and checked, never a path reopened later; errors
  log GStreamer's domain and code, not its text. Each platform decodes with its system
  framework: Linux GStreamer (`video/gst.rs`, from a sealed memfd, needs
  `gst-plugins-good` and `gst-libav` at run time); macOS AVFoundation (`video/avf.rs`, the checked bytes
  in a private temp file) and Windows Media Foundation (`video/mf.rs`, a memory stream) are
  `paced::Decoder`s: `paced::Player` runs one on its own thread, paces frames by their times,
  and restarts it to loop or refit. A hidden
  window pauses through gpui's `WindowVisibility`; Hyprland 0.56 sends no `suspended` state for a
  window on another workspace, so there it keeps playing. Decoder threads wake gpui from outside,
  so tests drive the real decoder only outside the gpui test scheduler.
- Untrusted files: read with `bounded_read::read`, Cindy's `readBoundedFileNoFollow` (no-follow
  open, regular file, size limit, stable while read).
- Toasts: `src/toast.rs` ports `lib/toast.ts` as one `Toasts` global with a board per window, as
  each Cindy renderer keeps its own; timers run on gpui's executor clock. `toast/view.rs` draws
  `Toast.tsx` and `ToastContainer.tsx`; its `Slide` element does the FLIP in prepaint, so the
  slide starts in the frame the new tops are known. `motion::ease_out` is CSS `ease-out`.
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
  (`src/layout_root/screen_tests.rs`). Live checks on Hyprland: `hl.dsp.cursor.move({ x, y })`,
  `hl.dsp.window.float/resize/move({ window = "address:..", x, y })`; a click needs a uinput device.
- userData is created at startup, as Electron does.
- Linux builds need `libxkbcommon-x11-dev` and `libfontconfig-dev`.
- Logging: `log` facade, written by `src/logger.rs` in Cindy's main-log format to
  `main-YYYY-MM-DD.log` (debug builds: `logs/` in the checkout; release: `<userData>/logs`).
  Level: `LOG_LEVEL`, then `log-config.json`, else `trace` in debug and `info` in release; other
  crates log at `warn` and above. A log's `target` is its Cindy scope.

## Commands

- `cargo run` opens the app.
- `scripts/sync-locales.sh <cindy checkout> <commit>` copies Cindy's locale files into `locales/`
  and records the commit in `locales/COMMIT`. Never edit `locales/` by hand.
- `node scripts/sync-themes.mjs <cindy checkout with node_modules> <commit>` runs Cindy's
  `colors.ts` and built-in themes through Cindy's esbuild and writes `themes/` (registry, built-ins,
  logos, `COMMIT`). Never edit `themes/` by hand.
- `node scripts/sync-golden.mjs <cindy checkout> <commit>` runs Cindy's own `convertVsCodeTheme`,
  `normalizeLocalThemeColors` and `layoutTree` on the inputs under `tests/fixtures/` and writes the
  expected outputs; the Rust ports must match them.
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
- No file over 500 lines (vendored upstream code apart). Comments only where needed, one short line.
- `TASKS.md` is progress truth. One task, one review pass, one commit, one push.
- Update this file when the stack or direction changes.
