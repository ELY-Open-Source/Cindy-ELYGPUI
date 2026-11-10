// buildMessageStreamRenderProjection for a ready view, on Cindy's own functions.
import fs from 'node:fs';
import path from 'node:path';

const wellFormed = (value) => JSON.parse(JSON.stringify(value, (_, v) => (typeof v === 'string' ? v.toWellFormed() : v)));

/** A range's rows by the case's own order, `take` trimming them; the state and how to make it again. */
function detail(rows, summary, spec, mapRows) {
  const at = (id) => rows.findIndex((row) => row.id === id || (id.startsWith('history-live:') && row.clientId === id.slice(13)));
  const [first, last] = [at(summary.firstMessageId), at(summary.lastMessageId)];
  const range = first < 0 ? [] : rows.slice(first, last < first ? undefined : last + 1);
  const picked = spec.rows ? spec.rows.filter((index) => rows[index]) : range.slice(...(spec.take ?? [0])).map((row) => rows.indexOf(row));
  const ids = picked.map((index) => [index, spec.reid?.[rows[index].clientId] ?? rows[index].id]);
  const messages = mapRows(ids.map(([index, id]) => ({ ...rows[index], id })));
  const state = {
    revision: spec.revision ?? summary.revision, lastMessageId: messages.at(-1)?.id ?? '',
    loading: spec.loading === true, complete: spec.complete !== false, error: spec.error ?? null,
  };
  return [{ ...state, messages }, { ...state, rows: ids }];
}

export function historyRender({ fixtures }, { stream, messageWindow, mapServerMessages }) {
  const dir = path.join(fixtures, 'history-render');
  const { rows: sets, cases } = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  // The store's `mapRows`.
  const mapRows = (rows) => mapServerMessages(structuredClone(rows)).map((row) => ({ ...row, id: row.id ?? row.clientId, createdAt: row.createdAt ?? '' }));
  const expected = cases.map((input) => {
    const source = sets[input.rows];
    const items = messageWindow.mapHistoryViewMessages(messageWindow.projectHistoryView(source, input.streaming === true, true), mapRows);
    const summaries = messageWindow.historyWorkSummaries(items);
    const expanded = (input.expand ?? []).flatMap((index) => (summaries[index] ? [summaries[index].key] : []));
    const read = (input.details ?? []).flatMap((spec) => (summaries[spec.of] ? [[summaries[spec.of].key, detail(source, summaries[spec.of], spec, mapRows)]] : []));
    const details = read.map(([key, [, again]]) => [key, again]);
    const snapshot = { items, details: new Map(read.map(([key, [state]]) => [key, state])), expanded: new Set(expanded), nextCursor: null, hasMore: false, loading: false, ready: true, error: null, stale: false };
    let calls = [];
    const view = {
      retainExpanded: (keys) => { calls.push([...keys]); return () => {}; },
      setExpanded: (key, open) => { calls.push([key, open]); },
      loadDetails: async (summary) => { calls.push(summary.key); },
      getSnapshot: () => snapshot,
    };
    const called = (run) => { calls = []; run(); return calls; };
    const deferred = (work) => (work ? {
      key: work.key, expanded: work.expanded, revision: work.revision, previewComplete: work.previewComplete,
      lines: 'lines' in work, livePreview: work.livePreview, awaitingDetails: work.awaitingDetails, loading: work.loading, failed: work.failed,
      visible: [[true, false], [true, true], [false, true], [false, false]].map(([full, preview]) => called(() => work.setVisible(full, preview))[0]),
      toggle: called(work.toggle), retry: called(work.retry),
    } : null);
    const shown = (item) => ({
      type: item.type, key: item.key,
      ...(item.type === 'message' ? { message: item.message } : {}),
      ...(item.type === 'tool_segment' ? { calls: item.toolCalls.map((call) => call.clientId) } : {}),
      ...(item.type === 'agent_task' ? { call: item.toolCall.clientId } : {}),
      ...(item.type === 'work_group' ? {
        durationMs: item.durationMs ?? null, isStreaming: item.isStreaming, startedAtMs: item.startedAtMs ?? null,
        deferred: deferred(item.deferred), children: item.children.map(shown),
      } : {}),
    });
    let result;
    try {
      result = stream.buildMessageStreamRenderProjection({
        historyView: view, historySnapshot: snapshot, historyLiveMessages: (input.live ?? []).map(({ row, set }) => ({ ...mapRows([row])[0], ...set })),
        isSessionStreaming: input.sessionStreaming === true,
        handoff: { pending: new Set(input.pending ?? []), tail: new Set(input.tail ?? []) },
        renderOptions: { workingDir: input.workingDir ?? '' }, generatedFilesItemCache: new Map(),
        buildRaw: () => { throw new Error('the raw fallback ran'); },
      }).items.map(shown);
    } catch (error) {
      result = { throws: String(error) };
    }
    return [{ expanded, details }, result];
  });
  const lines = wellFormed(expected).map((entry) => JSON.stringify(entry));
  fs.writeFileSync(path.join(dir, 'expected.json'), `[\n${lines.join(',\n')}\n]\n`);
  const all = JSON.stringify(expected);
  const count = (text) => all.split(text).length - 1;
  return `${expected.length} (${count('"deferred":{')} deferred, ${count('"thinkingRedacted":true')} placeholders, ${count('"throws"')} throws)`;
}
