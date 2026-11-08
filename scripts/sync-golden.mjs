// Usage: node scripts/sync-golden.mjs <cindy checkout with node_modules> <commit>
import { execFileSync } from 'node:child_process';
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
  const archive = execFileSync('git', ['-C', cindy, 'archive', commit, src], { maxBuffer: 1 << 30 });
  execFileSync('tar', ['-x', '-C', tmp], { input: archive });
  const entry = path.join(tmp, src, 'golden-entry.ts');
  fs.writeFileSync(entry, [
    "import './renderer/themes/colors';",
    "export { convertVsCodeTheme } from './shared/theme-import';",
    "export { normalizeLocalThemeColors } from './renderer/themes/local-themes-normalize';",
  ].join('\n'));
  const bundle = path.join(tmp, 'bundle.mjs');
  await esbuild.build({ entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error' });
  const { convertVsCodeTheme, normalizeLocalThemeColors } = await import(pathToFileURL(bundle).href);

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

  const full = execFileSync('git', ['-C', cindy, 'rev-parse', `${commit}^{commit}`]).toString().trim();
  fs.writeFileSync(path.join(fixtures, 'COMMIT'), `${full}\n`);
  fs.rmSync(path.join(importDir, 'COMMIT'), { force: true });
  console.log(`golden from ${full}: ${inputs.length} imports, ${cases.length} normalize cases`);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
