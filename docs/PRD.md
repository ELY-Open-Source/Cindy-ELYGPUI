# Cindy-ELYGPUI PRD

Cindy-ELYGPUI is Cindy's desktop client rebuilt in Rust on GPUI and Ely GPUI Components.
The reference is Cindy at `makecindy/cindy` commit `1ac47f30aeca5c766d4cd7f2add01e77e666de08`
(2026-11-08, the merge of the Omarchy live theme). Where this file and Cindy disagree, Cindy's code wins, and this file is corrected.

## What 1:1 means

- The user sees the same product: the same screens, flows, copy in the same 12 locales, and themes.
- Cindy's persisted data opens unchanged: `~/.cindy/` files, the desktop SQLite database and its
  migration history, local themes, plugins, settings.
- Wire protocols stay compatible: the Cindy mobile app, Cindy servers, remote daemons and IM
  channels talk to this client as they talk to Cindy's.
- Cindy's internal layout does not carry over. Electron's main, preload and renderer collapse into
  one GPUI process; IPC channels inside the desktop become function calls. React, Vite and Electron
  Forge have no counterpart.
- The mobile app stays Cindy's Expo app. GPUI has no mobile target; this client serves it through
  device-link.

## Platforms

Linux, macOS and Windows, each on x86_64 and arm64. CI builds and tests all six.

## Chapters

Each chapter is ported in order. A chapter's first task expands it into tasks in `TASKS.md`,
read from Cindy's code and rule docs for that area.

1. **Foundation.** Cargo crate, Ely window, logging, CI on six targets, license and notice.
2. **Logging and diagnostics.** Cindy's log files, line format, redaction and crash markers
   (`docs/dev-rules/log-upload-and-redaction.md`).
3. **Configuration and storage.** `~/.cindy/` layout, userData paths, the desktop SQLite schema read
   through Cindy's existing migrations, settings with default and override kept apart
   (`configuration-and-overrides.md`, `database-and-migrations.md`, `credentials-and-local-storage.md`).
4. **i18n.** Cindy's 12 locales and `common.json` keys, glossary, locale fallback.
5. **Themes and appearance.** Built-in families, light, dark and system modes, local themes,
   VS Code and Obsidian import, the Omarchy live theme, fonts and sizes (`DESIGN.md` §10).
6. **Window shell and layout.** Window chrome, the left sidebar, the split layout tree with one
   `chat-main`, tab groups, the right sidebar, navigation between chat and settings.
7. **Mock mode.** `--mock`: Cindy's mock sessions and data, no login (`ui-mock-data.md`).
8. **Sessions.** Session list, projects and folders, pin, archive, search, interrupted-task resume.
9. **Chat view.** Message rendering (Markdown, code, math, diagrams, tool calls, artifacts),
   composer, attachments, permission modes, queue and interjection, stop and continue.
10. **Agent runtime.** maker-core's engines (Claude Code, Codex, Pi) driven through Cindy's pinned
    CLIs, event stream, input coordinator, permissions, usage.
11. **Models and providers.** Model catalog, providers, defaults, recommendations, compat proxies.
12. **Settings.** Every settings page and the settings search.
13. **Account and region.** Cindy auth client, sign-in, `cn` and `global` editions, quota and usage.
14. **Teammates.** Bots, cloud companions, group chat, teammate authority.
15. **Automations.** Scheduler, webhook triggers, usage-limit auto continue.
16. **Plugins.** `.cindy` runtime, sandbox, capability slots, library storage, Ghost discovery;
    installed plugins keep working (`plugin-security-and-authoring.md` §5).
17. **Tools and integrations.** MCP servers, lizi-mcps, cindy-tools, browser use, input handoff.
18. **Files and git.** File browser, ripgrep search, git safety, worktrees, review (`/review`).
19. **Remote.** SSH workspaces, remote file service, cc-manager and pi-manager daemons, machines.
20. **Device link.** Mobile remote control, relay, shared tasks, provider sharing and groups
    (`protocol-compatibility.md`).
21. **IM channels.** Telegram, Feishu, WeChat iLink, Slack hook, permission confirmation.
22. **Desktop extras.** Voice input, screenshots and recording, remote desktop, menu bar quota,
    tray, setup migration, local agent API.
23. **Distribution.** Updater, packaging and signing on the six targets.

## Out of scope

- Cindy's server, which lives in its own repository.
- Cindy's mobile app.
- Dev-build console helpers on `window`, such as `__cindyLayout`: Rust has no DevTools console.
