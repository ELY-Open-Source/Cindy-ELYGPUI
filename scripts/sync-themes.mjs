// Usage: node scripts/sync-themes.mjs <cindy checkout with node_modules> <commit>
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [cindy, commit] = process.argv.slice(2);
if (!cindy || !commit) throw new Error('usage: sync-themes.mjs <cindy checkout> <commit>');
const out = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', 'themes');
const esbuild = createRequire(path.join(path.resolve(cindy), 'package.json'))('esbuild');

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cindy-themes-'));
try {
  const src = 'apps/desktop/src';
  const archive = execFileSync('git', ['-C', cindy, 'archive', commit, src], { maxBuffer: 1 << 30 });
  execFileSync('tar', ['-x', '-C', tmp], { input: archive });
  const themesDir = path.join(tmp, src, 'renderer/themes');
  const builtins = fs.readdirSync(path.join(themesDir, 'builtin')).filter((f) => f.endsWith('.ts')).sort();
  const entry = path.join(themesDir, 'sync-entry.ts');
  fs.writeFileSync(entry, [
    "import './colors';",
    "import { colorRegistry } from './color-registry';",
    ...builtins.map((f, i) => `import * as b${i} from './builtin/${f.slice(0, -3)}';`),
    `export const builtins = [${builtins.map((_, i) => `...Object.values(b${i})`).join(', ')}];`,
    'export const colors = colorRegistry.getColors();',
  ].join('\n'));
  const pngs = new Map();
  const bundle = path.join(tmp, 'bundle.mjs');
  await esbuild.build({
    entryPoints: [entry],
    bundle: true,
    format: 'esm',
    platform: 'node',
    outfile: bundle,
    logLevel: 'error',
    plugins: [{
      name: 'png-path',
      setup(build) {
        build.onLoad({ filter: /\.png$/ }, (args) => {
          const name = path.basename(args.path);
          pngs.set(name, args.path);
          return { contents: `export default ${JSON.stringify(`assets/${name}`)};`, loader: 'js' };
        });
      },
    }],
  });
  const { builtins: exported, colors } = await import(pathToFileURL(bundle).href);
  const themes = exported.filter((t) => t && typeof t === 'object' && typeof t.id === 'string' && t.colors);
  if (themes.length !== builtins.length) {
    throw new Error(`expected ${builtins.length} built-in themes, found ${themes.length}`);
  }
  fs.rmSync(out, { recursive: true, force: true });
  fs.mkdirSync(path.join(out, 'builtin'), { recursive: true });
  fs.mkdirSync(path.join(out, 'assets'), { recursive: true });
  const registry = colors.map(({ id, defaults }) => ({ id, light: defaults.light, dark: defaults.dark }));
  fs.writeFileSync(path.join(out, 'registry.json'), `${JSON.stringify(registry, null, 1)}\n`);
  for (const theme of themes) {
    fs.writeFileSync(path.join(out, 'builtin', `${theme.id}.json`), `${JSON.stringify(theme, null, 1)}\n`);
  }
  for (const [name, from] of pngs) fs.copyFileSync(from, path.join(out, 'assets', name));
  const full = execFileSync('git', ['-C', cindy, 'rev-parse', `${commit}^{commit}`]).toString().trim();
  fs.writeFileSync(path.join(out, 'COMMIT'), `${full}\n`);
  console.log(`themes from ${full}: ${registry.length} tokens, ${themes.length} built-ins, ${pngs.size} assets`);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
