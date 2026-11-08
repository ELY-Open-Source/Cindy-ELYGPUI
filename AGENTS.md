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
- `node scripts/sync-golden.mjs <cindy checkout> <commit>` runs Cindy's own
  `convertVsCodeTheme` and `normalizeLocalThemeColors` on the inputs under `tests/fixtures/` and
  writes the expected outputs; the Rust ports must match them.
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
