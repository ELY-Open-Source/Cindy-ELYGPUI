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
- [ ] 5.7 `appearance-settings.json` read as `main/appearance-settings-store.ts` does (UI, code and
  terminal font sizes and families, wallpaper fields), applied to Ely's font scale and families

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

- [ ] 22.0 Expand from Cindy's code

## 23. Distribution

- [ ] 23.0 Expand from Cindy's code
- [ ] 23.last Dispatch CI and get all six targets green
