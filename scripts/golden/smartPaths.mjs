// Cindy's renderer path cache over its real main resolver: each case's steps run on a real tree.
import fs from 'node:fs';
import path from 'node:path';

/** Runs queued microtasks and the promises they chain. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

export async function smartPaths({ tmp, fixtures, write }, renderer, main) {
  const dir = path.join(fixtures, 'smart-paths');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const realNow = Date.now;
  const expected = [];
  try {
    for (const [n, { name, files = [], steps }] of cases.entries()) {
      const root = path.join(tmp, `smart-${n}`);
      const real = (text) => text.replaceAll('<root>', root);
      const shown = (value) => JSON.parse(JSON.stringify(value ?? null).replaceAll(root, '<root>'));
      const put = (file) => {
        fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
        fs.writeFileSync(path.join(root, file), '');
      };
      fs.mkdirSync(path.join(root, 'workspace'), { recursive: true });
      files.forEach(put);
      main._clearResolveCache();
      renderer._clearSmartResolveCache();
      let [now, ticked, mark] = [1_700_000_000_000, 0, 0];
      Date.now = () => now;
      // A call waits at its gate, as a walk on another thread has not read the disk yet.
      const gates = [];
      const gated = (answer) => (params) => new Promise((resolve) => gates.push(() => answer(params).then(resolve)));
      const isPathAllowed = () => true;
      globalThis.window = { electronAPI: {
        resolvePath: gated((p) => main.resolveWorkspacePathCached(p.href, p.workingDir, { isPathAllowed, refresh: p.refresh === true })),
        resolvePathBatch: gated((p) => main.resolveWorkspacePathBatchCached(p.hrefs, p.workingDir, { isPathAllowed }, p.revalidateNegative === true)),
      } };
      const direct = [];
      const folder = (given) => real(given ?? '<root>/workspace');
      const run = ([op, href, extra, where]) => {
        if (op === 'mark') mark = now;
        else if (op === 'ask') void renderer.resolveLocalPathSmartCached(real(href), folder(where));
        else if (op === 'peek') return renderer.peekResolveLocalPathSmart(real(href), folder(where)) ?? null;
        else if (op === 'recheck') {
          const boundary = extra === 'now' ? now : extra === 'mark' ? mark : undefined;
          return renderer.revalidateNegativeLocalPathSmart(real(href), folder(where), boundary) !== null;
        } else if (op === 'direct') {
          const slot = direct.push(undefined) - 1;
          void renderer.resolveLocalPathSmart(real(href), folder(where), { refresh: extra === true }).then((result) => { direct[slot] = result; });
        } else throw new Error(`unknown op ${op}`);
        return null;
      };
      const out = [];
      for (const step of steps) {
        if (step.do) {
          out.push(step.do.map(run));
          await settle();
        } else if (step.land) {
          while (gates.length > 0) {
            for (const open of gates.splice(0)) await open();
            await settle();
          }
          out.push(direct.splice(0));
        } else {
          if (gates.length > 0) throw new Error(`${name}: a step between a send and its landing`);
          if (step.tick) {
            now += step.tick;
            ticked += step.tick;
          } else if (step.write) put(step.write);
          else if (step.remove) fs.rmSync(path.join(root, step.remove));
          else if (step.flood) {
            // Fresh answers push the oldest out of the cache.
            for (let i = 0; i < step.flood; i += 1) void renderer.resolveLocalPathSmartCached(`flood-${i}.ts`, folder());
            await settle();
            for (const open of gates.splice(0)) await open();
            await settle();
          }
          out.push(null);
        }
      }
      // The main cache's 30 seconds run on this clock here and on a real one in the port.
      if (ticked >= 30_000) throw new Error(`${name}: ticks reach the main cache's lifetime`);
      if (gates.length > 0) throw new Error(`${name}: a lookup never landed`);
      expected.push({ name, files, steps, out: shown(out) });
    }
  } finally {
    Date.now = realNow;
    delete globalThis.window;
  }
  write(path.join(dir, 'expected.json'), expected);
  return expected.reduce((n, { steps }) => n + steps.length, 0);
}
