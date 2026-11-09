// `mapServerMessages` on `chat-message/cases.json`, from a bundle that exports the store's private function.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const wellFormed = (value) => {
  if (typeof value === 'string') return value.toWellFormed();
  if (Array.isArray(value)) return value.map(wellFormed);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, wellFormed(item)]));
  }
  return value;
};

export async function chatMessage({ esbuild, tmp, nodePaths, alias, fixtures, write }) {
  const store = path.join(tmp, 'apps/desktop/src/renderer/lib/makerChatStore.ts');
  fs.appendFileSync(store, '\nexport { mapServerMessages };\n');
  const entry = path.join(tmp, 'apps/desktop/src/chat-message-entry.ts');
  fs.writeFileSync(entry, "export { mapServerMessages } from './renderer/lib/makerChatStore';\n");
  const bundle = path.join(tmp, 'chat-message.mjs');
  const empty = Object.fromEntries(['.png', '.svg', '.css', '.webp', '.mp3', '.jpg', '.gif', '.woff2'].map((e) => [e, 'empty']));
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, loader: empty,
    define: { 'import.meta.env': JSON.stringify({ PROD: true, DEV: false, MODE: 'production' }) },
  });
  const { mapServerMessages } = await import(pathToFileURL(bundle).href);
  const dir = path.join(fixtures, 'chat-message');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const expected = cases.map(({ name, rows }) => {
    try {
      const mapped = mapServerMessages(structuredClone(rows));
      const undefinedKeys = mapped.map((row) => Object.keys(row).filter((key) => row[key] === undefined));
      return { name, rows, result: wellFormed(JSON.parse(JSON.stringify(mapped))), undefinedKeys };
    } catch (error) {
      return { name, rows, error: error.message };
    }
  });
  write(path.join(dir, 'expected.json'), expected);
  return expected.length;
}
