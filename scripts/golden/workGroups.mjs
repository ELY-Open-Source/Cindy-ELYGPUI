// groupWorkRuns over buildRenderItems' items, and the stream's row scans, on Cindy's own functions.
import fs from 'node:fs';
import path from 'node:path';

import { options } from './renderItems.mjs';

const wellFormed = (value) => JSON.parse(JSON.stringify(value, (_, v) => (typeof v === 'string' ? v.toWellFormed() : v)));

export function workGroups({ fixtures }, { stream, groups }) {
  const dir = path.join(fixtures, 'work-groups');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const render = JSON.parse(fs.readFileSync(path.join(fixtures, 'render-items', 'expected.json'), 'utf8')).buildRenderItems;
  const built = (rows, opts) => stream.buildRenderItems(rows, undefined, undefined, options(opts)).items;
  /** An item by key and times; a group with what it holds. */
  const shown = (item) => ({
    type: item.type, key: item.key, startMs: groups.renderItemStartMs(item), endMs: groups.renderItemEndMs(item),
    ...(item.type === 'work_group' ? {
      durationMs: item.durationMs ?? null, isStreaming: item.isStreaming, startedAtMs: item.startedAtMs ?? null,
      children: item.children.map(shown),
    } : {}),
  });
  const grouped = (rows, opts, streaming) => groups.groupWorkRuns(built(rows, opts), streaming);
  const ids = (rows) => [...new Set(rows.map((row) => row.clientId).filter((id) => typeof id === 'string')), 'absent'];
  // A render-items case goes by its index, a case of this fixture by its index in a list.
  const refs = [
    ...render.flatMap(([args, result], index) => (Array.isArray(result) ? [[index, args[0], args[3]]] : [])),
    ...cases.rows.map(([rows, opts], index) => [[index], rows, opts]),
  ];
  const expected = {
    groups: refs.map(([ref, rows, opts]) => [ref,
      [false, true].map((streaming) => grouped(rows, opts, streaming).map(shown)),
      [...stream.collectTurnFinalAssistantClientIds(rows)],
      [stream.findFirstUserMessageClientId(rows, false), stream.findFirstUserMessageClientId(rows, true), stream.findLastUserMessageClientId(rows)]]),
    ids: cases.rows.map(([rows, opts]) => ids(rows).map((id) => [id,
      stream.assistantHasFollowingUserBoundary(rows, id),
      [false, true].map((streaming) => grouped(rows, opts, streaming).map((item) => stream.renderItemContainsClientId(item, id)))])),
    forks: cases.forks.map(([rows, opts, fork]) =>
      [false, true].map((streaming) => stream.insertForkOriginItem(grouped(rows, opts, streaming), fork ?? undefined)
        .map((item) => (item.type === 'fork_origin' ? item : item.key)))),
    messageTs: cases.messageTs.map((row) => groups.messageTs(row)),
  };
  // One case a line.
  const lists = Object.entries(wellFormed(expected)).map(([name, list]) =>
    `${JSON.stringify(name)}: [\n${list.map((entry) => JSON.stringify(entry)).join(',\n')}\n]`);
  fs.writeFileSync(path.join(dir, 'expected.json'), `{\n${lists.join(',\n')}\n}\n`);
  return expected.groups.length * 2 + expected.ids.length + expected.forks.length + expected.messageTs.length;
}
