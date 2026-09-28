//! Text metrics: register the backend's font metrics as the layout measurer.

use std::rc::Rc;

use draw_backend_wgpu::FontMetrics;
use draw_core::FontWeight;
use draw_ui::TextMeasurer;
use quill_app::{App, AppBuilder, LifecycleObserver, Plugin};

use crate::wgpu::SharedBackend;

/// Adapts the wgpu backend's font metrics to the layout engine.
pub struct BackendTextMeasurer {
    metrics: FontMetrics,
}

impl BackendTextMeasurer {
    pub fn new(metrics: FontMetrics) -> Self {
        Self { metrics }
    }
}

impl TextMeasurer for BackendTextMeasurer {
    fn advance(&self, ch: char, font_size: f32) -> f32 {
        self.metrics.advance(ch, font_size)
    }

    fn advance_weighted(&self, ch: char, font_size: f32, weight: FontWeight) -> f32 {
        self.metrics.advance_weighted(ch, font_size, weight)
    }

    fn line_height(&self, font_size: f32) -> f32 {
        self.metrics.line_height(font_size)
    }

    fn ascent(&self, font_size: f32) -> f32 {
        self.metrics.ascent(font_size)
    }

    fn measure_run(&self, text: &str, font_size: f32) -> f32 {
        self.metrics.measure_run(text, font_size)
    }

    fn measure_run_weighted(&self, text: &str, font_size: f32, weight: FontWeight) -> f32 {
        self.metrics.measure_run_weighted(text, font_size, weight)
    }
}

/// Registers the backend's real font metrics as the [`TextMeasurer`] service, so
/// layout measures the font that is actually painted.
///
/// Requires [`WgpuPlugin`](crate::WgpuPlugin), registered first.
#[derive(Default)]
pub struct TextMeasurePlugin;

impl Plugin for TextMeasurePlugin {
    fn name(&self) -> &'static str {
        "text-measure"
    }

    fn build(&self, app: &mut AppBuilder) {
        app.add_lifecycle_observer(TextMeasureLifecycle);
    }
}

struct TextMeasureLifecycle;

impl LifecycleObserver for TextMeasureLifecycle {
    fn resumed(&mut self, app: &mut App) {
        if app.services().get::<Rc<dyn TextMeasurer>>().is_some() {
            return;
        }
        let Some(backend) = app.services().get::<SharedBackend>().cloned() else {
            return;
        };
        let metrics = backend.borrow().text_metrics();
        let measurer: Rc<dyn TextMeasurer> = Rc::new(BackendTextMeasurer::new(metrics));
        app.services_mut().insert(measurer);
    }
}
