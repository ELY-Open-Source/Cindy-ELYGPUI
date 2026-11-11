# Markdown tree

The parse behind `src/markdown.rs`, moved from `AGENTS.md`.

`src/markdown.rs` is the parse of `MarkdownRenderer.tsx`'s `REMARK_PLUGINS`: remark-gfm with
`singleTilde: false`, remark-cjk-friendly and remark-math, golden `markdown-tree`
(`scripts/golden/markdownTree.mjs` runs remark-parse with the list's first three plugins; its cases
hold CommonMark 0.31.2's examples and seeded mixes of tokens). NUL reads as U+FFFD. `flanks.rs` is
micromark-extension-cjk-friendly's `tokenizeAttention` on the classes of `resources/attention-
classes.json`, which the sync reads off the extension's own `classifyCharacter`; the crate asks it
through `ParseOptions.attention_flanks` with the slice its text tokenizer was fed, micromark's chunk
stream: past a slice's end lies whitespace, and a variation selector reads the character before it
only inside its chunk, which a NUL ends. `autolinks.rs` is mdast-util-gfm-autolink-literal's
`transformGfmAutolinkLiterals` with mdast-util-find-and-replace's walk: web addresses and emails
left in text become links, outside links and link references. The golden leaves out the `checked`
remark sets on a node that is no list item, which nothing reads. The crate's patches for this tree,
each found by the golden: `previousUnbalanced` for literal autolinks (`label_spans`: a label start
holds them back until a failed end balances it), an email tried before `www`, no `mailto:` or
`xmpp:` prefix, a code or math span's padding of spaces or line endings, a chunk that starts inside
a tab keeps the tab's spaces, a list prefix counted in columns, a label read from its source
(`decode_string`, micromark's `decodeString`), an identifier's first word, an image's alt with the
alts inside it, a table head after a line a flow shortcut refused, and the table head's flag reset
when it is refused. Containers as micromark's document closes them: `interrupt` holds while the flow
has a construct open and through the containers of one line, a list is spread by two line endings
between items (a quote that closed there ends before its last line's ending), an item by a blank
line before its last that is its own, complete html on a lazy line interrupts a paragraph and stays
in its containers, the empty line at end of file is lazy to open quotes alone, and no line continues
a setext heading lazily. An underline over definitions alone is refused when it is read
(`paragraph_before` reads the content's definitions then), and none follows an underline. Attention
and strikethrough resolve apart, as micromark's two resolvers do: at the top the kind the text met
first goes first, in a label attention does (`insideSpan`'s order), and a matched pair resolves what
it holds, then lets it go. An opener that ends where a micromark chunk ends (before a NUL,
`ParseOptions.chunk_ends`) keeps its whole run as text once part of it is used, as `movePoint`
leaves it. An email is micromark's, read before attention: its domain is read off the source, it
keeps an underscore past its first byte while no label start stands unbalanced, an escape takes its
first byte only when that is punctuation, and none starts in the domain of the email before it or
right after a link's atext. A task check opens the first content of the flow an item starts, so a
lazy line after an empty item loses its `[x]` as Cindy's does, a footnote definition that closed
before it is passed, and content after its line's spaces is not first; after a definition a line's
indent stays outside the paragraph. The check drops one unit after it, so a CRLF keeps its LF.
Spread (`util/infer.rs`, `spreads`) replays micromark on the port's events: each container's exit
goes where micromark leaves it (where a new container starts, else where the line that closes it
starts, else the document's end; a blank line closes no item), `subtokenize` moves it before the
line endings it follows until a quote's prefix or a definition's indent stops it, then `prepareList`
runs over each list; `to_mdast` reads the table. A definition inside another continues when the
outer one took its indent and no space follows. An empty item a blank line closed leaves the next
filled line lazy, so indented code there ends at its line and holds no `interrupt`; a container ends
indented code where its line starts, not at the ending of the line it opened; the setext check reads
no content line from before a container that opened. A fence's indent counts in columns, a blank
line in a definition leaves its spaces to the flow, and the flow a new container closes is lazy to
nothing, so unclosed html keeps its line ending.
