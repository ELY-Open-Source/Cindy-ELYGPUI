// The markdown-hast golden: the tree react-markdown hands Cindy's renderers.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const ENTRY = [
  "export { REMARK_PLUGINS, REMARK_PLUGINS_PRIVILEGED, golden } from './renderer/components/chat/MarkdownRenderer';",
  "export { default as Markdown } from 'react-markdown';",
  "export { regex as slugRegex } from 'github-slugger/regex.js';",
].join('\n');
// What the renderer keeps to itself, exported for the golden's bundle alone.
const EXPORT = '\nexport const golden = { REHYPE_PLUGINS, trustedUrlTransform, previewSafeUrlTransform, allowlistedImageUrlTransform, resolvedLocalFromResult, shouldRenderCodeReferenceLabel, parseSessionCardHref };\n';
const exporter = {
  name: 'renderer-exports',
  setup(build) {
    build.onLoad({ filter: /[\\/]MarkdownRenderer\.tsx$/ }, (args) => ({
      contents: fs.readFileSync(args.path, 'utf8') + EXPORT, loader: 'tsx',
    }));
  },
};

/** The plugin at `index`, which must carry `name`. */
function named(list, index, name) {
  const plugin = [list[index]].flat()[0];
  if (plugin.name !== name) throw new Error(`plugin ${index} is ${plugin.name}, not ${name}`);
  return list[index];
}

/** A hast node less its place and data. */
function bare(node) {
  if (node.type === 'text') return node.value;
  if (node.type !== 'element') throw new Error(`unexpected ${node.type} node`);
  return { tag: node.tagName, props: node.properties, children: node.children.map(bare) };
}

/** The code points github-slugger drops from a heading's text. */
function dropped(regex) {
  const out = [];
  for (let cp = 0; cp <= 0x10ffff; cp += 1) {
    if (cp >= 0xd800 && cp <= 0xdfff) continue;
    const text = String.fromCodePoint(cp);
    const left = text.replace(regex, '');
    if (left === text) continue;
    if (left !== '') throw new Error(`half of U+${cp.toString(16)} is dropped`);
    const last = out.at(-1);
    if (last && last[1] === cp - 1) last[1] = cp;
    else out.push([cp, cp]);
  }
  return out;
}

/** `cases.json`: Markdown sources. */
export async function markdownHast({ esbuild, tmp, nodePaths, alias, define, assets, fixtures, write, src }) {
  const dir = path.join(fixtures, 'markdown-hast');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const entry = path.join(tmp, src, 'markdown-hast-entry.ts');
  fs.writeFileSync(entry, ENTRY);
  const bundle = path.join(tmp, 'markdown-hast.mjs');
  await esbuild.build({
    entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error',
    nodePaths, alias, loader: assets.loader, plugins: [exporter, ...assets.plugins], define, jsx: 'automatic', keepNames: true,
  });
  const { REMARK_PLUGINS, REMARK_PLUGINS_PRIVILEGED, golden, Markdown, slugRegex } = await import(pathToFileURL(bundle).href);
  write(path.join(fixtures, '..', '..', 'resources', 'slug-classes.json'), { dropped: dropped(slugRegex) });
  // A candidate lit by the resolver's answer, as the renderer's hook lights it.
  const lit = JSON.parse(fs.readFileSync(path.join(fixtures, 'smart-paths', 'lit-cases.json'), 'utf8'));
  write(path.join(fixtures, 'smart-paths', 'lit.json'), lit.map(([target, result]) => [target, result, golden.resolvedLocalFromResult(target, result)]));
  // Whether a resolved target's label reads as a file reference, which makes it a chip.
  const labels = JSON.parse(fs.readFileSync(path.join(fixtures, 'smart-paths', 'label-cases.json'), 'utf8'));
  write(path.join(fixtures, 'smart-paths', 'label.json'), labels.map(([target, label]) => [target, label, golden.shouldRenderCodeReferenceLabel(target, label)]));
  // A session-card link's target, or null for a link that draws as any other.
  const cards = JSON.parse(fs.readFileSync(path.join(fixtures, 'session-card', 'cases.json'), 'utf8'));
  write(path.join(fixtures, 'session-card', 'expected.json'), cards.map((href) => [href, golden.parseSessionCardHref(href)]));
  // Strict inline math comes with 9.8, as KaTeX and the highlighter do with theirs.
  const less = (list) => list.filter((plugin) => plugin !== named(list, 3, 'remarkStrictInlineMath'));
  let seen;
  const rehypePlugins = [
    named(golden.REHYPE_PLUGINS, 0, 'rehypeSlug'), named(golden.REHYPE_PLUGINS, 4, 'rehypeFencedCodeMarker'),
    () => (tree) => { seen = tree; },
  ];
  // react-markdown's own run: its post step changes the tree the last plugin saw.
  const run = (text, remarkPlugins, urlTransform) => {
    Markdown({ children: text, remarkPlugins, rehypePlugins, urlTransform, skipHtml: true });
    return seen.children.map(bare);
  };
  // Each case: the source, the trusted tree, then the other two faces, or null where they match it.
  write(path.join(dir, 'expected.json'), cases.map((text) => {
    const trusted = run(text, less(REMARK_PLUGINS_PRIVILEGED), golden.trustedUrlTransform);
    const base = JSON.stringify(trusted);
    const other = (transform) => {
      const tree = run(text, less(REMARK_PLUGINS), transform);
      return JSON.stringify(tree) === base ? null : tree;
    };
    return [text, trusted, other(golden.previewSafeUrlTransform), other(golden.allowlistedImageUrlTransform)];
  }));
  return cases.length;
}
