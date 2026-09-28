//! Chrome: surface / foreground layers wrapped around a control's content.
//!
//! This is the single way to draw something other than a control's own
//! [`ControlContent`](crate::ControlContent): a themed surface behind it, or a
//! foreground (a check mark, a dimming film) in front. It replaces the old
//! `NodeDecor` / `DecorRef` layer, so a control stores **one** self-draw value.

use igui_core::{Color, NodeId, Rect, Size};

use crate::content::{ContentRef, ControlContent, PaintEnv};
use crate::layout::{ContentSize, TextMeasurer};
use crate::paint::SurfaceStyle;
use crate::state::InteractState;

type BehindFn = Box<dyn Fn(&mut PaintEnv<'_>, Rect, InteractState)>;
type FrontFn = Box<dyn Fn(&mut PaintEnv<'_>, Rect, InteractState)>;

/// Wraps an optional inner content with a surface behind and a foreground in
/// front of it.
///
/// The layers are painted in order: surface, inner content, foreground. With no
/// inner content it is chrome alone (a panel), and with no chrome it is a plain
/// passthrough.
pub struct Chrome {
    behind: Option<BehindFn>,
    inner: Option<ContentRef>,
    front: Option<FrontFn>,
}

impl Chrome {
    /// Wraps `inner` (which may be `None`).
    pub fn new(inner: Option<ContentRef>) -> Self {
        Self {
            behind: None,
            inner,
            front: None,
        }
    }

    /// Paints a themed surface behind the content. The style is resolved from
    /// the interaction state each frame, so hover / press react without a
    /// second code path.
    pub fn background(mut self, resolve: impl Fn(InteractState) -> SurfaceStyle + 'static) -> Self {
        self.behind = Some(Box::new(move |env, rect, state| {
            env.surface(rect, &resolve(state));
        }));
        self
    }

    /// Paints a foreground in front of the content.
    pub fn foreground(
        mut self,
        draw: impl Fn(&mut PaintEnv<'_>, Rect, InteractState) + 'static,
    ) -> Self {
        self.front = Some(Box::new(draw));
        self
    }

    /// Whether there is nothing to draw (no chrome and no inner content).
    pub fn is_empty(&self) -> bool {
        self.behind.is_none() && self.inner.is_none() && self.front.is_none()
    }
}

impl ControlContent for Chrome {
    fn measure(&self, measurer: &dyn TextMeasurer, available: Size) -> ContentSize {
        self.inner.as_ref().map_or(ContentSize::ZERO, |content| {
            content.measure(measurer, available)
        })
    }

    fn paint_behind(&self, env: &mut PaintEnv<'_>, rect: Rect, state: InteractState) {
        if let Some(behind) = &self.behind {
            behind(env, rect, state);
        }
        if let Some(inner) = &self.inner {
            inner.paint_behind(env, rect, state);
        }
    }

    fn draw(&self, id: NodeId, env: &mut PaintEnv<'_>, rect: Rect, state: InteractState) {
        if let Some(inner) = &self.inner {
            inner.draw(id, env, rect, state);
        }
    }

    fn paint_front(&self, env: &mut PaintEnv<'_>, rect: Rect, state: InteractState) {
        if let Some(inner) = &self.inner {
            inner.paint_front(env, rect, state);
        }
        if let Some(front) = &self.front {
            front(env, rect, state);
        }
    }

    fn as_text(&self) -> Option<&str> {
        self.inner.as_ref().and_then(|content| content.as_text())
    }

    fn as_text_size(&self) -> Option<f32> {
        self.inner
            .as_ref()
            .and_then(|content| content.as_text_size())
    }

    fn set_text(&mut self, text: &str) -> bool {
        self.inner
            .as_mut()
            .is_some_and(|content| content.set_text(text))
    }

    fn set_color(&mut self, color: Color) -> bool {
        self.inner
            .as_mut()
            .is_some_and(|content| content.set_color(color))
    }
}
