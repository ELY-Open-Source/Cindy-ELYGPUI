// Cindy's path resolver over real folders: each case's tree is built, then its steps run in order.
import fs from 'node:fs';
import path from 'node:path';

export async function pathResolver({ tmp, fixtures, write }, resolver) {
  const dir = path.join(fixtures, 'path-resolver');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const expected = [];
  for (const [n, { name, files = [], steps }] of cases.entries()) {
    const root = path.join(tmp, `resolver-${n}`);
    const real = (text) => text.replaceAll('<root>', root);
    const shown = (value) => JSON.parse(JSON.stringify(value ?? null).replaceAll(root, '<root>'));
    const put = (file) => {
      fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
      fs.writeFileSync(path.join(root, file), '');
    };
    for (const folder of ['workspace', 'home']) fs.mkdirSync(path.join(root, folder), { recursive: true });
    files.forEach(put);
    resolver._clearResolveCache();
    const tidy = (result) => ({ status: result.status, candidates: result.candidates, kind: result.kind ?? null });
    const out = [];
    for (const step of steps) {
      const blocked = step.blocked === 'all' ? null : (step.blocked ?? []).map(real);
      const options = {
        homeDir: path.join(root, 'home'), maxCandidates: step.max, maxDepth: step.depth, refresh: step.refresh,
        isPathAllowed: (file) => blocked !== null && !blocked.includes(file),
      };
      const folder = real(step.dir ?? '<root>/workspace');
      const hrefs = (list) => [...new Set(list)];
      const each = (list, results) => hrefs(list).map((href) => [href, tidy(results[real(href)])]);
      if (step.write) put(step.write);
      else if (step.remove) fs.rmSync(path.join(root, step.remove));
      else if (step.link) fs.symlinkSync(step.link[1], path.join(root, step.link[0]));
      if (step.find !== undefined) out.push(resolver.bfsFindBySuffix(folder, real(step.find), options));
      else if (step.many) {
        const found = await resolver.bfsFindManyBySuffix(folder, step.many.map(real), options);
        out.push(hrefs(step.many).map((href) => [href, found.get(real(href))]));
      } else if (step.decode !== undefined) out.push(resolver.safeDecodeHrefForLookup(step.decode));
      else if (step.resolve !== undefined) out.push(tidy(await resolver.resolveWorkspacePath(real(step.resolve), folder, options)));
      else if (step.batch) out.push(each(step.batch, await resolver.resolveWorkspacePathBatch(step.batch.map(real), folder, options)));
      else if (step.cached !== undefined) out.push(tidy(await resolver.resolveWorkspacePathCached(real(step.cached), folder, options)));
      else if (step.batchCached) {
        out.push(each(step.batchCached, await resolver.resolveWorkspacePathBatchCached(step.batchCached.map(real), folder, options, step.revalidate === true)));
      } else out.push(null);
    }
    expected.push({ name, files, steps, out: shown(out) });
  }
  write(path.join(dir, 'expected.json'), expected);
  return expected.reduce((n, { steps }) => n + steps.length, 0);
}
