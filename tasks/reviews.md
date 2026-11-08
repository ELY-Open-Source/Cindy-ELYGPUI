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

## 6.2 Layout tree

- Round 1: FAIL. `walkPanes` missing (collapsePrefs uses it); `js_number` took Rust's shortest
  digits, not the closest. Fixed: `walk_panes`; closest digits, checked on 200k doubles.
- Round 2: PASS.

## 6.3a Layout geometry

- Round 1: PASS.

## 6.3b The tree on screen

- Round 1: FAIL. A non-row root got draggable dividers; Cindy keeps them still. Fixed: flattened
  root with still dividers; still dividers share the family surface underlay.
- Round 2: PASS.

## 6.3c Toasts

- Round 1: FAIL. Hover during the exit fade dropped the exit task; the source header lacked the
  message's text style; a doc comment sat on the wrong function. Fixed all three.
- Round 2: PASS.

## 6.3d Panel swap

- Round 1: FAIL. `anchored()` fit the ghost and zone inside the window; Cindy's stay at the
  pointer offset and clip. Fixed: absolute boxes in a deferred layer at the content origin.
- Round 2: PASS.

## 6.3e Temporary maximize

- Moved into 6.7: its only trigger is `TabGroupPanel.tsx`; built alone it would be dead code.

## 6.4 Panel registry

- Spread to first callers: registry and per-session collapse to 6.7 (`tabGroupPanes.ts`),
  sidebar collapse and the legacy migration to 6.6 (`MainLayout.tsx`). `clearPanelCollapsedRecord`
  has no caller in Cindy and is not ported.

## 6.4 App shortcuts

- Round 1: FAIL. Overflowing flags lost JSON.parse truthiness; `insert` unmapped; keypad digits
  read as top-row digits. Fixed the first two; the third is a gpui limit (no keypad marker or
  scancode reaches the app), written in AGENTS.md.
- Round 2: PASS.

## 6.5a Left sidebar frame

- Round 1: FAIL. A press mid-expand kept the slide running; the tween ignored a new duration.
  Fixed: a new duration restarts it, with a regression test.
- Round 2: PASS.

## 6.5b and 6.5c Sidebar top nav and sections

- Moved: every top nav row and section needs a route, search, owner key or page that lands later
  (6.8, 8, 13, 14, 15, 16); built now they would be dead buttons. Placed in 8.0 and 13.0 to 16.0.

## 6.6a Window chrome

- Round 1: FAIL. Header presses reached the pane's panel long press; the header inset slid in
  from 0 on the first frame. Fixed both, with regressions.
- Round 2: PASS.

## 6.6b Chrome cluster and sidebar peek

- Round 1: FAIL. A pin mid-slide left the drawer off screen; a hide during the pin slid the
  column; the toggle lacked aria-expanded. Fixed all three; the first two with regressions.
- Round 2: PASS.

## 6.7 and 6.10 Tab groups and detached windows

- Moved to 9.1 and 9.2: the right sidebar's tabs live per task, are mostly plugins, and are turned
  on by the chat view; built in chapter 6 they would host nothing.

## 6.8 Routes

- Moved: the task routes need the task list (8.0); the settings route hides the sidebar and is left
  only through the settings page (12.0), so built now it would strand the user on an empty page.

## 6.9a Reduced motion

- Round 1: PASS.

## 6.9b Window zoom

- Round 1: FAIL. gpui's `border_*_1()` helpers are fixed pixels, so borders did not zoom; shadow
  lengths did not zoom either.
- Round 2: PASS. Borders use `css(1.0)`; `shadow_token` scales by the zoom.

## 6.9c Dock icon

- Round 1: PASS.

## 6.9d Window material

- Round 1: PASS.

## 6.9e1 Static wallpaper

- Round 1: FAIL. The custom wallpaper URL reached the log through `Debug` and path-bearing
  error contexts.
- Round 2: PASS. Settings log `custom: true|false`; custom-media errors carry fixed text.

## 6.9e2 Cached official original

- Round 1: FAIL. The bounded read compared size and mtime only, not Cindy's full handle
  version.
- Round 2: PASS. Identity, mode, size, mtime and ctime from the open handle.

## 6.9e3a Wallpaper video, Linux

- Round 1: FAIL. GStreamer error text named the custom file; filesrc reopened a verified path.
- Round 2: FAIL. Custom video containment was checked apart from its open.
- Round 3: FAIL. The identity checks opened the path, which blocks on a FIFO.
- Round 4: PASS. lstat identities, a non-blocking no-follow open, sealed verified bytes.

## 6.9e3b Wallpaper video, macOS

- Round 1: PASS. Not run on a Mac; the macOS CI leg runs the decode test.

## 6.9e3c Wallpaper video, Windows

- Round 1: FAIL. COM and Media Foundation started per decoder thread, never shut down.
- Round 2: PASS. Drop guards undo each successful start in reverse order.
