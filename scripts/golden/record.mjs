// Runs a Cindy test file with vitest stubbed and records the arguments it passes to chosen exports.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const VITEST = `
const tests = [];
const each = (register) => (table) => (name, fn) => table.forEach((row) => register(name, () => (Array.isArray(row) ? fn(...row) : fn(row))));
const it = Object.assign((name, fn) => fn && tests.push(fn), { skip() {}, todo() {}, only: (name, fn) => tests.push(fn) });
it.each = each(it);
const describe = Object.assign((name, fn) => fn(), { skip() {}, only: (name, fn) => fn() });
describe.each = each(describe);
const any = new Proxy(function () {}, { get: (_, key) => (key === 'then' ? undefined : any), apply: () => any });
export { it, it as test, describe };
export const expect = any;
export const beforeEach = () => {}, afterEach = () => {}, beforeAll = () => {}, afterAll = () => {};
export async function run() {
  let failed = 0;
  for (const fn of tests) {
    try { await fn(); } catch { failed++; }
  }
  return { ran: tests.length, failed };
}
`;

/** `targets`: import specifier as the test writes it, to the export names recorded. */
export async function record({ esbuild, tmp, nodePaths, alias, test, targets }) {
  const dir = fs.mkdtempSync(path.join(tmp, 'record-'));
  const vitest = path.join(dir, 'vitest.mjs');
  fs.writeFileSync(vitest, VITEST);
  const calls = path.join(dir, 'calls.mjs');
  fs.writeFileSync(calls, 'export const calls = [];\n');
  const plugin = {
    name: 'record',
    setup(build) {
      build.onResolve({ filter: /^vitest$/ }, () => ({ path: vitest }));
      build.onResolve({ filter: /.*/ }, (args) => {
        const spec = args.path.replace(/\.js$/, '');
        if (args.importer !== test || !(spec in targets)) return undefined;
        const real = `${path.resolve(path.dirname(test), spec)}.ts`;
        return { path: real, namespace: 'record', pluginData: targets[spec] };
      });
      build.onLoad({ filter: /.*/, namespace: 'record' }, (args) => ({
        resolveDir: path.dirname(args.path),
        contents: [
          `import * as real from ${JSON.stringify(args.path)};`,
          `import { calls } from ${JSON.stringify(calls)};`,
          `export * from ${JSON.stringify(args.path)};`,
          ...args.pluginData.map((name) => `export const ${name} = (...a) => { calls.push([${JSON.stringify(name)}, a]); return real.${name}(...a); };`),
        ].join('\n'),
      }));
    },
  };
  const entry = path.join(dir, 'entry.mjs');
  fs.writeFileSync(entry, `import ${JSON.stringify(test)};\nexport { run } from ${JSON.stringify(vitest)};\nexport { calls } from ${JSON.stringify(calls)};\n`);
  const bundle = path.join(dir, 'bundle.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, plugins: [plugin],
  });
  const mod = await import(pathToFileURL(bundle).href);
  const { ran, failed } = await mod.run();
  console.log(`recorded ${mod.calls.length} calls from ${path.basename(test)} (${ran} tests, ${failed} threw)`);
  return mod.calls;
}
