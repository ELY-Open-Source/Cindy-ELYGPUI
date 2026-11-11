// The hook-card golden: Cindy's `HookTaskCard` drawn in jsdom, its context unfolded, read off the DOM.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const ENTRY = [
  "export { default as HookTaskCard } from './renderer/components/chat/HookTaskCard';",
  "export { createRoot } from 'react-dom/client';",
  "export { act, createElement } from 'react';",
  "export { default as i18next } from 'i18next';",
  "export { initReactI18next } from 'react-i18next';",
].join('\n');

/** The header's icon: a lucide name, or the brand whose path it draws. */
function icon(svg) {
  const lucide = [...svg.classList].find((name) => name !== 'lucide' && name.startsWith('lucide-'));
  if (lucide) return lucide.slice(7);
  return svg.querySelector('path').getAttribute('d').startsWith('M5.042') ? 'slack' : 'x';
}

/** `cases.json`: `[hookSource, groupContext]`, handed on as `UserMessage` hands them. */
export async function hookCard({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, cindy, src }) {
  const dir = path.join(fixtures, 'hook-card');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const entry = path.join(tmp, src, 'hook-card-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'hook-card.mjs');
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
  try {
    const { HookTaskCard, createRoot, act, createElement, i18next, initReactI18next } = await import(pathToFileURL(bundle).href);
    const common = JSON.parse(fs.readFileSync(path.join(fixtures, '..', '..', 'locales', 'en', 'common.json'), 'utf8'));
    await i18next.use(initReactI18next).init({
      lng: 'en', resources: { en: { common } }, defaultNS: 'common', ns: ['common'], interpolation: { escapeValue: false },
    });
    const expected = [];
    for (const [source, groupContext] of cases) {
      const props = {
        im: source.im,
        origin: source.origin,
        userText: 'body',
        threadContext: source.threadContext,
        groupContext,
        replyContext: source.contextSnapshot?.replyContext,
        groupMessageCount: source.contextSnapshot?.groupMessageCount,
        replyMessageCount: source.contextSnapshot?.replyMessageCount,
      };
      const container = dom.window.document.createElement('div');
      const root = createRoot(container);
      await act(async () => root.render(createElement(HookTaskCard, props)));
      const heading = container.querySelector('span.truncate');
      const toggle = container.querySelector('button[aria-expanded]');
      if (toggle) await act(async () => toggle.click());
      const sections = [...container.querySelectorAll('section')].map((section) => ({
        heading: section.querySelector('h4').textContent,
        lines: [...section.querySelectorAll('div.whitespace-pre-wrap')].map((line) => {
          const author = line.querySelector('span.font-semibold');
          return author ? [author.textContent, line.textContent] : [null, line.textContent];
        }),
      }));
      if (toggle && sections.length === 0) throw new Error(`no context unfolded for ${JSON.stringify(source)}`);
      expected.push([[source, groupContext], {
        icon: icon(container.querySelector('svg')),
        label: heading.textContent,
        title: heading.getAttribute('title'),
        context: toggle ? { label: toggle.textContent, sections } : null,
      }]);
      await act(async () => root.unmount());
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
