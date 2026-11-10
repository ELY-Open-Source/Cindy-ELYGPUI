// buildRenderItems on the rows Cindy's tests build, with the options a local history uses.
import { recorded } from './recorded.mjs';

const tests = 'apps/desktop/src/renderer/__tests__';
const names = ['buildRenderItems', 'isGeneratedFilesTurnSealed'];
const relative = ['buildRenderItemsKeyStability', 'ghostCardRenderItems', 'handleStreamEventSubagentMeta',
  'remoteHistoryHandoff',
  'subagentDurableStatusRenderItems', 'subagentInternalMessageFilter', 'toolSegmentSettledIds', 'workGroupDeliveryProse',
  'workGroupDurationAnchor', 'workGroupHistoryGap', 'workGroupRunningSubagent', 'workflowCardStreamPlacement'];
const aliased = ['streamingMessageProjection', 'taskSwitchProjectionCache'];
const TESTS = Object.fromEntries([
  ...relative.map((name) => [`${tests}/${name}.test.ts`, { '../components/chat/MessageStream': names }]),
  ...aliased.map((name) => [`${tests}/${name}.test.ts`, { '@/components/chat/MessageStream': names }]),
]);

const pick = (o, keys) => JSON.parse(JSON.stringify(Object.fromEntries(keys.map((k) => [k, o[k]])),
  (_, v) => (typeof v === 'string' ? v.toWellFormed() : v)));
const media = (m) => pick(m, ['kind', 'url', 'modelFile', 'audioTrack', 'audioInCard', 'imageInCard']);
// A file that is ready reads the same with the key or without it.
const file = (f) => pick({ ...f, ready: f.ready === false ? false : undefined }, ['path', 'name', 'source', 'ready', 'artifact', 'artifactConfirmed']);

/** An item with rows as indices into `rows` and maps as entry lists. */
function shown(item, rows) {
  const at = (row) => rows.indexOf(row);
  switch (item.type) {
    case 'message': return { type: item.type, key: item.key, message: at(item.message) };
    case 'agent_plan': return pick(item, ['type', 'key', 'todos', 'sourceClientIds', 'createdAt']);
    case 'tool_segment': return {
      type: item.type, key: item.key, toolCalls: item.toolCalls.map(at), resultMap: [...item.resultMap],
      resultTsMap: [...item.resultTsMap], settledIds: [...item.settledIds],
    };
    case 'agent_task': return { ...pick(item, ['type', 'key', 'persistedStatus', 'result', 'resultTsMs']), toolCall: at(item.toolCall) };
    case 'tool_media': return { type: item.type, key: item.key, items: item.items.map(media) };
    case 'generated_files': return { ...pick(item, ['type', 'key', 'turnStartMs', 'turnEndMs', 'turnSealed']), files: item.files.map(file) };
    default: throw new Error(`an item the port does not build: ${item.type}`);
  }
}

// Markdown image targets come with the Markdown renderer (9.6), voice rows with 22.0.
const later = (row) => row?.systemCardType === 'voice-call' || (row?.role === 'assistant'
  && typeof row.content === 'string' && (row.content.includes('![') || /<img/i.test(row.content)));
const keep = (fn, args) => fn !== 'buildRenderItems' || !(Array.isArray(args[0]) && args[0].some(later));

export function renderItems(golden, { buildRenderItems, isGeneratedFilesTurnSealed }) {
  return recorded({ ...golden, name: 'render-items', tests: TESTS, keep, fns: {
    // Task updates, ghost cards and the other options come with their chapters.
    buildRenderItems: (rows, _updates, _cards, opts) => {
      const kept = pick(opts ?? {}, ['historyWindowIncomplete', 'workingDir', 'historyArtifacts']);
      try {
        return buildRenderItems(rows, undefined, undefined, kept).items.map((item) => shown(item, rows));
      } catch (error) {
        return { throws: error.message };
      }
    },
    isGeneratedFilesTurnSealed: (slice, following) => isGeneratedFilesTurnSealed(slice, following ?? undefined),
  } });
}
