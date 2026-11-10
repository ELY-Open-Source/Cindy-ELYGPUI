// The source-labels golden: Cindy's `MessageSourceLabels` drawn in jsdom, each label read off the DOM.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const ENTRY = [
  "export { MessageSourceLabels } from './renderer/components/chat/MessageSourceLabels';",
  "export { sessionsStore } from './renderer/lib/sessionsStore';",
  "export { createRoot } from 'react-dom/client';",
  "export { act, createElement } from 'react';",
  "export { MemoryRouter, useLocation } from 'react-router-dom';",
  "export { default as i18next } from 'i18next';",
  "export { initReactI18next } from 'react-i18next';",
].join('\n');

/** `cases.json`: `[props, titles]`, the titles of the tasks the task list holds. */
export async function sourceLabels({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, cindy, src }) {
  const dir = path.join(fixtures, 'source-labels');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const entry = path.join(tmp, src, 'source-labels-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'source-labels.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, ...assets, define, jsx: 'automatic',
  });
  const { JSDOM } = createRequire(path.join(path.resolve(cindy), 'package.json'))('jsdom');
  const dom = new JSDOM('<!doctype html><div id="root"></div>', { url: 'http://localhost/' });
  const names = ['window', 'document', 'navigator', 'localStorage', 'HTMLElement', 'Node', 'MutationObserver'];
  const before = names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  for (const name of names) Object.defineProperty(globalThis, name, { value: dom.window[name], configurable: true });
  // The preload bridge: every call subscribes to nothing and never answers, as a host with no device link.
  const pending = Object.assign(() => {}, { then: () => pending, catch: () => pending, finally: () => pending });
  // Only the task list answers: the held tasks, for the bucket the store asks for.
  let held = [];
  const bridge = (at) => new Proxy(() => {}, {
    get: (_, key) => bridge(`${at}.${String(key)}`),
    apply: () => (at === '.localDb.sessions.list' ? Promise.resolve(held) : pending),
  });
  dom.window.electronAPI = bridge('');
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  try {
    const cindyUi = await import(pathToFileURL(bundle).href);
    const { MessageSourceLabels, sessionsStore, createRoot, act, createElement, MemoryRouter, useLocation, i18next, initReactI18next } = cindyUi;
    const common = JSON.parse(fs.readFileSync(path.join(fixtures, '..', '..', 'locales', 'en', 'common.json'), 'utf8'));
    await i18next.use(initReactI18next).init({
      lng: 'en', resources: { en: { common } }, defaultNS: 'common', ns: ['common'], interpolation: { escapeValue: false },
    });
    let location;
    const Probe = () => { const at = useLocation(); location = at.pathname + at.search; return null; };
    const read = (el, open) => {
      const svg = el.querySelector('svg');
      const avatar = el.querySelector('span[aria-hidden]');
      return {
        kind: el.dataset.messageOrigin,
        icon: svg ? [...svg.classList].find((name) => name !== 'lucide' && name.startsWith('lucide-')).slice(7) : 'bot',
        ...(avatar ? { initial: avatar.textContent, tint: avatar.style.backgroundColor } : {}),
        text: el.querySelector('span.truncate').textContent,
        tip: el.getAttribute('title'),
        open,
      };
    };
    const expected = [];
    for (const [props, titles] of cases) {
      sessionsStore.reset();
      held = Object.entries(titles ?? {}).map(([id, title]) => ({ id, title, status: 'active' }));
      await sessionsStore.ensureByFilter('active');
      for (const [id, title] of Object.entries(titles ?? {})) {
        if (sessionsStore.findById(id)?.title !== title) throw new Error(`task ${id} is not held`);
      }
      const draw = async (click) => {
        const container = dom.window.document.createElement('div');
        const root = createRoot(container);
        await act(async () => root.render(createElement(MemoryRouter, { initialEntries: ['/start'] },
          createElement(Probe), createElement(MessageSourceLabels, props))));
        const labels = [...container.querySelectorAll('[data-message-origin]')];
        const clicked = labels[click];
        if (clicked?.tagName === 'BUTTON') await act(async () => clicked.click());
        const out = labels.map((el) => read(el, null));
        await act(async () => root.unmount());
        return [out, clicked?.tagName === 'BUTTON' ? location : null];
      };
      const [labels] = await draw(-1);
      for (let n = 0; n < labels.length; n += 1) {
        const [, open] = await draw(n);
        if (open === '/start') throw new Error(`label ${n} of ${JSON.stringify(props)} is a button that went nowhere`);
        labels[n].open = open;
      }
      expected.push([[props, titles ?? {}], labels]);
    }
    write(path.join(dir, 'expected.json'), expected);
    return expected.length;
  } finally {
    for (const [name, descriptor] of before) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
    delete globalThis.IS_REACT_ACT_ENVIRONMENT;
    dom.window.close();
  }
}
