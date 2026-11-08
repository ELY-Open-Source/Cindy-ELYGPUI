// Usage: node scripts/sync-golden.mjs <cindy checkout with node_modules> <commit>
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [cindy, commit] = process.argv.slice(2);
if (!cindy || !commit) throw new Error('usage: sync-golden.mjs <cindy checkout> <commit>');
const fixtures = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', 'tests/fixtures');
const esbuild = createRequire(path.join(path.resolve(cindy), 'package.json'))('esbuild');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cindy-golden-'));
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
    "export * as pinnedOrder from './renderer/features/cc-agent/lib/pinnedSidebarOrder';",
    "export * as sourceLabel from './renderer/features/cc-agent/lib/sessionSourceLabel';",
  ].join('\n'));
  const bundle = path.join(tmp, 'bundle.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths: [links, path.join(path.resolve(cindy), 'node_modules')],
    alias: { '@': path.join(tmp, src, 'renderer') },
    define: { 'import.meta.env': JSON.stringify({ PROD: true, DEV: false, MODE: 'production' }) },
  });
  const { convertVsCodeTheme, normalizeLocalThemeColors, layoutTree, appShortcuts, appearance, cjkSeg, projectGrouping, mainListModel, visibility, pinnedOrder, sourceLabel } = await import(pathToFileURL(bundle).href);

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
  const segDir = path.join(fixtures, 'cjk-seg');
  const segInput = JSON.parse(fs.readFileSync(path.join(segDir, 'input.json'), 'utf8'));
  write(path.join(segDir, 'expected.json'), segInput.map((text) => cjkSeg(text)));

  const groupDir = path.join(fixtures, 'grouping');
  const groupCases = JSON.parse(fs.readFileSync(path.join(groupDir, 'input.json'), 'utf8'));
  // CCAgentSidebarUpper's composition, local tasks, no filters beyond status.
  const ids = (list) => list.map((x) => x.id);
  const shown = (e) => e.kind === 'project'
    ? { kind: e.kind, key: e.project.projectKey, name: e.project.displayName, sessions: ids(e.project.sessions) }
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
    const plain = projectGrouping.groupSessions(sidebar, options(false));
    const withPins = projectGrouping.groupSessions(sidebar, options(true));
    const universe = visibility.visibleSidebarProjects(projectGrouping.groupSessions(c.sessions, options(true)).projects, hidden, platform);
    const pinnedKeys = visibility.buildProjectKeyComparisonSet(
      order.map(pinnedOrder.projectKeyFromPinnedEntryId).filter(Boolean), platform);
    const unpinned = (p) => ({ ...p, sessions: p.sessions.filter((s) => s.pinnedAt == null) });
    const notHidden = visibility.visibleSidebarProjects(withPins.projects, hidden, platform);
    const pinnedProjects = notHidden.filter((p) => visibility.projectKeyComparisonSetHas(pinnedKeys, p.projectKey, platform));
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
    const mainProjects = notHidden
      .filter((p) => !visibility.projectKeyComparisonSetHas(pinnedKeys, p.projectKey, platform))
      .map(unpinned);
    const entries = mainListModel.buildMainListEntries({
      projects: mainProjects, dialogues: plain.dialogues, unclassified: plain.unclassified,
      groupBy: c.groupBy ?? 'project', groupDialogue: c.groupDialogue ?? true, sortBy,
      projectOrder: c.projectOrder ?? 'activity', manualProjectOrder: c.manualProjectOrder ?? [],
    });
    return {
      name: c.name,
      pinned: pinned.map((e) => e.kind === 'session'
        ? { kind: 'session', session: e.session.id, label: labels.get(e.session.id) ?? null }
        : { kind: 'project', key: e.project.projectKey, name: e.project.displayName, sessions: ids(e.displaySessions) }),
      entries: entries.map(shown),
    };
  }));

  const full = execFileSync('git', ['-C', cindy, 'rev-parse', `${commit}^{commit}`]).toString().trim();
  fs.writeFileSync(path.join(fixtures, 'COMMIT'), `${full}\n`);
  fs.rmSync(path.join(importDir, 'COMMIT'), { force: true });
  console.log(`golden from ${full}: ${inputs.length} imports, ${cases.length} normalize cases, ${layout.coerce.length + layout.ops.length} layout cases, ${keys.overrides.length} shortcut cases`);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
