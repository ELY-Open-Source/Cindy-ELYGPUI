// The stream-fade golden: Cindy's `StreamingMarkdownChunk` in Blink, its animations run by Cindy's CSS.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ENTRY = [
  "export { StreamingMarkdownChunk } from './renderer/components/chat/StreamingMarkdownChunk';",
  "export { createWordFadeState } from './renderer/components/chat/rehypeStreamWordFade';",
  "export { repairStreamingMarkdown } from './renderer/components/chat/repairStreamingMarkdown';",
  "export { splitStreamingMarkdownChunks } from './renderer/components/chat/streamingMarkdownChunks';",
  "export { createRoot } from 'react-dom/client';",
  "export { flushSync } from 'react-dom';",
  "export { createElement } from 'react';",
  "export { default as remarkGfm } from 'remark-gfm';",
].join('\n');
// The tokens the rules read, and the list styles Tailwind gives Cindy's `ul` and `ol`.
const BASE = ':root{--motion-fast:150ms;--motion-ease-out:cubic-bezier(0.16,1,0.3,1)}ul,ol{list-style:none}.list-disc{list-style-type:disc}.list-decimal{list-style-type:decimal}';

/** `expected.json`: each scenario's steps `{now, text, spans, items, settled}`, an item `[place, attribute, animation, delay]`, as Blink ran them. */
export async function streamFade({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, cindy, src }) {
  const entry = path.join(tmp, src, 'stream-fade-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'stream-fade.js');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'iife', globalName: 'cindyFade', platform: 'browser', outfile: bundle, logLevel: 'error',
    nodePaths, alias, loader: assets.loader, plugins: assets.plugins, jsx: 'automatic',
    define: { ...define, 'process.env.NODE_ENV': '"production"' },
  });
  const css = fs.readFileSync(path.join(tmp, src, 'renderer', 'styles', 'globals.css'), 'utf8');
  const from = css.indexOf('\n.stream-word {');
  const last = css.indexOf('@keyframes stream-word-in', from);
  const to = css.indexOf('\n}\n', last);
  if (from < 0 || last < 0 || to < 0) throw new Error('the fade rules are not where they were');
  const styles = path.join(tmp, 'stream-fade.css');
  fs.writeFileSync(styles, BASE + css.slice(from, to + 3));
  const out = path.join(tmp, 'stream-fade.json');
  const electron = path.join(path.resolve(cindy), 'node_modules', '.bin', 'electron');
  const runner = path.join(path.dirname(fileURLToPath(import.meta.url)), 'streamFadeRun.cjs');
  execFileSync(electron, ['--no-sandbox', runner, bundle, styles, out, path.join(tmp, 'stream-fade-profile')], { stdio: ['ignore', 'inherit', 'inherit'] });
  const expected = JSON.parse(fs.readFileSync(out, 'utf8'));
  write(path.join(fixtures, 'stream-fade', 'expected.json'), expected);
  return expected.scenarios.reduce((count, [, steps]) => count + steps.length, 0);
}
