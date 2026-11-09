//! `pinSearch`: the search row sticky, the content after it clipped below it.

use gpui::{
    AnyElement, App, Bounds, ContentMask, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, ScrollHandle, Window, point, px, size,
};

use super::Search;

/// The row: held at the scroller's top while `on`.
pub fn row(child: AnyElement, handle: ScrollHandle, on: bool) -> Stick {
    Stick {
        child,
        mode: Mode::Row { handle, on },
    }
}

/// Content after the row: clipped below the row while it is pinned.
pub fn after(child: AnyElement) -> Stick {
    Stick {
        child,
        mode: Mode::After,
    }
}

pub struct Stick {
    child: AnyElement,
    mode: Mode,
}

enum Mode {
    Row { handle: ScrollHandle, on: bool },
    After,
}

impl IntoElement for Stick {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Stick {
    /// The clip for content after a pinned row, this frame.
    fn mask(&self, cx: &App) -> Option<ContentMask<Pixels>> {
        let Mode::After = self.mode else {
            return None;
        };
        let bottom = cx.global::<Search>().pinned_bottom?;
        let far = px(1.0e6);
        Some(ContentMask {
            bounds: Bounds::new(point(-far, bottom), size(far * 2.0, far * 2.0)),
        })
    }
}

impl Element for Stick {
    type RequestLayoutState = ();
    type PrepaintState = Option<ContentMask<Pixels>>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<ContentMask<Pixels>> {
        let Mode::Row { handle, on } = &self.mode else {
            let mask = self.mask(cx);
            window.with_content_mask(mask, |window| self.child.prepaint(window, cx));
            return mask;
        };
        let held = match on {
            true => (handle.bounds().top() - bounds.top()).max(px(0.0)),
            false => px(0.0),
        };
        cx.global_mut::<Search>().pinned_bottom = on.then(|| bounds.bottom() + held);
        window.with_element_offset(point(px(0.0), held), |window| {
            self.child.prepaint(window, cx)
        });
        None
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        mask: &mut Option<ContentMask<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(*mask, |window| self.child.paint(window, cx));
    }
}

#[cfg(test)]
mod tests;
