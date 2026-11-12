// Run by `streamedChunks.mjs` in Cindy's Electron as Node: argv is the bundle, Cindy's checkout, the output.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [bundle, cindy, out] = process.argv.slice(2);
let seed = 0;
const next = () => {
  seed = (seed + 0x6d2b79f5) >>> 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = (count) => Math.floor(next() * count);
const one = (list) => list[pick(list.length)];

const WORDS = ['hello', 'world', 'The', 'quick', 'fox', 'a', '3.14', "don't", 'e.g.', 'a-b', 'x:y', '1,000', 'naïve', '(ok)', 'end.', '你好', '世界',
  '这是一个测试', 'こんにちは', 'カタカナ', '한국어', '😀', '**bold**', '*slanted*', '`code`', '`a b`', '[link](http://a.b/c)', '~~gone~~', 'hello',
  '[go task](cindy://session/task-id)', '[a folder](cindy://project/abc)', '[card one](cindy://session-card/task-id?title=Task&wake=created&last_active=null)',
  '[no card](cindy://session-card/)', '[a sound](xdt-audio://local/a.mp3)'];
const sentence = () => Array.from({ length: 1 + pick(9) }, () => one(WORDS)).join(' ');
const BLOCKS = [
  () => sentence(), () => sentence(), () => sentence(), () => `${sentence()}\n${sentence()}`,
  () => Array.from({ length: 1 + pick(4) }, () => `- ${sentence()}`).join('\n'),
  () => Array.from({ length: 1 + pick(3) }, (_, nth) => `${nth + 1}. ${sentence()}`).join('\n'),
  () => `> ${sentence()}`, () => `## ${sentence()}`, () => `- [ ] ${sentence()}\n- [x] ${sentence()}`,
  () => `\`\`\`ts\nconst a = 1;\n\nlet b;\n\`\`\``, () => `| a | b |\n| - | - |\n| ${one(WORDS)} | ${sentence()} |`,
  () => `[ref]: http://a.b/c`, () => `<div>${sentence()}</div>`, () => `- ${sentence()}\n\n  ${sentence()}`,
];
const document = () => Array.from({ length: 2 + pick(5) }, () => one(BLOCKS)()).join('\n\n');

const { JSDOM } = createRequire(path.join(cindy, 'package.json'))('jsdom');
const dom = new JSDOM('<!doctype html>', { url: 'http://localhost/', pretendToBeVisual: true });
for (const name of ['window', 'document', 'navigator', 'HTMLElement', 'Node', 'requestAnimationFrame', 'cancelAnimationFrame']) {
  Object.defineProperty(globalThis, name, { value: dom.window[name], configurable: true });
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
globalThis.fetch = () => { throw new Error('the golden asks nothing of the network'); };
// A renderer has met Han text before long: ICU keeps the CJK engine from then on.
[...new Intl.Segmenter(undefined, { granularity: 'word' }).segment(String.fromCodePoint(0x6f22, 0x5b57))];
const cindyCode = await import(pathToFileURL(bundle).href);
const { golden, act, createElement } = cindyCode;
// Strict inline math, KaTeX and the highlighter come with their chapters, as in the hast golden.
const named = (list, index, name) => {
  const plugin = [list[index]].flat()[0];
  if (plugin.name !== name) throw new Error(`plugin ${index} is ${plugin.name}, not ${name}`);
  return list[index];
};
const remark = cindyCode.REMARK_PLUGINS_PRIVILEGED;
const remarkPlugins = remark.filter((plugin) => plugin !== named(remark, 3, 'remarkStrictInlineMath'));
const rehypePlugins = [named(golden.REHYPE_PLUGINS, 0, 'rehypeSlug'), named(golden.REHYPE_PLUGINS, 4, 'rehypeFencedCodeMarker')];
// Cindy's own `a` renderer: a chip, a card or the player takes the place of a link's words.
const components = golden.linkRenderer();

/** The timeline every chunk of a message shares. */
const snapshot = ({ timeline }) => ({
  start: [...timeline.startAtByKey].sort(([a], [b]) => (a < b ? -1 : 1)),
  settled: [...timeline.settled].sort(),
  next: timeline.nextSegmentStartAtMs,
});

const streams = [];
for (let nth = 0; nth < 36; nth++) {
  seed = 0x51ed270b ^ (nth * 7919);
  const points = [...document()];
  const start = pick(2000) + pick(4) / 4;
  let clock = start;
  const owner = cindyCode.createWordFadeState();
  owner.timeline.nowFn = () => clock;
  const host = dom.window.document.createElement('div');
  const mount = cindyCode.createRoot(host);
  const steps = [];
  for (let shown = 0; shown < points.length;) {
    shown = Math.min(points.length, shown + 1 + pick(nth % 4 === 0 ? 40 : 12));
    const dt = [pick(30), pick(30), 100 + pick(40), 100 + pick(40), 300][pick(5)] + pick(4) / 4;
    clock += dt;
    const text = points.slice(0, shown).join('');
    // `MarkdownRendererBase`: the repair, then a chunk per piece, keyed by its start.
    const chunks = cindyCode.splitStreamingMarkdownChunks(cindyCode.repairStreamingMarkdown(text));
    const rows = chunks.map((chunk) => createElement(cindyCode.StreamingMarkdownChunk, {
      key: chunk.start, sourceKey: String(chunk.start), content: chunk.content, remarkPlugins, rehypePlugins, components,
      urlTransform: golden.trustedUrlTransform, wordFadeState: owner, emitSourceLines: false, wholeDocument: chunks.length === 1,
    }));
    await act(async () => mount.render(createElement(cindyCode.MemoryRouter, null, ...rows)));
    const spans = [...host.querySelectorAll('span[data-wf-key]')].map((span) => [span.getAttribute('data-wf-key'), span.textContent]);
    steps.push([dt, shown, spans, snapshot(owner)]);
  }
  await act(async () => mount.unmount());
  streams.push([start, points.join(''), steps]);
}
const versions = { icu: process.versions.icu, unicode: process.versions.unicode, v8: process.versions.v8 };
fs.writeFileSync(out, JSON.stringify({ versions, streams }));
process.exit(0);
