// Usage: node scripts/sync-golden.mjs <cindy checkout with node_modules> <commit>
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { ASSETS } from './golden/record.mjs';
import { renderItems } from './golden/renderItems.mjs';
import { sentText } from './golden/sentText.mjs';
import { hookCard } from './golden/hookCard.mjs';
import { markdownTree } from './golden/markdownTree.mjs';
import { markdownTransforms } from './golden/markdownTransforms.mjs';
import { sourceLabels } from './golden/sourceLabels.mjs';
import { historyRender } from './golden/historyRender.mjs';
import { workGroups } from './golden/workGroups.mjs';
import { ARTIFACT_TESTS, ATTACHMENT_TESTS, BOUNDARY_TESTS, FILE_KIND_TESTS, LINK_OPEN_TESTS, LINKIFY_TESTS, PLAN_TESTS, ROW_TESTS, SENT_TEXT_TESTS, USER_MESSAGE_TESTS, VIEW_TESTS, YIELD_TESTS, recorded } from './golden/recorded.mjs';
import { chatMessage, mapper } from './golden/chatMessage.mjs';
import { jsDate } from './golden/jsDate.mjs';
import { jsSort } from './golden/jsSort.mjs';
import { mergeMessages } from './golden/mergeMessages.mjs';
import * as workRuns from './golden/workRuns.mjs';

const [cindy, commit] = process.argv.slice(2);
if (!cindy || !commit) throw new Error('usage: sync-golden.mjs <cindy checkout> <commit>');
const fixtures = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', 'tests/fixtures');
const esbuild = createRequire(path.join(path.resolve(cindy), 'package.json'))('esbuild');
// Scratch work stays in the checkout.
const scratch = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', '.scratch');
fs.mkdirSync(scratch, { recursive: true });
const tmp = fs.mkdtempSync(path.join(scratch, 'cindy-golden-'));
const write = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 1)}\n`);

try {
  const src = 'apps/desktop/src';
  const archive = execFileSync('git', ['-C', cindy, 'archive', commit, src, 'apps/desktop/drizzle', 'packages'], { maxBuffer: 1 << 30 });
  execFileSync('tar', ['-x', '-C', tmp], { input: archive });
  const links = path.join(tmp, 'node_modules');
  for (const dir of fs.readdirSync(path.join(tmp, 'packages'))) {
    const manifest = path.join(tmp, 'packages', dir, 'package.json');
    if (!fs.existsSync(manifest)) continue;
    const { name } = JSON.parse(fs.readFileSync(manifest, 'utf8'));
    if (!name?.startsWith('@cindy/')) continue;
    fs.mkdirSync(path.join(links, '@cindy'), { recursive: true });
    fs.symlinkSync(path.join(tmp, 'packages', dir), path.join(links, name));
  }
  const entry = path.join(tmp, src, 'golden-entry.ts');
  fs.writeFileSync(entry, [
    "import './renderer/themes/colors';",
    "export { convertVsCodeTheme } from './shared/theme-import';",
    "export { normalizeLocalThemeColors } from './renderer/themes/local-themes-normalize';",
    "export * as layoutTree from './shared/layoutTree';",
    "export * as appShortcuts from './shared/appShortcuts';",
    "export * as appearance from './shared/appearanceSettings';",
    "export { cjkSeg } from './main/localDb/cjkSeg';",
    "export * as projectGrouping from './renderer/features/cc-agent/lib/projectGrouping';",
    "export * as mainListModel from './renderer/features/cc-agent/lib/mainListModel';",
    "export * as visibility from './renderer/features/cc-agent/lib/sidebarProjectVisibility';",
    "export * as usage from './renderer/lib/usageFormat';",
    "export { formatCompactTokens } from '@cindy/maker-shared/usage-format';",
    "export * as pinnedOrder from './renderer/features/cc-agent/lib/pinnedSidebarOrder';",
    "export * as filterCore from './renderer/features/cc-agent/hooks/helpers/sidebarFilterCore';",
    "export * as sourceLabel from './renderer/features/cc-agent/lib/sessionSourceLabel';",
    "export { getSessionDisplayTitle, toStoredSessionTitle } from './renderer/features/cc-agent/lib/sessionDisplayTitle';",
    "export { richAnswerText } from '@cindy/maker-shared/rich-answer';",
    "export { markdownPreviewText } from './shared/markdownPreviewText';",
    "export { finalizePlainPreview } from './main/localDb/mapper';",
    "export * as conversationSearch from './main/localDb/conversationSearch.pure';",
    "export * as chatHistoryPure from './main/localDb/chatHistorySearch.pure';",
    "export { projectMcpAppContent } from '@cindy/maker-shared/mcp-apps';",
    "export * as commandIntent from '../../../packages/maker-shared/src/commandIntent';",
    "export { groupWorkRuns } from '../../../packages/maker-shared/src/workRunGrouping';",
    "export { normalizeDisplayCommand } from '@cindy/maker-shared/command-display';",
    "export * as toolUse from '@cindy/maker-shared/tool-use-descriptor';",
    "export { formatIslandToolDetail } from './main/agent-island/toolDetail';",
    "export { createLocalizedToolRowWording } from './main/agent-island/toolWording';",
    "export { setMainLocale } from './main/i18n';",
    "export * as payloadSummary from '@cindy/maker-shared/payload-summary';",
    "export * as messageWindow from '@cindy/maker-shared/message-window';",
    "export * as syntheticTrigger from '@cindy/maker-shared/synthetic-trigger';",
    "export * as messageNormalize from '@cindy/maker-shared/message-normalize';",
    "export * as messageRender from '@cindy/maker-shared/message-render';",
    "export * as agentTask from '@cindy/maker-shared/agent-task';",
    "export { hasRichAnswer, richAnswerMediaSources } from '@cindy/maker-shared/rich-answer';",
    "export { extractToolResultMedia } from './renderer/components/chat/AgentActionRow';",
    "export { buildRenderItems, isGeneratedFilesTurnSealed } from './renderer/components/chat/MessageStream';",
    "export * as stream from './renderer/components/chat/MessageStream';",
    "export * as desktopGroups from './renderer/components/chat/messageWorkGroups';",
    "export * as generatedFiles from './renderer/lib/generatedFiles';",
    "export { resolveLocalPath, resolveToolFilePath } from './renderer/lib/localPathResolver';",
    "export { historyOutlineContent, historyOutlineContentStamp, withHistoryArtifacts } from './main/localDb/ipc/historyViewOutline';",
    "export { extractCommandOutputPathCandidates } from './shared/commandOutputPaths';",
    "export * as urlBoundary from '../../../packages/maker-shared/src/urlTextBoundary';",
    "export { findLinkifyMatches } from './renderer/components/chat/userMessageLinkify';",
    "export * as deepLink from './renderer/lib/deepLink';",
    "export * as userDisplay from './renderer/components/chat/userMessageDisplayText';",
    "export * as userCollapse from './renderer/components/chat/userMessageCollapse';",
    "export * as userMessage from './renderer/components/chat/UserMessage';",
    "export * as chatQuotes from '@cindy/maker-shared/chat-quotes';",
    "export * as linkOpen from './renderer/hooks/useLinkOpenPreference';",
    "export { resolveSafe as resolveCachedImage } from './main/imageCacheStore';",
    "export { resolveSafe as resolveBlob } from './main/cindy-media/blobStore';",
    "export * as attachmentSafety from './shared/attachmentSafety';",
    "export { isSafetyDowngradedAttachment } from './renderer/lib/chatAttachmentSave';",
    "export * as filePresentation from '../../../packages/maker-shared/src/filePresentation';",
    "export { sentAgentReferenceDisplayLabel } from './renderer/components/chat/SentAgentReferenceChip';",
    "export { slashCommandDisplayLabel } from '../../../packages/maker-shared/src/composerPalette';",
    "export { SQLiteSyncDialect } from 'drizzle-orm/sqlite-core';",
  ].join('\n'));
  // Main-process modules import Electron: `app` is a named export, `getPath` a fixed folder.
  const electronStub = path.join(tmp, 'electron-stub.cjs');
  fs.writeFileSync(electronStub, 'const target = Object.assign(function () {}, { app: 0 });\nconst any = new Proxy(target, { get: (_, key) => key === Symbol.toPrimitive ? () => "" : key === "getPath" ? () => "/user-data" : any, apply: () => any, construct: () => any });\nmodule.exports = any;\n');
  const bundle = path.join(tmp, 'bundle.mjs');
  const define = { 'import.meta.env': JSON.stringify({ PROD: true, DEV: false, MODE: 'production' }) };
  const nodePaths = [links, path.join(path.resolve(cindy), 'node_modules')];
  const alias = { '@': path.join(tmp, src, 'renderer'), electron: electronStub };
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, ...ASSETS,
    // The sent-text golden runs `renderContent`, whose JSX needs its runtime.
    define, jsx: 'automatic',
  });
  const { convertVsCodeTheme, normalizeLocalThemeColors, layoutTree, appShortcuts, appearance, cjkSeg, projectGrouping, mainListModel, visibility, pinnedOrder, filterCore, sourceLabel, usage, formatCompactTokens, getSessionDisplayTitle, toStoredSessionTitle, richAnswerText, markdownPreviewText, finalizePlainPreview, conversationSearch, chatHistoryPure, projectMcpAppContent, commandIntent, groupWorkRuns, normalizeDisplayCommand, toolUse, formatIslandToolDetail, createLocalizedToolRowWording, setMainLocale, payloadSummary, messageWindow, syntheticTrigger, messageNormalize, messageRender, agentTask, hasRichAnswer, richAnswerMediaSources, extractToolResultMedia, buildRenderItems, isGeneratedFilesTurnSealed, stream, desktopGroups, generatedFiles, resolveLocalPath, resolveToolFilePath, historyOutlineContent, historyOutlineContentStamp, withHistoryArtifacts, extractCommandOutputPathCandidates, urlBoundary, findLinkifyMatches, deepLink, userDisplay, userCollapse, userMessage, chatQuotes, linkOpen, filePresentation, attachmentSafety, isSafetyDowngradedAttachment, resolveCachedImage, resolveBlob, sentAgentReferenceDisplayLabel, slashCommandDisplayLabel, SQLiteSyncDialect } = await import(pathToFileURL(bundle).href);

  const importDir = path.join(fixtures, 'theme-import');
  fs.rmSync(path.join(importDir, 'expected'), { recursive: true, force: true });
  fs.mkdirSync(path.join(importDir, 'expected'));
  const inputs = fs.readdirSync(path.join(importDir, 'input')).sort();
  for (const file of inputs) {
    const raw = fs.readFileSync(path.join(importDir, 'input', file), 'utf8');
    let expected;
    try {
      const result = convertVsCodeTheme(raw, path.parse(file).name);
      expected = result && {
        themes: result.themes.map((t) => ({ name: t.name, type: t.type, colors: Object.entries(t.colors) })),
        report: result.report,
      };
    } catch (error) {
      expected = { error: String(error.message) };
    }
    write(path.join(importDir, 'expected', `${path.parse(file).name}.json`), expected);
  }

  const localDir = path.join(fixtures, 'local-themes');
  const cases = JSON.parse(fs.readFileSync(path.join(localDir, 'normalize-input.json'), 'utf8'));
  write(path.join(localDir, 'normalize-expected.json'), cases.map((c) => ({
    name: c.name,
    colors: normalizeLocalThemeColors(c.colors, c.type),
  })));

  const layoutDir = path.join(fixtures, 'layout');
  const layout = JSON.parse(fs.readFileSync(path.join(layoutDir, 'input.json'), 'utf8'));
  const num = (a) => ({ NaN, Infinity }[a] ?? a);
  write(path.join(layoutDir, 'expected.json'), {
    coerce: layout.coerce.map((c) => {
      let raw = null;
      try {
        raw = 'text' in c ? JSON.parse(c.text) : c.raw;
      } catch {}
      return { name: c.name, ...layoutTree.coerceLayout(raw) };
    }),
    ops: layout.ops.map((c) => {
      const result = layoutTree[c.op](layout.bases[c.base], ...c.args.map(num));
      const panes = c.op === 'walkPanes' && result.map((p) => ({ id: p.id, panelKind: p.panelKind, collapsed: p.collapsed ?? null }));
      return { name: c.name, result: panes || (result ?? null) };
    }),
    kinds: layout.kinds.map((k) => layoutTree.tabGroupIdFromPanelKind(k)),
    groups: layoutTree.TAB_GROUP_IDS.map((g) => [g, layoutTree.tabGroupPanelKind(g)]),
  });

  const keysDir = path.join(fixtures, 'app-shortcuts');
  const keys = JSON.parse(fs.readFileSync(path.join(keysDir, 'input.json'), 'utf8'));
  const sc = appShortcuts;
  write(path.join(keysDir, 'expected.json'), keys.platforms.map((platform) => ({
    platform,
    definitions: sc.APP_SHORTCUT_DEFINITIONS.map((d) => ({
      id: d.id,
      scope: d.scope,
      rebindable: d.rebindable,
      hidden: Boolean(d.hiddenInSettings),
      available: sc.isAppShortcutAvailableOnPlatform(d.id, platform),
      defaults: d.getDefaultCombos(platform),
    })),
    overrides: keys.overrides.map((c) => {
      const normalized = sc.normalizeAppShortcutOverrides('text' in c ? JSON.parse(c.text) : c.raw, platform);
      return {
        name: c.name,
        normalized,
        effective: [...sc.getEffectiveAppShortcuts(normalized, platform)],
        yielded: [...sc.getEffectiveAppShortcuts(normalized, platform, keys.yieldTo)],
      };
    }),
    formats: keys.formats.map((f) => sc.formatAppShortcutCombo(f, platform)),
    overlaps: ['app', 'workdir-doc', 'browser', 'composer', 'global'].flatMap((x, _, all) =>
      all.map((y) => [x, y, sc.appShortcutScopesOverlap(x, y)])),
  })));

  const appearanceDir = path.join(fixtures, 'appearance');
  const looks = JSON.parse(fs.readFileSync(path.join(appearanceDir, 'input.json'), 'utf8'));
  write(path.join(appearanceDir, 'expected.json'), looks.map((c) => {
    const s = appearance.normalizeAppearanceSettings(JSON.parse(c.text));
    return {
      name: c.name,
      wallpaperId: s.wallpaperId,
      wallpaperOverlay: s.wallpaperOverlay,
      wallpaperVisibility: s.wallpaperVisibility ?? null,
      wallpaperBlur: s.wallpaperBlur ?? null,
      visibility: [false, true].map((dark) => appearance.getWallpaperVisibility(s, dark)),
    };
  }));

  const drizzle = path.join(tmp, 'apps/desktop/drizzle');
  const hash = (file) => createHash('sha256').update(fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n')).digest('hex');
  const migrations = fs.readdirSync(drizzle).filter((f) => /^\d{4}_.*\.sql$/.test(f)).sort().map((fileName) => {
    const script = path.join(drizzle, 'scripts', fileName.replace(/\.sql$/, '.ts'));
    return {
      seq: parseInt(fileName.slice(0, 4), 10),
      fileName,
      sqlHash: hash(path.join(drizzle, fileName)),
      scriptHash: fs.existsSync(script) ? hash(script) : null,
    };
  });
  fs.writeFileSync(path.join(fixtures, '..', '..', 'resources', 'migration-manifest.json'), `${JSON.stringify(migrations, null, 1)}\n`);

  const ranges = (re) => {
    const out = [];
    for (let cp = 0; cp <= 0x10ffff; cp += 1) {
      if (cp >= 0xd800 && cp <= 0xdfff) continue;
      if (!re.test(String.fromCodePoint(cp))) continue;
      const last = out.at(-1);
      if (last && last[1] === cp - 1) last[1] = cp;
      else out.push([cp, cp]);
    }
    return out;
  };
  write(path.join(fixtures, '..', '..', 'resources', 'cjk-classes.json'), {
    unicode: process.versions.unicode,
    han: ranges(/\p{Script=Han}/u),
    letterOrNumber: ranges(/[\p{L}\p{N}]/u),
    mark: ranges(/\p{M}/u),
  });
  write(path.join(fixtures, '..', '..', 'resources', 'url-classes.json'), {
    unicode: process.versions.unicode,
    boundary: ranges(/[\p{P}\p{Z}]/u),
    letter: ranges(/\p{L}/u),
  });
  const segDir = path.join(fixtures, 'cjk-seg');
  const segInput = JSON.parse(fs.readFileSync(path.join(segDir, 'input.json'), 'utf8'));
  write(path.join(segDir, 'expected.json'), segInput.map((text) => cjkSeg(text)));

  const groupDir = path.join(fixtures, 'grouping');
  const groupCases = JSON.parse(fs.readFileSync(path.join(groupDir, 'input.json'), 'utf8'));
  // CCAgentSidebarUpper's composition, local tasks, no filters beyond status.
  const ids = (list) => list.map((x) => x.id);
  const shown = (e) => e.kind === 'project'
    ? { kind: e.kind, key: e.project.projectKey, name: e.project.displayName, workingDir: e.project.workingDir, sessions: ids(e.project.sessions) }
    : e.kind === 'session' ? { kind: e.kind, session: e.session.id }
    : { kind: e.kind, sessions: ids(e.sessions) };
  write(path.join(groupDir, 'expected.json'), groupCases.map((c) => {
    const platform = c.platform ?? 'linux';
    const sortBy = c.sortBy ?? 'recency';
    const hidden = new Set(c.hiddenProjectKeys ?? []);
    const order = c.pinnedOrder ?? [];
    const persistent = projectGrouping.buildPersistentLocalProjects(c.recentWorkdirs ?? [], c.sessions, platform);
    const options = (pins) => ({
      projectAliases: c.aliases ?? {}, persistentLocalProjects: persistent, localPlatform: platform,
      includePinnedInProjects: pins,
    });
    const sidebar = visibility.sidebarSessionsWithHiddenProjectsAsDialogues(c.sessions, hidden, platform);
    const cutoff = c.lastActivityCutoff ? Date.parse(c.lastActivityCutoff) : null;
    const windowed = cutoff === null ? sidebar : sidebar.filter((s) => Date.parse(s.lastActivityAt ?? s.userSendAt ?? s.updatedAt) >= cutoff);
    const windowOptions = (pins) => ({ ...options(pins), persistentLocalProjects: cutoff === null ? persistent : [] });
    const plain = projectGrouping.groupSessions(sidebar, options(false));
    const withPins = projectGrouping.groupSessions(sidebar, options(true));
    const main = projectGrouping.groupSessions(windowed, windowOptions(false));
    const mainWithPins = projectGrouping.groupSessions(windowed, windowOptions(true));
    const universe = visibility.visibleSidebarProjects(projectGrouping.groupSessions(c.sessions, options(true)).projects, hidden, platform);
    const pinnedKeys = visibility.buildProjectKeyComparisonSet(
      order.map(pinnedOrder.projectKeyFromPinnedEntryId).filter(Boolean), platform);
    const unpinned = (p) => ({ ...p, sessions: p.sessions.filter((s) => s.pinnedAt == null) });
    const pinnedProjects = visibility.visibleSidebarProjects(withPins.projects, hidden, platform)
      .filter((p) => visibility.projectKeyComparisonSetHas(pinnedKeys, p.projectKey, platform));
    let pinned = [
      ...plain.pinned.map((s) => ({ kind: 'session', id: s.id, session: s })),
      ...pinnedProjects.map((p) => ({ kind: 'project', id: pinnedOrder.pinnedProjectEntryId(p.projectKey), project: p,
        displaySessions: mainListModel.sortSessionsForMainList(unpinned(p).sessions, sortBy) })),
    ];
    if (order.length > 0) {
      const rank = pinnedOrder.buildPinnedSidebarRank(order, platform);
      const at = (e) => rank.get(pinnedOrder.pinnedSidebarEntryComparisonKey(e.id, platform)) ?? Number.MAX_SAFE_INTEGER;
      pinned = pinned.sort((x, y) => at(x) - at(y));
    }
    const labels = sourceLabel.buildSessionSourceLabelMap(plain.pinned, universe, 'Chat');
    const vendor = c.vendor ?? 'all';
    const matches = (s) => vendor === 'all' || (s.agentKind ?? 'cc') === vendor;
    const mainProjects = visibility.visibleSidebarProjects(mainWithPins.projects, hidden, platform)
      .filter((p) => !visibility.projectKeyComparisonSetHas(pinnedKeys, p.projectKey, platform))
      .flatMap((p) => {
        const matching = p.sessions.filter(matches);
        if (matching.length === 0 && (!p.isPersistentLocal || (vendor !== 'all' && !projectGrouping.persistentProjectMatchesVendor(p, vendor)))) return [];
        return [{ ...p, sessions: matching.filter((s) => s.pinnedAt == null) }];
      });
    const entries = mainListModel.buildMainListEntries({
      projects: mainProjects, dialogues: main.dialogues.filter(matches), unclassified: main.unclassified.filter(matches),
      groupBy: c.groupBy ?? 'project', groupDialogue: c.groupDialogue ?? true, sortBy,
      projectOrder: c.projectOrder ?? 'activity', manualProjectOrder: c.manualProjectOrder ?? [],
    });
    return {
      name: c.name,
      pinned: pinned.map((e) => e.kind === 'session'
        ? { kind: 'session', session: e.session.id, label: labels.get(e.session.id) ?? null }
        : { kind: 'project', key: e.project.projectKey, name: e.project.displayName, workingDir: e.project.workingDir, sessions: ids(e.displaySessions) }),
      entries: entries.map(shown),
    };
  }));

  // handlePinnedReorder and setManualPinnedOrder for a drag's shown order.
  const reorderDir = path.join(fixtures, 'pinned-reorder');
  const reorderCases = JSON.parse(fs.readFileSync(path.join(reorderDir, 'input.json'), 'utf8'));
  write(path.join(reorderDir, 'expected.json'), reorderCases.map((c) => {
    const platform = c.platform ?? 'linux';
    const key = (id) => pinnedOrder.pinnedSidebarEntryComparisonKey(id, platform);
    const pinnedIds = projectGrouping.pinnedSessionIdsInDisplayOrder(c.sessions);
    const active = pinnedOrder.activePinnedSidebarEntryIds(c.manual, pinnedIds, platform);
    const { baseOrder, order } = filterCore.buildManualPinnedReorder(c.manual, active, c.visible, key);
    return { name: c.name, active, base: baseOrder, next: filterCore.normalizeManualPinnedOrder(order, active, key) };
  }));

  // SessionInfoMeta's token and money text.
  const tokens = [0, 999, 1000, 1049, 1050, 999_949, 999_950, 1_234_567, 999_950_000, 2e12];
  const amounts = [0.004, 0.125, 1.005, 9.995, 10, 12.5, 999.5, 1000, 12_345.6];
  write(path.join(fixtures, 'meta.json'), {
    tokens: tokens.map((n) => [n, formatCompactTokens(n)]),
    money: ['USD', 'CNY'].flatMap((currency) =>
      amounts.map((amount) => [currency, amount, usage.formatMoney({ amount, currency, approximate: false, kind: 'actual-cost' })])),
  });

  // SessionItem's display title, with English text and UTC times.
  process.env.TZ = 'UTC';
  const en = JSON.parse(fs.readFileSync(path.join(tmp, src, 'renderer/i18n/locales/en/common.json'), 'utf8'));
  const english = (key) => key.split('.').reduce((node, part) => node?.[part], en);
  const titles = ['New Maker', 'new maker', '[Schedule] nightly', '[Schedule]nightly', 'Fix login', '[Cindy Make] Fix login',
    '[cindy-make]   Fix login', '[ab12] Fix login', '[ab12]', 'Resolve Integration Conflicts', '处理源码更新冲突',
    '[Cindy-Make] 処理', 'cindyMake.merge.taskTitle', '[ab12] Resolve Source Update Conflicts',
    '[Cindy Make]\u0085Fix', '[ab12]\u0085\u2028Fix', '[ab12]\ufeff Fix'];
  const sources = ['desktop', 'scheduler', 'cindy-make', 'cindy-make-merge'];
  const dirs = [null, '/repo', '/home/u/.cindy/worktrees/ab12cd34', 'C:\\x\\Merge-Worktrees\\ab12-z\\', '/x/worktrees/a_b'];
  write(path.join(fixtures, 'display-title.json'), titles.flatMap((title) => sources.flatMap((source) => dirs.map((workingDir) => {
    const session = { title, source, workingDir, createdAt: '2026-03-04T05:06:07.000Z' };
    const unnamed = english(source === 'cindy-make' ? 'cindyMake.code.taskName' : 'ccAgent.common.unnamedSession');
    return [title, source, workingDir, getSessionDisplayTitle(session, unnamed, english)];
  }))));

  const edits = ['Fix', '[Schedule] Fix', '[ab12] Fix', '[Cindy Make] Fix', '', '\u0085', '[ab12]\u0085Fix'];
  write(path.join(fixtures, 'stored-title.json'), ['New Maker', '[Schedule] nightly', 'Fix login'].flatMap((title) =>
    sources.flatMap((source) => dirs.flatMap((workingDir) => edits.map((edited) =>
      [title, source, workingDir, edited, toStoredSessionTitle({ title, source, workingDir }, edited)])))));

  // The task preview: rich answers, Markdown and the mapper's finalize step.
  const previewDir = path.join(fixtures, 'preview');
  const previewCases = JSON.parse(fs.readFileSync(path.join(previewDir, 'cases.json'), 'utf8'));
  write(path.join(previewDir, 'expected.json'), {
    rich: previewCases.rich.map((source) => [source, richAnswerText(source)]),
    markdown: previewCases.markdown.map((source) => [source, markdownPreviewText(source)]),
    finalize: previewCases.finalize.map(([text, role]) => [text, role, finalizePlainPreview(text, role)]),
  });

  // conversationSearch.pure: title matches, visible text, previews and the merged order.
  const searchDir = path.join(fixtures, 'search');
  const search = JSON.parse(fs.readFileSync(path.join(searchDir, 'cases.json'), 'utf8'));
  const cs = conversationSearch;
  const item = (r) => ({
    id: r.session.id, matchKind: r.matchKind, titleMatchIndices: r.titleMatchIndices,
    titleScore: r.titleScore === null ? null : String(r.titleScore),
    contentHit: r.contentHit?.messageId ?? null, contentHits: r.contentHits.map((h) => h.messageId),
    rankScore: String(r.rankScore),
  });
  write(path.join(searchDir, 'expected.json'), {
    fuzzy: search.fuzzy.map(([title, query]) => {
      const m = cs.fuzzyTitleMatch(title, query);
      return m && { score: String(m.score), indices: m.indices };
    }),
    preview: search.preview.map(([role, content, query, max, meta]) =>
      cs.normalizeConversationContentPreview(role, content, query, max ?? undefined, meta)),
    visible: search.visible.map(([role, content, preserveWhitespace]) =>
      cs.visibleMessageText(role, content, { preserveWhitespace })),
    fts: {
      tokens: search.fts.tokens.map((q) => [chatHistoryPure.extractMessagesFtsTokens(q), chatHistoryPure.buildMessagesFtsMatch(q)]),
      visible: search.fts.visible.map(([text, q]) => chatHistoryPure.visibleTextMatchesMessagesFtsQuery(text, q)),
      fuse: search.fts.fuse.map((ids) => chatHistoryPure.fuseRRF(ids, []).map((e) => [e.messageId, String(e.score), e.ftsRank])),
    },
    mcpApp: search.mcpApp.map(([content, marked]) => projectMcpAppContent(content, marked) ?? null),
    merge: search.merge.map((c) => Object.fromEntries(['relevance', 'activityDesc', 'activityAsc'].map((sortBy) =>
      [sortBy, cs.mergeConversationSearchResults({ ...structuredClone(c), sortBy }).map(item)]))),
  });

  // commandIntent and normalizeDisplayCommand on the inputs of Cindy's own tests.
  const commandDir = path.join(fixtures, 'command-intent');
  const commandCases = JSON.parse(fs.readFileSync(path.join(commandDir, 'cases.json'), 'utf8'));
  const ci = commandIntent;
  write(path.join(commandDir, 'expected.json'), {
    command: commandCases.command.map((c) => [c, ci.commandIntentFromCommand(c) ?? null]),
    actions: commandCases.actions.map((a) => [a, ci.commandIntentFromActions(a) ?? null]),
    intents: commandCases.intents.map((args) => [args, ci.commandIntentsFromActions(...args)]),
    display: commandCases.display.map((c) => [c, normalizeDisplayCommand(c)]),
  });

  // describeToolUse, kept to the fields the row label reads; a cut pair shows as U+FFFD.
  const toolDir = path.join(fixtures, 'tool-use');
  const toolCases = JSON.parse(fs.readFileSync(path.join(toolDir, 'cases.json'), 'utf8'));
  const labelFields = {
    command: ['description', 'command', 'intent'], file: ['action', 'filePath', 'fileName'], fileChange: ['changes'],
    search: ['pattern'], web: ['mode', 'target'], todo: [], task: ['description'],
    mcp: ['serverLabel', 'toolLabel', 'createdPath', 'sourceCandidates'], dynamic: ['namespace', 'toolLabel'], collab: ['toolLabel'], generic: [],
  };
  const pick = (o, keys) => Object.fromEntries(keys.filter((k) => o[k] !== undefined).map((k) => [k, o[k]]));
  const labelled = (d) => ({
    kind: d.kind, toolName: d.toolName, ...pick(d, labelFields[d.kind]),
    ...(d.kind === 'fileChange' ? { changes: d.changes.map((c) => pick(c, ['action', 'path', 'fileName', 'moveFileName'])) } : {}),
  });
  write(path.join(toolDir, 'expected.json'), {
    describe: toolCases.describeToolUse.map(([name, input]) => [name, input, labelled(toolUse.describeToolUse(name, input ?? undefined))]),
    parse: toolCases.parseToolName.map(([name]) => [name, toolUse.parseToolName(name)]),
    truncate: toolCases.truncateToolText.map(([text, max]) => [text, max, toolUse.truncateToolText(text, max).toWellFormed()]),
    humanize: toolCases.humanizeToolToken.map(([token]) => [token, toolUse.humanizeToolToken(token)]),
  });
  // The interrupted dialog's activity, as `interrupted-resume-candidates` builds it, per locale.
  const portLocales = fs.readdirSync(path.join(fixtures, '..', '..', 'locales')).filter((l) => l !== 'COMMIT').sort();
  write(path.join(toolDir, 'activity.json'), portLocales.map((locale) => {
    setMainLocale(locale);
    const wording = createLocalizedToolRowWording();
    return [locale, toolCases.island.map(([name, input]) =>
      (formatIslandToolDetail(name, input ?? undefined, { wording }) ?? name).toWellFormed())];
  }));

  const golden = { esbuild, tmp, nodePaths, alias, fixtures, write };
  const rowCases = await recorded({ ...golden, name: 'history-rows', tests: ROW_TESTS, fns: {
    extractPayloadToolResultMedia: payloadSummary.extractPayloadToolResultMedia,
    extractPayloadToolResultFiles: payloadSummary.extractPayloadToolResultFiles,
    extractPayloadToolCardIds: payloadSummary.extractPayloadToolCardIds,
    hasVisibleHistoryResult: messageWindow.hasVisibleHistoryResult,
    isContinuationMessage: syntheticTrigger.isContinuationMessage,
    parseMessageToolUse: messageNormalize.parseMessageToolUse,
    messageContentToPreview: messageNormalize.messageContentToPreview,
    isOrcaCommunicationTool: messageNormalize.isOrcaCommunicationTool,
    isAgentTaskToolName: agentTask.isAgentTaskToolName,
    isAgentPlanToolName: messageRender.isAgentPlanToolName,
    isSteerUserRow: messageRender.isSteerUserRow,
    isDeliveryProseText: messageRender.isDeliveryProseText,
    hasRichAnswer,
  } });
  console.log(`${rowCases} history row cases`);
  const viewCases = await recorded({ ...golden, name: 'history-view', tests: VIEW_TESTS, fns: {
    projectHistoryView: messageWindow.projectHistoryView,
    historySubagentScopes: (rows, root) => Object.fromEntries(messageWindow.historySubagentScopes(rows, root)),
    completesHistoryTurn: messageWindow.completesHistoryTurn,
  } });
  console.log(`${viewCases} history view cases`);
  const runCases = await recorded({ ...golden, name: 'work-runs', tests: {}, fns: { groupWorkRuns: workRuns.run(groupWorkRuns) } });
  console.log(`${runCases} work run cases`);
  const artifactCases = await recorded({ ...golden, name: 'history-artifacts', tests: ARTIFACT_TESTS, fns: {
    extractCommandOutputPathCandidates, withHistoryArtifacts,
  } });
  console.log(`${artifactCases} history artifact cases`);
  // A throw is recorded as its message.
  const thrown = (fn) => (...args) => {
    try { return fn(...args); } catch (error) { return { throws: error.message }; }
  };
  const planCases = await recorded({ ...golden, name: 'plan-timeline', tests: PLAN_TESTS, fns: {
    findMessageTodoInsertions: thrown((...args) => [...messageRender.findMessageTodoInsertions(...args)]),
    getLatestMessageTodoState: thrown(messageRender.getLatestMessageTodoState),
    findLatestMessageTodoInsertion: thrown(messageRender.findLatestMessageTodoInsertion),
    extractPlanTodos: thrown(messageRender.extractPlanTodos),
    isPlanUserBoundary: messageRender.isPlanUserBoundary,
  } });
  console.log(`${planCases} plan timeline cases`);
  // Keys in one order, a cut pair as U+FFFD, `undefined` for an absent argument.
  const keyed = (keys) => (o) => JSON.parse(JSON.stringify(Object.fromEntries(keys.map((k) => [k, o[k]])),
    (_, v) => (typeof v === 'string' ? v.toWellFormed() : v)));
  const media = keyed(['kind', 'url', 'modelFile', 'audioTrack', 'audioInCard', 'imageInCard']);
  const yieldCases = await recorded({ ...golden, define, name: 'turn-yields', tests: YIELD_TESTS, fns: {
    extractToolResultMedia: (result) => extractToolResultMedia(result).map(media),
    dedupeToolMediaByUrl: (list) => messageRender.dedupeToolMediaByUrl(list).map(media),
    richAnswerMediaSources: (source) => [...richAnswerMediaSources(source)],
    collectGeneratedFiles: (rows, dir) => generatedFiles.collectGeneratedFiles(rows, dir)
      .map(keyed(['path', 'name', 'source', 'ready', 'artifact', 'artifactConfirmed'])),
    extractDocumentArtifactMetadata: (name, input, result) =>
      keyed(['value'])({ value: generatedFiles.extractDocumentArtifactMetadata(name, input, result ?? undefined) }).value ?? null,
    isExplicitFailedToolResult: (content) => generatedFiles.isExplicitFailedToolResult(content ?? undefined),
    resolveLocalPath, resolveToolFilePath,
    // The input as JSON text: its key order is the case.
    sourcePathCandidates: (name, raw) => toolUse.sourcePathCandidatesFromDescriptor(toolUse.describeToolUse(name, JSON.parse(raw))),
  } });
  console.log(`${yieldCases} turn yield cases`);
  const boundaryCases = await recorded({ ...golden, name: 'url-boundary', tests: BOUNDARY_TESTS, fns: {
    clipBareHttpAutolinkText: urlBoundary.clipBareHttpAutolinkText,
    matchBareHttp: (text) => text.match(new RegExp(urlBoundary.BARE_HTTP_URL_RE_SOURCE, 'g')) ?? [],
    markdownWrapMarkerFromPrefix: urlBoundary.markdownWrapMarkerFromPrefix,
  } });
  console.log(`${boundaryCases} URL boundary cases`);
  const linkifyCases = await recorded({ ...golden, name: 'linkify', tests: LINKIFY_TESTS, fns: {
    findLinkifyMatches,
    parseSessionDeepLinkHref: deepLink.parseSessionDeepLinkHref,
    parseProjectDeepLinkHref: deepLink.parseProjectDeepLinkHref,
    findMarkdownLabelStart: deepLink.findMarkdownLabelStart,
    unescapeMarkdownLabelBrackets: deepLink.unescapeMarkdownLabelBrackets,
  } });
  console.log(`${linkifyCases} linkify cases`);
  const userMessageCases = await recorded({ ...golden, name: 'user-message', tests: USER_MESSAGE_TESTS, fns: {
    resolveUserDisplayText: userDisplay.resolveUserDisplayText,
    resolveHookGroupContext: userDisplay.resolveHookGroupContext,
    parseOrcaCommunicationContent: userDisplay.parseOrcaCommunicationContent,
    mayExceedVisualLineThreshold: userCollapse.mayExceedVisualLineThreshold,
  } });
  console.log(`${userMessageCases} user message cases`);
  const sentTextCases = await recorded({ ...golden, name: 'sent-text', tests: SENT_TEXT_TESTS,
    fns: sentText({ userMessage, chatQuotes, sentAgentReferenceDisplayLabel, slashCommandDisplayLabel }) });
  console.log(`${sentTextCases} sent text cases`);
  const linkOpenCases = await recorded({ ...golden, name: 'link-open', tests: LINK_OPEN_TESTS,
    fns: { isLocalOpenUrl: linkOpen.isLocalOpenUrl } });
  console.log(`${linkOpenCases} link open cases`);
  const fileKindCases = await recorded({ ...golden, name: 'file-presentation', tests: FILE_KIND_TESTS, fns: {
    getFileVisualKind: filePresentation.getFileVisualKind,
    glyph: (input) => filePresentation.FILE_VISUAL_GLYPHS[filePresentation.getFileVisualKind(input)],
  } });
  console.log(`${fileKindCases} file presentation cases`);
  const attachmentCases = await recorded({ ...golden, name: 'attachment-safety', tests: ATTACHMENT_TESTS, fns: {
    attachmentExtension: attachmentSafety.attachmentExtension,
    isDangerousAttachmentName: attachmentSafety.isDangerousAttachmentName,
    isSafetyDowngradedAttachment,
  } });
  console.log(`${attachmentCases} attachment safety cases`);
  // Where an address resolves under the stub's userData, or why it does not.
  const placed = (resolveSafe) => (url) => {
    try { return path.relative('/user-data', resolveSafe(url).absPath); } catch (error) { return `refused: ${error.message}`; }
  };
  const mediaCases = await recorded({ ...golden, name: 'media-store', tests: {},
    fns: { cached: placed(resolveCachedImage), blob: placed(resolveBlob) } });
  console.log(`${mediaCases} media store cases`);
  console.log(`${await sourceLabels({ ...golden, define, assets: ASSETS, cindy, src })} source label cases`);
  console.log(`${await hookCard({ ...golden, define, assets: ASSETS, cindy, src })} hook card cases`);
  console.log(`${await markdownTree({ ...golden, define, assets: ASSETS, src })} markdown tree cases`);
  console.log(`${await markdownTransforms({ ...golden, define, assets: ASSETS, src })} markdown transform cases`);
  console.log(`${await renderItems({ ...golden, define }, { buildRenderItems, isGeneratedFilesTurnSealed })} render item cases`);
  console.log(`${workGroups(golden, { stream, groups: desktopGroups })} work group cases`);
  const mapServerMessages = await mapper(golden);
  console.log(`${historyRender(golden, { stream, messageWindow, mapServerMessages })} history render cases`);
  console.log(`${chatMessage(golden, mapServerMessages)} chat message cases`);
  console.log(`${await mergeMessages(golden)} merge cases`);
  console.log(`${jsDate({ cindy, fixtures, write })} date parse cases`);
  console.log(`${jsSort({ fixtures })} sort cases`);
  const dialect = new SQLiteSyncDialect();
  write(path.join(fixtures, 'history-outline', 'sql.json'), {
    content: dialect.sqlToQuery(historyOutlineContent()),
    stamp: dialect.sqlToQuery(historyOutlineContentStamp()),
  });

  const full = execFileSync('git', ['-C', cindy, 'rev-parse', `${commit}^{commit}`]).toString().trim();
  fs.writeFileSync(path.join(fixtures, 'COMMIT'), `${full}\n`);
  fs.rmSync(path.join(importDir, 'COMMIT'), { force: true });
  console.log(`golden from ${full}: ${inputs.length} imports, ${cases.length} normalize cases, ${layout.coerce.length + layout.ops.length} layout cases, ${keys.overrides.length} shortcut cases`);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
// The stores the source-labels golden mounted keep timers; the work is done.
process.exit(0);
