// The markdown-tree golden: the parsed mdast, and remark-cjk-friendly's character classes.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const ENTRY = [
  "export { REMARK_PLUGINS } from './renderer/components/chat/MarkdownRenderer';",
  "export { unified } from 'unified';",
  "export { default as remarkParse } from 'remark-parse';",
  "export { classifyCharacter } from 'micromark-extension-cjk-friendly-util';",
].join('\n');

// The plugins that extend the parse; the rest of the list transforms its tree.
const PARSE = 3;

/** A node as remark holds it, less its place, its hast hints and a `checked` off a list item. */
function bare(node) {
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === 'position' || key === 'data') continue;
    if (key === 'checked' && node.type !== 'listItem') continue;
    out[key] = key === 'children' ? value.map(bare) : value;
  }
  return out;
}

/** Sorted, inclusive ranges of the code points `test` holds. */
function ranges(test) {
  const out = [];
  for (let cp = 0; cp <= 0x10ffff; cp += 1) {
    if ((cp >= 0xd800 && cp <= 0xdfff) || !test(cp)) continue;
    const last = out.at(-1);
    if (last && last[1] === cp - 1) last[1] = cp;
    else out.push([cp, cp]);
  }
  return out;
}

/** `cases.json`: Markdown sources. */
export async function markdownTree({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, src }) {
  const dir = path.join(fixtures, 'markdown-tree');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const entry = path.join(tmp, src, 'markdown-tree-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'markdown-tree.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, ...assets, define, jsx: 'automatic',
  });
  const { REMARK_PLUGINS, unified, remarkParse, classifyCharacter } = await import(pathToFileURL(bundle).href);
  const parser = unified().use(remarkParse).use(REMARK_PLUGINS.slice(0, PARSE));
  write(path.join(dir, 'expected.json'), cases.map((text) => [text, bare(parser.parse(text))]));
  write(path.join(fixtures, '..', '..', 'resources', 'attention-classes.json'), {
    unicode: process.versions.unicode,
    whitespace: ranges((cp) => classifyCharacter(cp) === 1),
    selector: ranges((cp) => classifyCharacter(cp) === 16384),
    ivs: ranges((cp) => classifyCharacter(cp) === 8192),
    cjk: ranges((cp) => (classifyCharacter(cp) & 4096) !== 0),
    punctuation: ranges((cp) => (classifyCharacter(cp) & 2) !== 0),
  });
  return cases.length;
}
