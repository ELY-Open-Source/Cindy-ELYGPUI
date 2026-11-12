// Run by `streamFade.mjs` in Cindy's Electron: argv is the bundle, the CSS, the output, a profile folder.
const { app, BrowserWindow, session } = require('electron');
const fs = require('node:fs');

const [bundle, styles, out, profile] = process.argv.slice(-4);
app.setPath('userData', profile);
app.setPath('sessionData', profile);
app.disableHardwareAcceleration();

const page = async () => {
  const { StreamingMarkdownChunk, createWordFadeState, repairStreamingMarkdown, splitStreamingMarkdownChunks, createRoot, flushSync, createElement, remarkGfm } = cindyFade;
  let seed = 0;
  const next = () => {
    seed = (seed + 0x6d2b79f5) >>> 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const pick = (count) => Math.floor(next() * count);
  const one = (list) => list[pick(list.length)];
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const tick = String.fromCharCode(96);
  const WORDS = ['alpha', 'beta', 'gamma', 'delta', 'one', 'two', tick + 'code' + tick, '**bold**'];
  const LEADS = ['- ', '- ', '- ', '1. ', '- [ ] ', '  - ', '', '> - '];
  // The same classes, in the same order, as Cindy's `ul` and `ol` renderers.
  const components = {
    ul: ({ children, node, ...props }) => createElement('ul', { className: 'my-2 ml-6 list-disc space-y-1', ...props }, children),
    ol: ({ children, node, ...props }) => createElement('ol', { className: 'my-2 ml-6 list-decimal space-y-1', ...props }, children),
  };
  const remarkPlugins = [remarkGfm];
  const rehypePlugins = [];
  const FIXED = [
    [{ text: 'one two' }, { wait: 60 }, { wait: 250 }],
    [{ text: '-' }, { wait: 80 }, { text: '- alpha' }, { wait: 100 }, { text: '- alpha beta' }, { wait: 250 }],
    [{ text: '-' }, { wait: 300 }, { text: '- alpha' }, { text: '- alpha beta' }, { wait: 250 }],
    [{ text: '- one' }, { wait: 300 }, { text: '- alpha' }, { text: '- alpha beta' }, { wait: 250 }],
    [{ text: '1. one' }, { wait: 300 }, { text: '1. alpha' }, { text: '1. alpha beta' }, { wait: 250 }],
    [{ text: '- one' }, { wait: 60 }, { text: '1. alpha' }, { wait: 250 }],
    [{ text: '- [ ] ' }, { wait: 250 }, { text: '- [ ] alpha' }, { text: '- [ ] alpha beta' }, { wait: 250 }],
    [{ text: '- [ ] ' }, { wait: 80 }, { text: '- [ ] alpha' }, { wait: 100 }, { text: '- [ ] alpha beta' }, { wait: 250 }],
    [{ text: '- [ ] alpha' }, { wait: 100 }, { text: '- [ ] beta' }, { wait: 60 }, { text: '- [ ] beta gamma' }, { wait: 250 }],
    [{ text: '- ' }, { wait: 250 }, { text: '-\n  - alpha' }, { text: '-\n  - alpha beta' }, { wait: 250 }],
    [{ text: '- a\n-' }, { wait: 300 }, { text: '- a b c d e\n- x' }, { wait: 100 }, { text: '- a b c d e\n- x y' }, { wait: 250 }],
    [{ text: '- a\n-' }, { wait: 60 }, { text: '- a b c d e\n- x' }, { wait: 100 }, { text: '- a b c d e\n- x y' }, { wait: 250 }],
    [{ text: '- one' }, { wait: 60 }, { text: '-' }, { wait: 60 }, { text: '- two' }, { wait: 250 }, { text: '-' }, { text: '- one' }, { wait: 250 }],
    [{ text: 'a\n- one' }, { wait: 300 }, { text: 'a\n- one\n- two' }, { wait: 60 }, { text: 'a\n- one\n- two three' }, { wait: 250 }],
    [{ text: '- one\n1. two' }, { wait: 300 }, { text: '- one\n1. two three' }, { wait: 60 }, { text: '- alpha\n1. two three' }, { wait: 250 }],
    [{ text: '- a\n- b c' }, { wait: 60 }, { text: '- a\n-' }, { wait: 250 }],
    [{ text: 'a\n- one' }, { wait: 300 }, { text: 'a b\n- one' }, { wait: 60 }, { text: 'a b\n- two' }, { wait: 250 }],
    [{ text: '- one\n# h' }, { wait: 300 }, { text: '- one\n# h x' }, { wait: 60 }, { text: '- two\n# h x' }, { wait: 250 }],
    [{ text: '- one\n\n  two' }, { wait: 300 }, { text: '- one\n\n  two three' }, { wait: 60 }, { text: '- alpha\n\n  two three' }, { wait: 250 }],
  ];
  const drawn = () => {
    seed = 0;
    return FIXED.concat(Array.from({ length: 30 }, (_, nth) => {
      seed = 0x2545f491 ^ (nth * 7919);
      const lines = [one(LEADS)];
      const steps = [];
      for (let n = 3 + pick(6); n > 0; n--) {
        const roll = pick(10);
        const last = lines.length - 1;
        if (roll < 5) lines[last] += (/\S$/.test(lines[last]) ? ' ' : '') + one(WORDS);
        else if (roll < 7 && lines.length < 4) lines.push(one(LEADS) + (pick(3) ? one(WORDS) : ''));
        else if (roll < 9) lines[last] = one(LEADS) + one(WORDS);
        else lines[pick(lines.length)] = one(LEADS);
        steps.push({ text: lines.join('\n') });
        if (pick(4)) steps.push({ wait: one([50, 100, 250, 400]) });
      }
      steps.push({ wait: 300 });
      return steps;
    }));
  };

  // Every animation met, with the time it ends by the page's clock.
  const ends = new Map();
  const watch = () => {
    for (const animation of document.getAnimations()) {
      const end = animation.effect.getComputedTiming().endTime;
      if (animation.playState !== 'finished') ends.set(animation, performance.now() + end - (animation.currentTime ?? 0));
    }
  };
  // A step is read 45ms clear of every end: an event has fired or is yet to.
  const settle = async () => {
    for (;;) {
      watch();
      const now = performance.now();
      const near = [...ends.values()].filter((end) => Math.abs(end - now) < 45);
      if (near.length === 0) return;
      await sleep(Math.max(...near) + 45 - now);
    }
  };
  const place = (item, host) => {
    const parts = [];
    for (let node = item; node !== host; node = node.parentElement) {
      const same = [...node.parentElement.children].filter((other) => node.parentElement === host || other.tagName === node.tagName);
      parts.unshift(`${node.tagName.toLowerCase()}-${same.indexOf(node)}`);
    }
    return parts.join('/');
  };

  await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
  const run = async (steps) => {
    ends.clear();
    const origin = performance.now() - 1000;
    let clock = 1000;
    const owner = createWordFadeState();
    owner.timeline.nowFn = () => clock;
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    const rows = [];
    for (const step of steps) {
      if (step.wait) await sleep(step.wait);
      await settle();
      clock = performance.now() - origin;
      if (step.text !== undefined) {
        const chunks = splitStreamingMarkdownChunks(repairStreamingMarkdown(step.text));
        if (chunks.length !== 1) throw new Error(`${JSON.stringify(step.text)} is no single chunk`);
        flushSync(() => root.render(createElement(StreamingMarkdownChunk, {
          key: 0, sourceKey: '0', content: chunks[0].content, remarkPlugins, rehypePlugins, components, wordFadeState: owner, emitSourceLines: false, wholeDocument: true,
        })));
        watch();
      }
      const spans = [...host.querySelectorAll('span[data-wf-key]')].map((span) => [span.getAttribute('data-wf-key'), span.classList.contains('stream-word')]);
      const items = [...host.querySelectorAll('li')].map((item) => {
        const own = item.getAnimations({ subtree: true }).filter((animation) => animation.effect.target === item && animation.effect.pseudoElement === '::marker');
        const state = own.length === 0 ? 'none' : own[0].playState === 'finished' ? 'finished' : 'running';
        const delay = own.length === 0 ? null : own[0].effect.getComputedTiming().delay;
        return [place(item, host), item.hasAttribute('data-stream-marker'), state, delay];
      }).sort(([a], [b]) => (a < b ? -1 : 1));
      rows.push({ now: clock, text: step.text ?? null, spans, items, settled: [...owner.timeline.settled].sort() });
    }
    flushSync(() => root.unmount());
    host.remove();
    return rows;
  };
  const all = drawn();
  // The page's first animations start late: one run warms it.
  await run(all[0]);
  const scenarios = [];
  for (const steps of all) scenarios.push([steps, await run(steps)]);
  return scenarios;
};

app.whenReady().then(async () => {
  session.defaultSession.webRequest.onBeforeRequest((details, done) => done({ cancel: !details.url.startsWith('about:') }));
  const win = new BrowserWindow({ show: false, webPreferences: { offscreen: true, backgroundThrottling: false } });
  await win.loadURL('about:blank');
  await win.webContents.insertCSS(fs.readFileSync(styles, 'utf8'));
  await win.webContents.executeJavaScript(fs.readFileSync(bundle, 'utf8'));
  const scenarios = await win.webContents.executeJavaScript(`(${page})()`);
  const rows = scenarios.map(([steps, read]) => [steps.map((step) => step.text ?? null), read]);
  fs.writeFileSync(out, JSON.stringify({ chrome: process.versions.chrome, scenarios: rows }));
  app.quit();
}).catch((error) => {
  console.error(error);
  app.exit(1);
});
