// Runs Cindy's `rehypeStreamWordFade.ts` in Cindy's Electron on seeded streams: tests/fixtures/word-fade/expected.json.
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const root = path.join(__dirname, '..');
const [cindy, commit] = process.argv.slice(2);
if (!cindy || !commit) throw new Error('usage: electron scripts/word-fade.js <cindy checkout> <commit>');
const source = 'apps/desktop/src/renderer/components/chat/rehypeStreamWordFade.ts';
const ts = execFileSync('git', ['-C', cindy, 'show', `${commit}:${source}`], { encoding: 'utf8' });
const esbuild = require(path.join(path.resolve(cindy), 'node_modules', 'esbuild'));
const scratch = path.join(root, '.scratch', 'word-fade');
fs.mkdirSync(scratch, { recursive: true });
fs.writeFileSync(path.join(scratch, 'fade.cjs'), esbuild.transformSync(ts, { loader: 'ts', format: 'cjs' }).code);
// A renderer has met Han text before long: ICU keeps the CJK engine from then on.
[...new Intl.Segmenter(undefined, { granularity: 'word' }).segment(String.fromCodePoint(0x6f22, 0x5b57))];
const fade = require(path.join(scratch, 'fade.cjs'));

let seed = 0;
const next = () => {
  seed = (seed + 0x6d2b79f5) >>> 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = (count) => Math.floor(next() * count);
const one = (list) => list[pick(list.length)];
const chance = (odds) => next() < odds;

const WORDS = ['hello', 'world', 'The', 'quick', 'fox', 'a', '3.14', "don't", 'e.g.', 'a-b', 'foo_bar', 'x:y', '1,000', 'naïve', '(ok)', '—', 'end.', 'hello',
  '你好', '世界', '这是一个测试', '流式输出', 'こんにちは', 'カタカナ', '漢字かな交じり', '한국어', '😀', '👍🏽', 'https://x.y/z?q=1', 'Привет', 'hello'];
const GAPS = [' ', ' ', ' ', ' ', '  ', '\n', ' \n', '\t', String.fromCodePoint(0xa0), String.fromCodePoint(0x3000), ''];
const WRAPS = [null, null, null, null, 'strong', 'em', 'a', 'del'];
const TAGS = ['p', 'p', 'p', 'li', 'li', 'h2', 'quote', 'cell', 'pre'];

const unit = () => {
  const roll = pick(20);
  if (roll === 0) return { kind: 'code', text: one(WORDS), chip: chance(0.3) };
  if (roll === 1) return { kind: 'katex', text: 'x^2' };
  if (roll === 2) return { kind: 'br' };
  if (roll === 3) return { kind: 'word', text: '', gap: one(GAPS), wrap: null };
  return { kind: 'word', text: one(WORDS), gap: one(GAPS), wrap: one(WRAPS) };
};
const block = () => ({ tag: one(TAGS), units: Array.from({ length: pick(9) }, unit) });
const text = (value) => ({ type: 'text', value });
const el = (tagName, children, properties = {}) => ({ type: 'element', tagName, properties, children });

/** A block's inline nodes: word units in one wrap share a text node. */
const inlines = (units, cut) => {
  const out = [];
  let run = null;
  units.forEach((u, nth) => {
    if (u.kind !== 'word') {
      run = null;
      if (u.kind === 'br') out.push(el('br', []));
      else if (u.kind === 'katex') out.push(el('span', [el('span', [text(u.text)], { className: ['katex-html'] })], { className: ['katex'] }));
      else out.push(el('code', u.chip ? [el('span', [text(u.text)], { className: ['chip'] }), text('!')] : [text(u.text)]));
      return;
    }
    const last = nth === units.length - 1;
    // A cut falls between code points: the port's text holds no lone surrogate.
    const points = [...u.text];
    const value = last && cut ? points.slice(0, Math.max(1, points.length - cut)).join('') : u.text + u.gap;
    if (run && run.wrap === u.wrap) {
      run.node.value += value;
      return;
    }
    const node = text(value);
    run = { wrap: u.wrap, node };
    out.push(u.wrap ? el(u.wrap, [node], u.wrap === 'a' ? { href: 'https://example.com/' } : {}) : node);
  });
  return out;
};

/** The tree of `blocks` up to `limit` places: a block's opening is a place, as each of its units is. */
const tree = (blocks, limit, cut) => {
  const children = [];
  let left = limit;
  let list = null;
  for (const b of blocks) {
    if (left <= 0) break;
    left -= 1;
    const shown = b.units.slice(0, left);
    const ends = left <= b.units.length;
    left -= b.units.length;
    const inner = b.tag === 'pre' ? [el('code', [text(shown.map((u) => u.text ?? '').join(' '))], { dataFencedCode: '' })] : inlines(shown, ends ? cut : 0);
    if (b.tag !== 'li') list = null;
    if (children.length > 0 && !list) children.push(text('\n'));
    if (b.tag === 'li') {
      if (!list) {
        list = el(chance(0.5) ? 'ul' : 'ol', [text('\n')]);
        children.push(list);
      }
      list.children.push(el('li', inner, chance(0.2) ? { className: ['task-list-item'] } : {}), text('\n'));
    } else if (b.tag === 'quote') children.push(el('blockquote', [text('\n'), el('p', inner), text('\n')]));
    else if (b.tag === 'cell') children.push(el('table', [el('tbody', [el('tr', [el('td', inner), el('td', [])])])]));
    else children.push(el(b.tag, inner));
  }
  return { type: 'root', children };
};

const bare = (node) => (node.type === 'text' ? node.value : { tag: node.tagName, props: node.properties ?? {}, children: node.children.map(bare) });
const seen = (state) => state.previous.map((s) => [s.kind, s.content, s.key]);
const snapshot = (owner) => ({
  nextId: owner.nextId,
  previous: seen(owner),
  sources: [...owner.sourceStateByKey].sort(([a], [b]) => Number(a) - Number(b)).map(([key, s]) => [Number(key), s.nextId, seen(s)]),
  start: [...owner.timeline.startAtByKey].sort(([a], [b]) => (a < b ? -1 : 1)),
  settled: [...owner.timeline.settled].sort(),
  next: owner.timeline.nextSegmentStartAtMs,
});

/** The keys a commit mounts, in document order: active spans and marked items. */
const mounted = (node, out) => {
  for (const child of node.children) {
    if (child.type !== 'element') continue;
    const props = child.properties ?? {};
    const active = Array.isArray(props.className) && props.className.includes('stream-word');
    if (typeof props.dataWfKey === 'string' && (active || props.dataStreamMarker)) out.push(props.dataWfKey);
    mounted(child, out);
  }
  return out;
};

const scenario = (nth) => {
  seed = 0x9e3779b9 ^ (nth * 7919);
  const owner = fade.createWordFadeState();
  let clock = pick(1000) + pick(4) / 4;
  owner.timeline.nowFn = () => clock;
  const steps = [];
  const blocks = Array.from({ length: 2 + pick(5) }, block);
  const places = () => blocks.reduce((sum, b) => sum + 1 + b.units.length, 0);
  const bounds = nth % 3 === 0 ? [0] : [0, 1 + pick(blocks.length - 1)];
  let limit = 1 + pick(3);
  const render = (key, whole, input) => {
    const committed = fade.getOrCreateWordFadeSourceState(owner, String(key));
    const tries = chance(0.15) ? 2 : 1;
    for (let attempt = 1; attempt <= tries; attempt++) {
      const candidate = whole ? fade.createWholeDocumentWordFadeCandidate(owner, committed) : fade.createWordFadeCandidate(committed);
      const out = structuredClone(input);
      fade.rehypeStreamWordFade(candidate)(out);
      const commit = attempt === tries;
      if (commit) {
        fade.commitWordFadeCandidate(committed, candidate);
        if (whole) fade.retainOnlyWordFadeSourceState(owner, String(key));
      }
      steps.push({ op: 'run', source: key, whole, commit, now: clock, tree: bare(input).children, out: bare(out).children, state: snapshot(owner) });
      if (!commit) continue;
      for (const wfKey of mounted(out, [])) {
        if (fade.isWordFadeSettled(committed, wfKey) || chance(0.1)) continue;
        const delay = fade.scheduleWordFadeSegment(committed, wfKey, { style: { setProperty() {} } });
        steps.push({ op: 'schedule', source: key, key: wfKey, now: clock, delay });
      }
    }
  };
  for (let round = 0; round < 9 + pick(6); round++) {
    const whole = bounds.length === 1 || round % 7 === 5;
    if (whole) render(0, true, tree(blocks, limit, chance(0.4) ? 1 + pick(3) : 0));
    else {
      let left = limit;
      bounds.forEach((from, at) => {
        const part = blocks.slice(from, bounds[at + 1] ?? blocks.length);
        const size = part.reduce((sum, b) => sum + 1 + b.units.length, 0);
        if (left > 0) render(from * 100, false, tree(part, left, left <= size && chance(0.4) ? 1 : 0));
        left -= size;
      });
    }
    clock += pick(90) + pick(4) / 4 + (chance(0.15) ? 200 : 0);
    for (const [key, start] of owner.timeline.startAtByKey) {
      if (clock - start >= 150 && !owner.timeline.settled.has(key) && chance(0.6)) {
        fade.markWordFadeSettled(owner, key);
        steps.push({ op: 'settle', key });
      }
    }
    limit = Math.min(places(), limit + (chance(0.7) ? 1 + pick(4) : 12));
    if (chance(0.25)) {
      const b = one(blocks);
      const at = pick(b.units.length + 1);
      const edit = pick(4);
      if (edit === 0) b.units.splice(at, 0, unit());
      else if (edit === 1 && b.units.length > 0) b.units.splice(Math.min(at, b.units.length - 1), 1);
      else if (edit === 2 && b.units[at]?.kind === 'word') b.units[at].wrap = one(WRAPS);
      else b.tag = one(TAGS);
    }
  }
  steps.push({ op: 'end', state: snapshot(owner) });
  return steps;
};

/** A tree the transformer marked comes back with its items emptied: an item keeps the key it has. */
const marked = () => {
  const owner = fade.createWordFadeState();
  owner.timeline.nowFn = () => 0;
  const committed = fade.getOrCreateWordFadeSourceState(owner, '0');
  const steps = [];
  const run = (input) => {
    const candidate = fade.createWholeDocumentWordFadeCandidate(owner, committed);
    const out = structuredClone(input);
    fade.rehypeStreamWordFade(candidate)(out);
    fade.commitWordFadeCandidate(committed, candidate);
    fade.retainOnlyWordFadeSourceState(owner, '0');
    steps.push({ op: 'run', source: 0, whole: true, commit: true, now: 0, tree: bare(input).children, out: bare(out).children, state: snapshot(owner) });
    return out;
  };
  const first = run({ type: 'root', children: [el('ul', [el('li', [text('one two')], { style: 'color:red' }), el('li', [text('three')])])] });
  const [one, two] = first.children[0].children;
  one.children = [];
  two.children = [el('pre', [el('code', [text('four')])])];
  run(first);
  steps.push({ op: 'end', state: snapshot(owner) });
  return steps;
};

const scenarios = [...Array.from({ length: 36 }, (_, nth) => scenario(nth)), marked()];
const dir = path.join(root, 'tests', 'fixtures', 'word-fade');
fs.mkdirSync(dir, { recursive: true });
const versions = { icu: process.versions.icu, unicode: process.versions.unicode, v8: process.versions.v8 };
fs.writeFileSync(path.join(dir, 'expected.json'), `${JSON.stringify({ versions, commit, scenarios })}\n`);
const count = (op) => scenarios.flat().filter((step) => step.op === op).length;
console.log(`word-fade: ${scenarios.length} scenarios, ${count('run')} runs, ${count('schedule')} schedules, ${count('settle')} settles`);
