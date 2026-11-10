// The functions of the sent-text golden, over Cindy's `UserMessage.tsx` and `chatQuotes.ts`.

const whole = (text) => text.toWellFormed();

/** `renderContent`'s nodes for a text with no saved range: strings joined, chips by what they show. */
function parts(userMessage, content, legacy) {
  const t = (key) => key;
  const nodes = userMessage.renderContent(content, '/work', undefined, undefined, t, undefined, false, [], legacy ? undefined : [], undefined, [], false);
  const out = [];
  for (const node of nodes.flatMap((span) => span.props.children)) {
    if (typeof node === 'string') {
      if (typeof out.at(-1) === 'string') out[out.length - 1] += node;
      else out.push(node);
      continue;
    }
    const { label, tooltip, icon } = node.props;
    if (!icon) out.push({ command: label });
    else if (icon.props.name !== undefined) out.push({ file: label, path: tooltip });
    else if (icon.type.displayName === 'Sparkles') out.push({ agent: label });
    else out.push({ dir: label, path: tooltip });
  }
  return out;
}

export function sentText({ userMessage, chatQuotes, sentAgentReferenceDisplayLabel, slashCommandDisplayLabel }) {
  const { parseChatQuoteSegments } = chatQuotes;
  return {
    buildSentInlineTokens: (content, pasted, slash, references) =>
      userMessage.buildSentInlineTokens(content, pasted, slash, references).map((token) => ({
        kind: token.kind,
        text: whole(token.text),
        ...(token.kind === 'pasted' ? { display: token.display } : {}),
        ...(token.kind === 'agent-reference' ? { index: references.indexOf(token.reference) } : {}),
      })),
    projectSentInlinePlainText: (...args) => whole(userMessage.projectSentInlinePlainText(...args)),
    sentAgentReferenceDisplayLabel: (reference) => whole(sentAgentReferenceDisplayLabel(reference)),
    slashCommandDisplayLabel,
    formatQuoteForSend: chatQuotes.formatQuoteForSend,
    quoteSourceDisplayLabel: chatQuotes.quoteSourceDisplayLabel,
    parseChatQuoteSegments,
    joinChatQuoteTextSegments: (content) => chatQuotes.joinChatQuoteTextSegments(parseChatQuoteSegments(content)),
    stripChatQuoteMarkerLines: chatQuotes.stripChatQuoteMarkerLines,
    // One argument: the segments are the content's own.
    locateChatQuoteTextSegmentStarts: (content, segments) =>
      userMessage.locateChatQuoteTextSegmentStarts(content, segments ?? parseChatQuoteSegments(content)),
    projectSentRanges: (ranges, start, length) =>
      userMessage.projectSentRanges(ranges, start, length).map(({ start, end }) => ({ start, end })),
    sentTextParts: (content, legacy) => parts(userMessage, content, legacy),
  };
}
