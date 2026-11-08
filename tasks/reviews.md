# Reviews

Each round: task, round, verdict, findings and their fixes. Reviewer: Claude Code on Kimi K3.1 Pro,
effort high, in a Herdr pane.

## 1.1 Crate, window, docs

- Round 1: PASS. No findings.

## 1.2 CI workflow

- Round 1: PASS. No findings.

## 1.3 CI green

- Round 1: PASS. Run 42450671411, all six legs green.

## 2.0 + 2.1 Region and userData

- Round 1: FAIL. Region test asserted global, so cn and dev builds failed it. Fixed: test checks the
  folder for the built region.
- Round 2: PASS.

## 1.4 Self-hosted CI

- Round 1: PASS. Run 42453290256 green: linux-x64 on omarchy-amd64, windows-x64 on elysia-windows,
  macos-arm64 on mac-mini, the rest GitHub-hosted.

## 2.2 Main log

- Round 1: FAIL. (1) closed terminal crashed the app; (2) LOG_LEVEL=error never opened the file;
  (3) fatal aliased to error; (4) invalid config diagnostics could vanish. All fixed.
- Round 2: FAIL. A non-object log-config.json was read as no override. Fixed: object required.
- Round 3: PASS.

## 1.5 CI on manual dispatch

- Round 1: PASS.

## 3.0 Storage map

- Round 1: FAIL. Lease, admission and cjk_seg summarized too loosely; inventory incomplete; owners
  pointed at consumers. Fixed: port those files whole; owners corrected.
- Round 2: FAIL. Wildcard rows and a deferred-owner escape hatch; blanket temp-rename rule. Fixed:
  every module gets one chapter; each store keeps its owner's publication and locking.
- Round 3: FAIL. Search missed stores with computed names. Fixed: added JSON.stringify writers.
- Round 4: FAIL. Search missed stores written through helpers. Fixed: added helper callers.
- Round 5: FAIL. Search missed stream writers and a `.json` mention without quotes. Fixed: search
  widened to any `.json` mention and any stream, append, rename or `write*File` writer; 583 modules.
  Review limit reached; committed with the round-5 findings fixed and the list marked a superset
  that each chapter re-checks at expansion.

## 4.0 i18n plan

- Round 1: FAIL. Plural spec named only `_one`/`_other`; namespaces, `reservedGhostIdPrefixes` and
  the `chat.remoteError.*` fallback were missing. Fixed in task 4.3.
- Round 2: PASS.

## 4.1 Locale files

- Round 1: PASS.

## 4.2 Locale choice

- Round 1: PASS.

## 4.3 Translator

- Round 1: PASS.

## 1.6 Reference commit 1ac47f3

- Round 1: PASS.

## 5.0 Themes plan

- Round 1: FAIL. Token count was 568 (call sites); Cindy has 576 with two loops. Dark-to-light
  default fallback missing. Both fixed.
- Round 2: PASS.

## 5.1 Theme data

- Round 1: PASS.

## 5.2 Token resolution and families

- Round 1: PASS.

## 5.3 Ely palette

- Round 1: PASS.

## 5.4 Theme import (VS Code)

- Round 1: FAIL. Whitespace was ASCII-only; Cindy splits and trims with ECMAScript whitespace.
  Fixed with js_space/js_trim; cases checked against Cindy's parseCssColor.
- Round 2: PASS.

## 5.5 Local themes

- Round 1: PASS.

## 5.6 Omarchy live theme

- Round 1: PASS.

## 5.7 Font settings

- Round 1: FAIL. serde_json rejected a whole file for one number past f64 (1e400) that JSON.parse
  reads as Infinity. Fixed: fields kept raw, uiSize parsed per JS with Number.isFinite.
- Round 2: PASS.

## 6.0 Window shell plan

- Round 1: FAIL. Plan listed parts, not behavior: maximize/fullscreen restore, window behavior
  store, collapse scopes, drag/maximize interactions, sidebar states and nav prefs, settings
  takeover, custom wallpaper, detached-window lifecycle. All added.
- Round 2: FAIL. Wallpaper veil/visibility/blur, the window-theme-mode mirror, and the zoom owner
  (appearance-settings-ipc.ts) were missing or misplaced. Fixed; storage.md rows corrected.
- Round 3: PASS.

## 6.1 Main window

- Round 1: FAIL. Minimized windows could overwrite normal bounds; window-state and window-behavior
  readers rejected a whole file for one number past f64. Fixed: bounds from gpui's windowed
  placement; shared cindy_json reader with JS number semantics.
- Round 2: FAIL. macOS reports Windowed while maximized. Fixed: normal needs Windowed and both
  flags off.
- Round 3: PASS.
