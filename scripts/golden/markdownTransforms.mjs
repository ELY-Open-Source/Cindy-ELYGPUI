// The markdown-transforms golden: each tree transformer alone, after the parse.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const ENTRY = [
  "export { REMARK_PLUGINS_PRIVILEGED } from './renderer/components/chat/MarkdownRenderer';",
  "export { unified } from 'unified';",
  "export { default as remarkParse } from 'remark-parse';",
].join('\n');

const PARSE = 3;
// Where each transformer stands in Cindy's list, by the name its source gives it.
const TRANSFORMS = { truncate: [4, 'remarkTruncateCjkUrls'], images: [5, 'remarkHtmlImages'], sessions: [7, 'remarkSessionLinks'] };

/** A node less its place: an image's hast size as `width` and `height`, a link's title always there. */
function bare(node) {
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === 'position' || key === 'data') continue;
    if (key === 'checked' && node.type !== 'listItem') continue;
    out[key] = key === 'children' ? value.map(bare) : value;
  }
  if (node.type === 'link' && out.title === undefined) out.title = null;
  if (node.type === 'image') Object.assign(out, node.data?.hProperties);
  return out;
}

/** `cases.json`: Markdown sources. */
export async function markdownTransforms({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, src }) {
  const dir = path.join(fixtures, 'markdown-transforms');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const entry = path.join(tmp, src, 'markdown-transforms-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'markdown-transforms.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, ...assets, define, jsx: 'automatic', keepNames: true,
  });
  const { REMARK_PLUGINS_PRIVILEGED: plugins, unified, remarkParse } = await import(pathToFileURL(bundle).href);
  const parser = unified().use(remarkParse).use(plugins.slice(0, PARSE));
  const run = Object.entries(TRANSFORMS).map(([key, [index, name]]) => {
    if (plugins[index].name !== name) throw new Error(`plugin ${index} is ${plugins[index].name}, not ${name}`);
    return [key, plugins[index]()];
  });
  write(path.join(dir, 'expected.json'), cases.map((text) => [
    text,
    Object.fromEntries(run.map(([key, transform]) => {
      const tree = parser.parse(text);
      transform(tree);
      return [key, bare(tree)];
    })),
  ]));
  return cases.length;
}
