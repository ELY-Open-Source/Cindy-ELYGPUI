//! `.msg-markdown`: the renderer tree's blocks, laid out as Blink lays out Cindy's classes.

mod inline;
mod margins;
mod marker;
mod table;
pub(super) mod text;

#[cfg(test)]
mod tests;

use ely_gpui_component::theme::ActiveTheme as _;
use gpui::{
    AnyElement, App, Div, FontWeight, Hsla, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Styled as _, TextStyle, Window, canvas, div,
};

use self::{
    margins::{Margins, Stack, collapse},
    marker::{Kind, Marker},
    text::{Inline, Mark, Metrics, Piece, metrics},
};
use super::{flow::draw, jump, target_link::Anchors};
use crate::{
    appearance_settings::{code_size, text_size},
    markdown::hast::{self, Element, Hast, Prop},
    theme::Appearance,
    units::{Zoom, css},
};

/// The text a block inherits, in CSS pixels.
#[derive(Clone)]
struct Face {
    size: f32,
    line: f32,
    weight: FontWeight,
    color: Hsla,
}

/// A block once laid out: a block with no height draws nothing, and its margins meet.
struct Laid {
    margins: Margins,
    element: Option<AnyElement>,
}

impl Laid {
    fn new((top, bottom): (f32, f32), element: Option<AnyElement>) -> Self {
        let hollow = element.is_none();
        Self {
            margins: Margins {
                top,
                bottom,
                hollow,
            },
            element,
        }
    }
}

/// The blocks in a column, each under the margin that collapsing leaves it.
fn column(laid: Vec<Laid>, open: bool) -> (Stack, Div) {
    let margins: Vec<Margins> = laid.iter().map(|laid| laid.margins).collect();
    let stack = collapse(&margins, open);
    let drawn = laid.into_iter().zip(&stack.gaps);
    let blocks = drawn.filter_map(|(laid, gap)| Some(div().mt(css((*gap)?)).child(laid.element?)));
    let column = div().flex().flex_col().children(blocks);
    (stack, column)
}

/// A container with no padding or border of its own: its children's margins pass through it.
fn open(own: (f32, f32), laid: Vec<Laid>, dress: impl FnOnce(Div) -> Div) -> Laid {
    let (stack, column) = column(laid, true);
    let margins = (own.0.max(stack.top), own.1.max(stack.bottom));
    let element = (!stack.empty).then(|| dress(column).into_any_element());
    Laid::new(margins, element)
}

/// The pending markers' items lie `by` further from the lines that follow.
fn shift(markers: &mut [Marker], by: f32) {
    markers.iter_mut().for_each(|marker| marker.x += by);
}

fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "blockquote"
            | "hr"
            | "pre"
            | "section"
            | "table"
    )
}

/// `ml-6`, `space-y-1` and `my-2`.
const LIST: (f32, f32, f32) = (24.0, 4.0, 8.0);
/// The quote's `border-l-2 pl-4`.
const QUOTE: f32 = 18.0;
/// The code block's border and `p-4`, on one side.
const CODE: f32 = 17.0;

struct Walk<'a> {
    window: &'a Window,
    cx: &'a App,
    zoom: f32,
    anchors: &'a Anchors,
}

/// `drawn` with the node's `id` kept where it paints, for an anchor to find.
fn named(node: &Element, drawn: AnyElement) -> AnyElement {
    let Some(Prop::Text(id)) = node.prop("id") else {
        return drawn;
    };
    let id = id.clone();
    let probe = canvas(
        move |bounds, _, cx| jump::painted(&id, bounds, cx),
        |_, _, _, _| {},
    );
    div()
        .relative()
        .child(drawn)
        .child(probe.absolute().inset_0())
        .into_any_element()
}

impl Walk<'_> {
    fn style(&self, face: &Face, mono: bool) -> TextStyle {
        let mut style = self.window.text_style();
        style.font_size = css(face.size).into();
        style.line_height = css(face.line).into();
        style.font_weight = face.weight;
        if mono {
            style.font_family = self.cx.theme().mono_family.clone();
        }
        style
    }

    /// A text block's rows container, which the pending markers hang from.
    fn rows(
        &self,
        (face, style, baseline): (&Face, &TextStyle, f32),
        rows: Vec<AnyElement>,
        markers: &mut Vec<Marker>,
    ) -> Div {
        let hung = markers.drain(..);
        let hung = hung.map(|marker| marker.draw(baseline, self.zoom, self.window));
        div()
            .relative()
            .flex()
            .flex_col()
            .whitespace_nowrap()
            .font(style.font())
            .text_size(css(face.size))
            .line_height(css(face.line))
            .text_color(face.color)
            .children(rows)
            .children(hung.collect::<Vec<_>>())
    }

    /// A row's own box, for a row that holds more than its text.
    fn row(&self, at: &Metrics) -> Div {
        div().flex().min_h(css(at.line / self.zoom))
    }

    fn look<'a>(
        &'a self,
        face: &'a Face,
        width: f32,
        path: &'a str,
        anywhere: bool,
    ) -> inline::Look<'a> {
        let appearance = self.cx.global::<Appearance>();
        let size = text_size(14.0, self.cx);
        inline::Look {
            path,
            face,
            code: Face {
                size,
                line: size * face.line / face.size,
                ..face.clone()
            },
            mono: self.cx.theme().mono_family.clone(),
            strong: appearance.inherited("md-strong-fg").unwrap_or(face.color),
            fill: appearance.token("msg-md-inline-code-bg"),
            width: css(width).to_pixels(self.window.rem_size()),
            zoom: self.zoom,
            anywhere,
            anchors: self.anchors,
        }
    }

    /// A run of inline nodes in parts: the text up to each handoff card, then the text after the last.
    fn parts(&self, nodes: &[Hast]) -> Vec<Inline> {
        let mut out = Vec::new();
        loop {
            let mut inline = Inline::part(out.len());
            (nodes.iter()).for_each(|node| {
                inline.read(node, Mark::default(), &|node| self.anchors.swap(node))
            });
            inline.end();
            let last = inline.cards.len() <= out.len();
            out.push(inline);
            if last {
                return out;
            }
        }
    }

    /// A run of inline nodes: its line boxes, and each handoff card a block among them.
    fn runs(
        &self,
        nodes: &[Hast],
        face: &Face,
        width: f32,
        path: &str,
        markers: &mut Vec<Marker>,
    ) -> Vec<Laid> {
        let mut out = Vec::new();
        for (part, inline) in self.parts(nodes).into_iter().enumerate() {
            // The lines after card `part - 1` are `<path>-p<part>`.
            let name = match part {
                0 => path.to_owned(),
                _ => format!("{path}-p{part}"),
            };
            let lines = (!inline.text.is_empty()).then(|| {
                let look = self.look(face, width, &name, false);
                let drawn = look.draw(&inline, self.window);
                let style = self.style(face, false);
                let block = self.rows((face, &style, drawn.baseline), drawn.rows, markers);
                let selector = name.clone();
                block.debug_selector(move || selector).into_any_element()
            });
            out.push(Laid::new((0.0, 0.0), lines));
            let Some(card) = inline.cards.get(part) else {
                continue;
            };
            // The markers that wait for a line hang by the card's first baseline.
            let metrics = self.anchors.card_metrics(*card);
            let baseline = metrics.baseline * self.zoom;
            let card = self.anchors.card(*card);
            let hung = markers.drain(..);
            let hung = hung.map(|marker| marker.draw(baseline, self.zoom, self.window));
            let card = div()
                .relative()
                .child(card)
                .children(hung.collect::<Vec<_>>());
            let margin = (metrics.margin, metrics.margin);
            out.push(Laid::new(margin, Some(card.into_any_element())));
        }
        out
    }

    /// `CodeBlockPre`'s surface over the code's text; its chrome and colors come with 9.7.
    fn pre(
        &self,
        pre: &Element,
        face: &Face,
        width: f32,
        path: &str,
        markers: &mut Vec<Marker>,
    ) -> Laid {
        let mut text = String::new();
        (pre.children.iter()).for_each(|node| hast::string(node, &mut text));
        let text = draw::laid(&text);
        // A final line feed opens no line, and an empty block holds none.
        let body = text.strip_suffix('\n').unwrap_or(&text);
        let size = code_size(0.0, self.cx);
        let code = Face {
            size,
            // `leading-normal` is Tailwind's 1.5.
            line: size * 1.5,
            ..face.clone()
        };
        let style = self.style(&code, true);
        let at = metrics(&style, self.window);
        let inner = css((width - 2.0 * CODE).max(0.0)).to_pixels(self.window.rem_size());
        let lines = body.split('\n').filter(|_| !text.is_empty());
        let broken = lines.flat_map(|line| text::code(line, &style, inner, self.window));
        let piece = |piece: Piece| match piece {
            Piece::Text(text) => text.into_any_element(),
            Piece::Tab(width) => div()
                .flex_none()
                .w(css(width / self.zoom))
                .into_any_element(),
        };
        let rows: Vec<AnyElement> = broken
            .map(|row| {
                self.row(&at)
                    .children(row.into_iter().map(piece))
                    .into_any_element()
            })
            .collect();
        let mut none = Vec::new();
        let taken = if rows.is_empty() { &mut none } else { markers };
        shift(taken, CODE);
        let rows = self.rows((&code, &style, at.baseline), rows, taken);
        let appearance = self.cx.global::<Appearance>();
        let selector = path.to_owned();
        let surface = div()
            .debug_selector(move || selector)
            .rounded(css(12.0))
            .border(css(1.0))
            .border_color(appearance.token("msg-code-block-border"))
            .bg(appearance.token("msg-code-block-bg"))
            .p(css(16.0))
            .pt(css(40.0))
            .child(rows);
        Laid::new((12.0, 12.0), Some(surface.into_any_element()))
    }

    /// `ul` and `ol`. A task list's own class replaces Cindy's, so it has no indent and no marker.
    fn list(
        &self,
        list: &Element,
        face: &Face,
        width: f32,
        path: &str,
        markers: &mut Vec<Marker>,
    ) -> Laid {
        let styled = list.prop("className").is_none();
        let (indent, between, around) = if styled { LIST } else { (0.0, 0.0, 0.0) };
        let start = match list.prop("start") {
            Some(Prop::Number(start)) => *start,
            _ => 1,
        };
        let style = self.style(face, false);
        shift(markers, indent);
        let mut items = Vec::new();
        let elements = list.children.iter().filter_map(|child| match child {
            Hast::Element(item) => Some(item),
            _ => None,
        });
        for (nth, item) in elements.enumerate() {
            let path = format!("{path}-{nth}");
            if styled {
                markers.push(Marker {
                    kind: match list.tag {
                        "ol" => Kind::Number(start + nth as u32),
                        _ => Kind::Disc,
                    },
                    style: style.clone(),
                    color: face.color,
                    x: 0.0,
                    selector: format!("{path}-marker"),
                });
            }
            let pending = markers.len();
            let laid = self.flow(&item.children, face, width - indent, &path, markers);
            let (stack, column) = column(laid, true);
            let top = if nth == 0 { 0.0 } else { between };
            let margins = (top.max(stack.top), stack.bottom);
            // An item with no line keeps its marker; with no block it is a line others take too.
            let lone = styled && markers.len() == pending;
            let blocks = |child: &Hast| matches!(child, Hast::Element(e) if is_block(e.tag));
            let kept = match item.children.iter().any(blocks) {
                _ if !lone => 0,
                true => 1,
                false => markers.len(),
            };
            let column = column.debug_selector(move || path);
            let element = match kept {
                0 => (!stack.empty).then(|| column.into_any_element()),
                _ => {
                    let at = metrics(&style, self.window);
                    let hung = markers.drain(markers.len() - kept..);
                    let hung = hung.map(|marker| marker.draw(at.baseline, self.zoom, self.window));
                    let item = column.relative().min_h(css(at.line / self.zoom));
                    Some(item.children(hung.collect::<Vec<_>>()).into_any_element())
                }
            };
            let element = element.map(|element| named(item, element));
            items.push(Laid::new(margins, element));
        }
        shift(markers, -indent);
        open((around, around), items, |column| column.ml(css(indent)))
    }

    fn quote(
        &self,
        quote: &Element,
        face: &Face,
        width: f32,
        path: &str,
        markers: &mut Vec<Marker>,
    ) -> Laid {
        let appearance = self.cx.global::<Appearance>();
        let face = Face {
            color: appearance.token("msg-blockquote-text"),
            ..face.clone()
        };
        shift(markers, QUOTE);
        let laid = self.flow(&quote.children, &face, width - QUOTE, path, markers);
        shift(markers, -QUOTE);
        let border = appearance.token("msg-blockquote-border");
        let selector = path.to_owned();
        open((8.0, 8.0), laid, |column| {
            column
                .debug_selector(move || selector)
                .border_l(css(2.0))
                .border_color(border)
                .pl(css(16.0))
        })
    }

    /// A heading's size, line and margin; `h4` to `h6` keep the text's own.
    fn heading(&self, level: usize, face: &Face) -> (Face, f32) {
        let (size, ratio, margin) = match level {
            1 => (20.0, 1.4, 12.0),
            2 => (18.0, 1.556, 12.0),
            3 => (16.0, 1.5, 8.0),
            _ => (15.0, 1.6, 0.0),
        };
        let size = text_size(size, self.cx);
        let appearance = self.cx.global::<Appearance>();
        let color = appearance.inherited(&format!("md-h{level}-fg"));
        let face = Face {
            size,
            line: size * ratio,
            weight: match level {
                1..=3 => FontWeight::MEDIUM,
                _ => face.weight,
            },
            color: color.unwrap_or(face.color),
        };
        (face, margin)
    }

    /// One block element; `edges` is whether it is its parent's first and last element.
    fn block(
        &self,
        block: &Element,
        edges: (bool, bool),
        face: &Face,
        (width, path): (f32, &str),
        markers: &mut Vec<Marker>,
    ) -> Option<Laid> {
        let laid = match block.tag {
            // `my-2 first:mt-0 last:mb-0`.
            "p" => {
                let margin = |edge| if edge { 0.0 } else { 8.0 };
                let runs = self.runs(&block.children, face, width, path, markers);
                open((margin(edges.0), margin(edges.1)), runs, |column| column)
            }
            "ul" | "ol" => self.list(block, face, width, path, markers),
            "blockquote" => self.quote(block, face, width, path, markers),
            "pre" => self.pre(block, face, width, path, markers),
            // `my-4`, and the top border that is all of its height.
            "hr" => {
                let color = self.cx.global::<Appearance>().token("msg-hr-border");
                let selector = path.to_owned();
                let rule = div().debug_selector(move || selector).h(css(1.0)).bg(color);
                Laid::new((16.0, 16.0), Some(rule.into_any_element()))
            }
            // The footnotes.
            "section" => {
                let laid = self.flow(&block.children, face, width, path, markers);
                open((0.0, 0.0), laid, |column| column)
            }
            "table" => self.table(block, face, (width, path), markers),
            // The footnotes' heading: the node's own class replaces Cindy's.
            _ if block.classed("sr-only") => return None,
            tag => {
                let level = tag[1..].parse().expect("a heading's level");
                let (face, margin) = self.heading(level, face);
                let runs = self.runs(&block.children, &face, width, path, markers);
                let mut laid = open((margin, margin), runs, |column| column);
                laid.element = laid.element.map(|lines| named(block, lines));
                laid
            }
        };
        Some(laid)
    }

    /// A container's children: its block elements, and a block about each run of inline nodes.
    fn flow(
        &self,
        children: &[Hast],
        face: &Face,
        width: f32,
        path: &str,
        markers: &mut Vec<Marker>,
    ) -> Vec<Laid> {
        let element = |node: &Hast| matches!(node, Hast::Element(_));
        let first = children.iter().position(element);
        let last = children.iter().rposition(element);
        let (mut out, mut run, mut nth) = (Vec::new(), 0, 0);
        for (index, child) in children.iter().enumerate() {
            let Hast::Element(block) = child else {
                continue;
            };
            if !is_block(block.tag) {
                continue;
            }
            // The text before block `nth` is `<path>-<nth>t`.
            let text = format!("{path}-{nth}t");
            out.extend(self.runs(&children[run..index], face, width, &text, markers));
            let edges = (first == Some(index), last == Some(index));
            let own = format!("{path}-{nth}");
            out.extend(self.block(block, edges, face, (width, &own), markers));
            (run, nth) = (index + 1, nth + 1);
        }
        let text = format!("{path}-{nth}t");
        out.extend(self.runs(&children[run..], face, width, &text, markers));
        out
    }
}

/// The tree's blocks `width` CSS pixels wide; `key` names them for tests.
pub(super) fn element(
    key: &str,
    tree: &[Hast],
    anchors: &Anchors,
    width: f32,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let walk = Walk {
        window,
        cx,
        zoom: cx.global::<Zoom>().0,
        anchors,
    };
    let size = text_size(15.0, cx);
    let face = Face {
        size,
        line: size * 1.6,
        weight: FontWeight::NORMAL,
        color: cx.global::<Appearance>().token("msg-assistant-text"),
    };
    let laid = walk.flow(tree, &face, width, &format!("{key}-md"), &mut Vec::new());
    // The bubble is a flex item, so the first and last margins stay inside it.
    let (stack, column) = column(laid, false);
    let selector = format!("{key}-markdown");
    column
        .debug_selector(move || selector)
        .w_full()
        .pb(css(stack.tail))
        .into_any_element()
}
