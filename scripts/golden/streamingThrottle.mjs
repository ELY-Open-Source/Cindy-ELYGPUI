// The streaming-throttle golden: Cindy's `useStreamingThrottle` mounted in jsdom on a set clock.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { exporter } from './markdownHast.mjs';

const ENTRY = [
  "export { golden } from './renderer/components/chat/MarkdownRenderer';",
  "export { createRoot } from 'react-dom/client';",
  "export { act, createElement } from 'react';",
].join('\n');
// Lengths about each step of the interval, in UTF-16 units.
const STARTS = [0, 0, 40, 11_980, 31_990, 79_985, 159_990, 319_990];

/** Seeded scenarios: a start clock, then rows `[dt, length, enabled]` of a text that grows. */
function scenarios() {
  let seed = 20261111;
  const next = () => {
    seed = (seed + 0x6d2b79f5) >>> 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const pick = (count) => Math.floor(next() * count);
  return Array.from({ length: 80 }, () => {
    let length = STARTS[pick(STARTS.length)];
    let enabled = next() < 0.8;
    const rows = Array.from({ length: 20 + pick(20) }, () => {
      const roll = pick(20);
      if (roll === 0) enabled = !enabled;
      else if (roll === 1) length = Math.max(0, length - pick(30));
      else if (roll > 3) length += pick(12);
      const wait = [0, pick(40), pick(40), pick(130), 400 + pick(100)][pick(5)];
      return [wait + pick(4) / 4, length, enabled];
    });
    return [[0, 37.5, 1000, 5000.25][pick(4)], rows];
  });
}

/** `expected.json`: each scenario's start and rows `[dt, length, enabled, shown before, shown, timer]`. */
export async function streamingThrottle({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, cindy, src }) {
  const entry = path.join(tmp, src, 'streaming-throttle-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'streaming-throttle.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, loader: assets.loader, plugins: [exporter, ...assets.plugins], define, jsx: 'automatic', keepNames: true,
  });
  const { JSDOM } = createRequire(path.join(path.resolve(cindy), 'package.json'))('jsdom');
  const dom = new JSDOM('<!doctype html>', { url: 'http://localhost/', pretendToBeVisual: true });
  const names = ['window', 'document', 'navigator', 'HTMLElement', 'Node', 'requestAnimationFrame', 'cancelAnimationFrame'];
  const before = names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  for (const name of names) Object.defineProperty(globalThis, name, { value: dom.window[name], configurable: true });
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  const realNow = performance.now;
  let now = 0;
  let timers = [];
  let ids = 0;
  performance.now = () => now;
  dom.window.setTimeout = (fn, delay) => {
    ids += 1;
    timers.push({ id: ids, at: now + delay, fn });
    return ids;
  };
  dom.window.clearTimeout = (id) => {
    timers = timers.filter((timer) => timer.id !== id);
  };
  try {
    const { golden, createRoot, act, createElement } = await import(pathToFileURL(bundle).href);
    let latest;
    const Probe = ({ value, enabled }) => {
      latest = golden.useStreamingThrottle(value, enabled);
      return null;
    };
    const expected = [];
    for (const [start, rows] of scenarios()) {
      now = start;
      timers = [];
      const mount = createRoot(dom.window.document.createElement('div'));
      const out = [];
      for (const [dt, length, enabled] of rows) {
        const target = now + dt;
        // Each timer fires at its own time, in order.
        for (;;) {
          const due = timers.filter((timer) => timer.at <= target).sort((a, b) => a.at - b.at)[0];
          if (!due) break;
          now = due.at;
          timers = timers.filter((timer) => timer !== due);
          await act(async () => due.fn());
        }
        now = target;
        const fired = latest === undefined ? null : latest.length;
        await act(async () => mount.render(createElement(Probe, { value: 'x'.repeat(length), enabled })));
        if (timers.length > 1) throw new Error('two timers wait');
        out.push([dt, length, enabled, fired, latest.length, timers[0]?.at ?? null]);
      }
      await act(async () => mount.unmount());
      if (timers.length > 0) throw new Error('a timer outlives its hook');
      latest = undefined;
      expected.push([start, out]);
    }
    write(path.join(fixtures, 'streaming-throttle', 'expected.json'), expected);
    return expected.reduce((count, [, rows]) => count + rows.length, 0);
  } finally {
    performance.now = realNow;
    for (const [name, descriptor] of before) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
    delete globalThis.IS_REACT_ACT_ENVIRONMENT;
    dom.window.close();
  }
}
