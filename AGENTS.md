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
- Toasts: `src/toast.rs` ports `lib/toast.ts` as one `Toasts` global with a board per window, as
  each Cindy renderer keeps its own; timers run on gpui's executor clock. `toast/view.rs` draws
  `Toast.tsx` and `ToastContainer.tsx`; its `Slide` element does the FLIP in prepaint, so the
  slide starts in the frame the new tops are known. `motion::ease_out` is CSS `ease-out`.
- Cindy's `components/ui` primitives are drawn with Ely's (the toast action is Ely's secondary
  small `Button`); Cindy's `--text-N` is `appearance_settings::text_px`.
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
- No file over 500 lines. Comments only where needed, one short line.
- `TASKS.md` is progress truth. One task, one review pass, one commit, one push.
- Update this file when the stack or direction changes.
