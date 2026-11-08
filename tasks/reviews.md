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
