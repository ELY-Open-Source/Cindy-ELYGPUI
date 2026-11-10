// Runs a Cindy test file with vitest stubbed and records the arguments it passes to chosen exports.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const VITEST = `
const tests = [];
const each = (register) => (table) => (name, fn) => table.forEach((row) => register(name, () => (Array.isArray(row) ? fn(...row) : fn(row))));
const it = Object.assign((name, fn) => fn && tests.push(fn), { skip() {}, todo() {}, only: (name, fn) => tests.push(fn) });
it.each = each(it);
it.skipIf = (c) => (c ? () => {} : it);
it.runIf = (c) => (c ? it : () => {});
const describe = Object.assign((name, fn) => fn(), { skip() {}, only: (name, fn) => fn() });
describe.each = each(describe);
describe.skipIf = (c) => (c ? () => {} : describe);
describe.runIf = (c) => (c ? describe : () => {});
const any = new Proxy(function () {}, { get: (_, key) => (key === 'then' ? undefined : any), apply: () => any });
export { it, it as test, describe };
export const expect = any, vi = any;
export const beforeEach = () => {}, afterEach = () => {}, beforeAll = () => {}, afterAll = () => {};
export async function run() {
  let failed = 0;
  for (const fn of tests) {
    try { await fn(); } catch { failed++; }
  }
  return { ran: tests.length, failed };
}
`;

// The renderer's modules import styles, fonts, pictures and `?url` files; none of them runs here.
const assetKinds = ['.css', '.woff', '.woff2', '.ttf', '.png', '.svg', '.jpg', '.webp', '.gif', '.mp3', '.wav', '.mp4', '.webm', '.ico'];
const queried = {
  name: 'queried',
  setup(build) {
    build.onResolve({ filter: /\?(url|raw|worker)|\.(png|jpg|svg|webp|gif)$/ }, (args) => ({ path: args.path, namespace: 'queried' }));
    build.onLoad({ filter: /.*/, namespace: 'queried' }, () => ({ contents: 'export default "";' }));
  },
};
export const ASSETS = { loader: Object.fromEntries(assetKinds.map((kind) => [kind, 'empty'])), plugins: [queried] };

const CLOCK = Date.UTC(2026, 9, 20);

/** `targets`: import specifier as the test writes it, to the export names recorded. */
export async function record({ esbuild, tmp, nodePaths, alias, define, test, targets }) {
  const dir = fs.mkdtempSync(path.join(tmp, 'record-'));
  const vitest = path.join(dir, 'vitest.mjs');
  fs.writeFileSync(vitest, VITEST);
  const calls = path.join(dir, 'calls.mjs');
  fs.writeFileSync(calls, 'export const calls = [];\n');
  const plugin = {
    name: 'record',
    setup(build) {
      build.onResolve({ filter: /^vitest$/ }, () => ({ path: vitest }));
      build.onResolve({ filter: /.*/ }, async (args) => {
        const spec = args.path.replace(/\.js$/, '');
        if (args.importer !== test || !(spec in targets) || args.pluginData?.real) return undefined;
        const real = await build.resolve(args.path, { resolveDir: args.resolveDir, kind: args.kind, pluginData: { real: true } });
        if (real.errors.length) throw new Error(`cannot resolve ${args.path} from ${test}`);
        return { path: real.path, namespace: 'record', pluginData: targets[spec] };
      });
      build.onLoad({ filter: /.*/, namespace: 'record' }, (args) => ({
        resolveDir: path.dirname(args.path),
        contents: [
          `import * as real from ${JSON.stringify(args.path)};`,
          `import { calls } from ${JSON.stringify(calls)};`,
          `export * from ${JSON.stringify(args.path)};`,
          ...args.pluginData.map((name) => `export const ${name} = (...a) => { calls.push([${JSON.stringify(name)}, structuredClone(a)]); return real.${name}(...a); };`),
        ].join('\n'),
      }));
    },
  };
  const entry = path.join(dir, 'entry.mjs');
  fs.writeFileSync(entry, `import ${JSON.stringify(test)};\nexport { run } from ${JSON.stringify(vitest)};\nexport { calls } from ${JSON.stringify(calls)};\n`);
  const bundle = path.join(dir, 'bundle.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, define, loader: ASSETS.loader, plugins: [plugin, ...ASSETS.plugins],
  });
  // A test's `new Date()` reads one fixed clock, so a recording repeats.
  const Real = Date;
  globalThis.Date = class extends Real {
    constructor(...args) { super(...(args.length ? args : [CLOCK])); }
    static now() { return CLOCK; }
  };
  // Random ids come from one seed, for the same reason.
  const [random, uuid] = [Math.random, crypto.randomUUID];
  let seed = 94;
  Math.random = () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const hex = (n) => Array.from({ length: n }, () => Math.floor(Math.random() * 16).toString(16)).join('');
  crypto.randomUUID = () => `${hex(8)}-${hex(4)}-4${hex(3)}-8${hex(3)}-${hex(12)}`;
  let mod, ran, failed;
  try {
    mod = await import(pathToFileURL(bundle).href);
    ({ ran, failed } = await mod.run());
  } finally {
    globalThis.Date = Real;
    Math.random = random;
    crypto.randomUUID = uuid;
  }
  console.log(`recorded ${mod.calls.length} calls from ${path.basename(test)} (${ran} tests, ${failed} threw)`);
  return mod.calls;
}
