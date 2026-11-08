// Usage: node scripts/sync-theme-import-golden.mjs <cindy checkout with node_modules> <commit>
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [cindy, commit] = process.argv.slice(2);
if (!cindy || !commit) throw new Error('usage: sync-theme-import-golden.mjs <cindy checkout> <commit>');
const fixtures = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', 'tests/fixtures/theme-import');
const esbuild = createRequire(path.join(path.resolve(cindy), 'package.json'))('esbuild');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cindy-theme-import-'));
try {
  const src = 'apps/desktop/src/shared/theme-import';
  const archive = execFileSync('git', ['-C', cindy, 'archive', commit, src], { maxBuffer: 1 << 28 });
  execFileSync('tar', ['-x', '-C', tmp], { input: archive });
  const bundle = path.join(tmp, 'bundle.mjs');
  await esbuild.build({
    entryPoints: [path.join(tmp, src, 'index.ts')],
    bundle: true,
    format: 'esm',
    platform: 'node',
    outfile: bundle,
    logLevel: 'error',
  });
  const { convertVsCodeTheme } = await import(pathToFileURL(bundle).href);
  const outDir = path.join(fixtures, 'expected');
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(outDir);
  const inputs = fs.readdirSync(path.join(fixtures, 'input')).sort();
  for (const file of inputs) {
    const raw = fs.readFileSync(path.join(fixtures, 'input', file), 'utf8');
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
    fs.writeFileSync(path.join(outDir, `${path.parse(file).name}.json`), `${JSON.stringify(expected, null, 1)}\n`);
  }
  const full = execFileSync('git', ['-C', cindy, 'rev-parse', `${commit}^{commit}`]).toString().trim();
  fs.writeFileSync(path.join(fixtures, 'COMMIT'), `${full}\n`);
  console.log(`golden from ${full}: ${inputs.length} inputs`);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
