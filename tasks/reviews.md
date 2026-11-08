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
