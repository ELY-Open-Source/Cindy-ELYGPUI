# Tasks

Progress truth, in `docs/PRD.md` chapter order. A task is ticked after review passes, then committed
and pushed. Reviews: `tasks/reviews.md`, at most five rounds per task.

## 1. Foundation

- [x] 1.1 Crate, Ely window, env logging, license, notice, `AGENTS.md`, PRD, this file; public repo
  `ELY-Open-Source/Cindy-ELYGPUI`, first push
- [x] 1.2 CI: fmt, clippy, test and build on Linux, macOS, Windows, x86_64 and arm64
- [x] 1.3 CI green on all six targets
- [ ] 1.4 CI on self-hosted runners where the org has one: Linux x64 `omarchy-amd64`, macOS arm64
  `macos-arm64`, Windows x64 `windows-amd64` (any idle one of that label)

## 2. Logging and diagnostics

- [x] 2.0 Expand from Cindy's code (`main/logger.ts`, `shared/mainLogRecordFormat.ts`,
  `main/regionUserData.ts`, `maker-shared/brandIdentity.ts`)
- [x] 2.1 Region from `CINDY_AUTH_REGION` at build (`cn`, `global`, `dev`; default `global`; other
  values fail the build) and the userData folder `Cindy`, `CindyGlobal` or `CindyDev` under the OS
  app-data folder, shared with Cindy
- [ ] 2.2 Main log: `main-YYYY-MM-DD.log` in `<userData>/logs` (debug builds: `logs/` in the
  checkout), Cindy's line format and continuation escaping, format sentinel on each file open, level
  from `LOG_LEVEL` then `log-config.json`, daily switch, 30-day retention, terminal mirror in debug
  builds
- Agent and session `ndjson` streams move to chapter 10; log upload and redaction to chapter 13.

## 3. Configuration and storage

- [ ] 3.0 Expand from Cindy's code

## 4. i18n

- [ ] 4.0 Expand from Cindy's code

## 5. Themes and appearance

- [ ] 5.0 Expand from Cindy's code

## 6. Window shell and layout

- [ ] 6.0 Expand from Cindy's code

## 7. Mock mode

- [ ] 7.0 Expand from Cindy's code

## 8. Sessions

- [ ] 8.0 Expand from Cindy's code

## 9. Chat view

- [ ] 9.0 Expand from Cindy's code

## 10. Agent runtime

- [ ] 10.0 Expand from Cindy's code, with the agent and session `ndjson` log streams

## 11. Models and providers

- [ ] 11.0 Expand from Cindy's code

## 12. Settings

- [ ] 12.0 Expand from Cindy's code

## 13. Account and region

- [ ] 13.0 Expand from Cindy's code, with log upload and redaction

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

- [ ] 22.0 Expand from Cindy's code

## 23. Distribution

- [ ] 23.0 Expand from Cindy's code
