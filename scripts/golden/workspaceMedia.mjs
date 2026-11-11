// The workspace-media golden: Cindy's `useWorkspaceMediaPath` mounted in jsdom over its real resolver.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const ENTRY = [
  "export { useWorkspaceMediaPath, _clearStreamingMediaKeys } from './renderer/components/chat/useWorkspaceMediaPath';",
  "export { _clearSmartResolveCache, resolveLocalPathSmartCached } from './renderer/lib/localPathResolver';",
  "export { createRoot } from 'react-dom/client';",
  "export { act, createElement, Fragment } from 'react';",
].join('\n');

/** Runs queued microtasks and the promises they chain. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** `cases.json`: steps `render` (`[id, src, enabled, streaming, dir?]` each), `land`, `focus` and the tree's. */
export async function workspaceMedia({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, cindy, src }, main) {
  const dir = path.join(fixtures, 'workspace-media');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const entry = path.join(tmp, src, 'workspace-media-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'workspace-media.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, ...assets, define, jsx: 'automatic',
  });
  const { JSDOM } = createRequire(path.join(path.resolve(cindy), 'package.json'))('jsdom');
  const dom = new JSDOM('<!doctype html><div id="root"></div>', { url: 'http://localhost/', pretendToBeVisual: true });
  const names = ['window', 'document', 'navigator', 'HTMLElement', 'Node', 'requestAnimationFrame', 'cancelAnimationFrame'];
  const before = names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  for (const name of names) Object.defineProperty(globalThis, name, { value: dom.window[name], configurable: true });
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  const realNow = Date.now;
  try {
    const hook = await import(pathToFileURL(bundle).href);
    const { act, createElement, Fragment } = hook;
    const expected = [];
    for (const [n, { name, files = [], steps }] of cases.entries()) {
      const root = path.join(tmp, `media-${n}`);
      const real = (text) => text.replaceAll('<root>', root);
      const shown = (value) => JSON.parse(JSON.stringify(value).replaceAll(root, '<root>'));
      const put = (file) => {
        fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
        fs.writeFileSync(path.join(root, file), '');
      };
      fs.mkdirSync(path.join(root, 'workspace'), { recursive: true });
      files.forEach(put);
      main._clearResolveCache();
      hook._clearSmartResolveCache();
      hook._clearStreamingMediaKeys();
      let [now, ticked] = [1_700_000_000_000, 0];
      Date.now = () => now;
      // A call waits at its gate, as a walk on another thread has not read the disk yet.
      const gates = [];
      const gated = (answer) => (params) => new Promise((resolve) => gates.push(() => answer(params).then(resolve)));
      const isPathAllowed = () => true;
      dom.window.electronAPI = {
        resolvePath: gated((p) => main.resolveWorkspacePathCached(p.href, p.workingDir, { isPathAllowed, refresh: p.refresh === true })),
        resolvePathBatch: gated((p) => main.resolveWorkspacePathBatchCached(p.hrefs, p.workingDir, { isPathAllowed }, p.revalidateNegative === true)),
      };
      const folder = (given) => real(given ?? '<root>/workspace');
      const land = async () => {
        while (gates.length > 0) {
          for (const open of gates.splice(0)) await open();
          await settle();
        }
      };
      // Each mounted hook's latest answer.
      const latest = new Map();
      const Probe = ({ id, given, where, enabled, streaming }) => {
        latest.set(id, hook.useWorkspaceMediaPath(given, where, enabled, streaming));
        return null;
      };
      const mount = hook.createRoot(dom.window.document.createElement('div'));
      let rows = [];
      // The port draws every row each frame, so the parent renders here after each step too.
      const render = () => act(async () => {
        const probes = rows.map(([id, given, enabled, streaming, where]) =>
          createElement(Probe, { key: id, id, given: given === null ? undefined : real(given), where: where === '' ? '' : folder(where), enabled, streaming }));
        mount.render(createElement(Fragment, null, ...probes));
        await settle();
      });
      const out = [];
      for (const step of steps) {
        if (step.render) {
          rows = step.render;
        } else if (step.focus) {
          await act(async () => { dom.window.dispatchEvent(new dom.window.Event('focus')); await settle(); });
        } else if (step.land) {
          await act(land);
        } else {
          if (gates.length > 0) throw new Error(`${name}: a step between a send and its landing`);
          if (step.tick) {
            now += step.tick;
            ticked += step.tick;
          } else if (step.write) put(step.write);
          else if (step.remove) fs.rmSync(path.join(root, step.remove));
          else if (step.flood) {
            // Fresh answers push the oldest out of the cache.
            await act(async () => {
              for (let i = 0; i < step.flood; i += 1) void hook.resolveLocalPathSmartCached(`flood-${i}.ts`, folder());
              await settle();
              await land();
            });
          } else throw new Error(`${name}: unknown step ${JSON.stringify(step)}`);
        }
        await render();
        if (step.land) {
          // A boundary's effect may send a lookup; the same landing takes it in.
          await act(land);
          await render();
        }
        out.push(Object.fromEntries(rows.map(([id]) => {
          const { src: shownSrc, pending, revision } = latest.get(id);
          return [id, { src: shownSrc ?? null, pending, revision }];
        })));
      }
      await act(async () => mount.unmount());
      // The main cache's 30 seconds run on this clock here and on a real one in the port.
      if (ticked >= 30_000) throw new Error(`${name}: ticks reach the main cache's lifetime`);
      if (gates.length > 0) throw new Error(`${name}: a lookup never landed`);
      expected.push({ name, files, steps, out: shown(out) });
    }
    write(path.join(dir, 'expected.json'), expected);
    return expected.reduce((count, { steps }) => count + steps.length, 0);
  } finally {
    Date.now = realNow;
    for (const [name, descriptor] of before) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
    delete globalThis.IS_REACT_ACT_ENVIRONMENT;
    dom.window.close();
  }
}
