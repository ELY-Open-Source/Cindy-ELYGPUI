// The streamed-chunks golden: Cindy's `StreamingMarkdownChunk` rows mounted in jsdom, in Cindy's Electron for V8's words.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { exporter } from './markdownHast.mjs';

const ENTRY = [
  "export { REMARK_PLUGINS_PRIVILEGED, golden } from './renderer/components/chat/MarkdownRenderer';",
  "export { StreamingMarkdownChunk } from './renderer/components/chat/StreamingMarkdownChunk';",
  "export { createWordFadeState } from './renderer/components/chat/rehypeStreamWordFade';",
  "export { repairStreamingMarkdown } from './renderer/components/chat/repairStreamingMarkdown';",
  "export { splitStreamingMarkdownChunks } from './renderer/components/chat/streamingMarkdownChunks';",
  "export { createRoot } from 'react-dom/client';",
  "export { act, createElement, Fragment } from 'react';",
  "export { MemoryRouter } from 'react-router-dom';",
].join('\n');

/** `expected.json`: each stream's start clock, its text and steps `[dt, code points shown, spans, timeline]`. */
export async function streamedChunks({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, cindy, src }) {
  const entry = path.join(tmp, src, 'streamed-chunks-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'streamed-chunks.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, loader: assets.loader, plugins: [exporter, ...assets.plugins], define, jsx: 'automatic', keepNames: true,
  });
  const out = path.join(tmp, 'streamed-chunks.json');
  const electron = path.join(path.resolve(cindy), 'node_modules', '.bin', 'electron');
  const runner = path.join(path.dirname(fileURLToPath(import.meta.url)), 'streamedChunksRun.mjs');
  execFileSync(electron, [runner, bundle, path.resolve(cindy), out], { env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, stdio: ['ignore', 'inherit', 'inherit'] });
  const expected = JSON.parse(fs.readFileSync(out, 'utf8'));
  write(path.join(fixtures, 'streamed-chunks', 'expected.json'), expected);
  return expected.streams.reduce((count, [, , steps]) => count + steps.length, 0);
}
