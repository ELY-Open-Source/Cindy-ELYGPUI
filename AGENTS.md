# AGENTS.md

Cindy-ELYGPUI: Cindy's desktop client in Rust, on GPUI and Ely GPUI Components. `docs/PRD.md`
defines scope and what 1:1 means.

## Stack

- Rust 1.98, edition 2024, pinned in `rust-toolchain.toml`. One crate, `cindy`, a binary.
- `ely-gpui-component` from GitHub, pinned by `rev` in `Cargo.toml`.
- gpui and gpui_platform as `gpui-pre` and `gpui-pre-platform` `=0.3.11`, Ely's pin.
  gpui_platform takes `font-kit`, `wayland` and `x11`. Ely's default feature turns on
  `runtime_shaders`.
- Linux builds need `libxkbcommon-x11-dev` and `libfontconfig-dev`.
- Logging: `log`, written by `env_logger` to stderr. `RUST_LOG` sets the filter; default
  `warn,cindy=info`.

## Commands

- `cargo run` opens the app.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. CI
  (`.github/workflows/ci.yml`) runs them and a release build on Linux, macOS and Windows,
  x86_64 and arm64.

## Rules

- Read Cindy's code and rule docs for an area before porting it. Cindy at the PRD's commit is the
  reference; do not guess its behavior.
- Keep Cindy's persisted formats and wire protocols. Change internal structure freely.
- Colors, sizes and radii come from Ely's `cx.theme()`. Reuse Ely components before writing new UI.
- Errors surface: no fallback that hides a failure. Log each key step.
- No file over 500 lines. Comments only where needed, one short line.
- `TASKS.md` is progress truth. One task, one review pass, one commit, one push.
- Update this file when the stack or direction changes.
