// The streaming-markdown golden: each text as it grows, one UTF-16 unit at a time.
const PARTS = [
  'alpha', ' beta', '\n', '\n\n', '\r\n', '\r\n\r\n', '**', '*', '***', '`', '```', '````ts', '~~~', '![', '[', ']', '](', ')',
  'http://a.b/c', '\\', '- ', '* ', '1. ', '12) ', '> ', '# ', '## ', '===', '---', '$$', ':::note', ':::', '<div>', '<!--',
  '[ref]: ', '    ', '\t', ' ', ' ', ' ', '\r', '字', '😀', 'x*y', '_',
];

/** A seeded mix of the parts: mulberry32 picks each. */
function mixed(seed) {
  let state = seed >>> 0;
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const count = 4 + Math.floor(next() * 14);
  return Array.from({ length: count }, () => PARTS[Math.floor(next() * PARTS.length)]).join('');
}

/** `fns`: Cindy's `repair` and `split`; the stream of a text: each repair that changed it, each split's starts. */
export function streamed({ repair, split }) {
  const stream = (text) => {
    const [repairs, starts] = [[], []];
    for (let length = 0; length <= text.length; length++) {
      const code = text.charCodeAt(length - 1);
      // Rust strings hold no half of a pair.
      if (code >= 0xd800 && code < 0xdc00) continue;
      const prefix = text.slice(0, length);
      const repaired = repair(prefix);
      if (repaired !== prefix) repairs.push([length, repaired]);
      const parts = split(prefix);
      if (parts.map((part) => part.content).join('') !== prefix) throw new Error(`chunks lose text: ${JSON.stringify(prefix)}`);
      if (parts.length > 1) starts.push([length, parts.map((part) => part.start)]);
    }
    return { repairs, starts };
  };
  return { stream, mix: (seed) => { const text = mixed(seed); return { text, ...stream(text) }; } };
}
