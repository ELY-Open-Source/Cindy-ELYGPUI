// `mergeMessages` and `confirmRemoteUsers` on `merge-messages/cases.json`; writes results only, keeping `-0` inputs.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

// `{ "$ref": "server:0" }` is that row; `"server:0/toolInput"` that row's field.
function resolve(value, lists) {
  if (Array.isArray(value)) return value.map((item) => resolve(item, lists));
  if (!value || typeof value !== 'object') return value;
  if (typeof value.$ref === 'string') {
    const [where, key] = value.$ref.split('/');
    const [list, index] = where.split(':');
    const row = lists[list][Number(index)];
    return key === undefined ? row : row[key];
  }
  return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, resolve(v, lists)]));
}

function rows(list, lists, name) {
  lists[name] = [];
  for (const row of list) lists[name].push(resolve(row, lists));
  return lists[name];
}

// Which input object each result row is, and the keys it holds as `undefined`.
function describe(result, inputs, same) {
  const origin = result.map((row) => {
    for (const [name, list] of Object.entries(inputs)) {
      const index = list.indexOf(row);
      if (index >= 0) return `${name}:${index}`;
    }
    return null;
  });
  const undefinedKeys = result.map((row) => Object.keys(row).filter((key) => row[key] === undefined));
  return { result: JSON.parse(JSON.stringify(result)), origin, undefinedKeys, same };
}

export async function mergeMessages({ esbuild, tmp, nodePaths, alias, fixtures, write }) {
  const store = path.join(tmp, 'apps/desktop/src/renderer/lib/makerChatStore.ts');
  fs.appendFileSync(store, '\nexport { mergeMessages };\n');
  const entry = path.join(tmp, 'apps/desktop/src/merge-messages-entry.ts');
  fs.writeFileSync(entry, [
    "export { mergeMessages } from './renderer/lib/makerChatStore';",
    "export { confirmRemoteUsers } from './renderer/lib/remoteUserHandoff';",
  ].join('\n'));
  const bundle = path.join(tmp, 'merge-messages.mjs');
  const empty = Object.fromEntries(['.png', '.svg', '.css', '.webp', '.mp3', '.jpg', '.gif', '.woff2'].map((e) => [e, 'empty']));
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, loader: empty,
    define: { 'import.meta.env': JSON.stringify({ PROD: true, DEV: false, MODE: 'production' }) },
  });
  const cindy = await import(pathToFileURL(bundle).href);
  const dir = path.join(fixtures, 'merge-messages');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const expected = cases.map((c) => {
    const lists = {};
    if (c.fn === 'confirm') {
      const messages = rows(c.messages, lists, 'messages');
      const result = cindy.confirmRemoteUsers(messages, new Set(c.confirmed));
      return { name: c.name, ...describe(result, lists, result === messages) };
    }
    const server = rows(c.server, lists, 'server');
    const existing = rows(c.existing, lists, 'existing');
    const { addOnlyExcept, ...options } = c.options ?? {};
    if (addOnlyExcept) options.addOnlyExcept = new Set(addOnlyExcept);
    const result = cindy.mergeMessages(server, existing, options, c.rowsOrder);
    return { name: c.name, ...describe(result, lists, result === existing) };
  });
  write(path.join(dir, 'expected.json'), expected);
  return expected.length;
}
