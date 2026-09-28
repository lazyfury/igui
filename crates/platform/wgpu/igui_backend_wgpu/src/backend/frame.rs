//! Frame lifecycle: the offscreen target, pipeline cache and render pass.

use super::*;
use pipeline::create_render_pipeline;

impl WgpuBackend {
    /// Begins a frame that renders into an external view, such as the texture
    /// view of a window surface.
    ///
    /// `width`/`height` are in device pixels and `format` must match the view's
    /// texture format. The view is cloned internally, so the caller keeps
    /// ownership of the surface texture and presents it after
    /// [`RenderBackend::end_frame`].
    pub fn begin_frame_with_view(
        &mut self,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        viewport: ViewportSize,
    ) -> Result<(), WgpuError> {
        let width = width.max(1);
        let height = height.max(1);
        self.ensure_pipeline(format);
        self.ensure_msaa(width, height, format);
        self.start_frame(viewport, width, height)?;
        let msaa_view = self.msaa.as_ref().map(|target| target.view.clone());
        self.frame = Some(Frame {
            view,
            msaa_view,
            width,
            height,
            format,
            offscreen: false,
        });
        Ok(())
    }

    /// Returns `true` when the current frame targets the offscreen texture, so
    /// [`WgpuBackend::read_pixels`] will reflect this frame.
    pub fn is_offscreen_frame(&self) -> bool {
        self.frame.as_ref().is_some_and(|frame| frame.offscreen)
    }

    /// Ensures a render pipeline exists for `format`, creating it on demand.
    pub(super) fn ensure_pipeline(&mut self, format: wgpu::TextureFormat) {
        if self.pipelines.contains_key(&format) {
            return;
        }
        let pipeline = create_render_pipeline(
            &self.device,
            &self.shader,
            &self.bind_group_layout,
            format,
            "fs_main",
            self.msaa_samples,
        );
        self.pipelines.insert(format, pipeline);
    }

    /// Resets per-frame staging and validates the lifecycle.
    pub(super) fn start_frame(
        &mut self,
        viewport: ViewportSize,
        width: u32,
        height: u32,
    ) -> Result<(), WgpuError> {
        if self.in_frame {
            return Err(WgpuError::AlreadyInFrame);
        }
        self.in_frame = true;
        self.viewport = viewport;
        self.device_width = width as f32;
        self.device_height = height as f32;
        self.state = State::default();
        self.stack.clear();
        // `vertices` / `ranges` are deliberately kept: a frame whose single
        // submitted list matches the cached one reuses them (see `submit`).
        self.submitted = 0;
        self.reused = false;
        Ok(())
    }

    pub(super) fn ensure_offscreen(&mut self, width: u32, height: u32) {
        if matches!(&self.offscreen, Some(target) if target.width == width && target.height == height)
        {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("igui_backend_wgpu.offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.offscreen = Some(OffscreenTarget {
            width,
            height,
            texture,
            view,
        });
    }

    /// Ensures a multisampled colour target exists for `width`x`height`/`format`.
    ///
    /// When [`WgpuBackend::set_msaa_samples`] is `1`, MSAA is disabled and no
    /// target is kept: frames render single-sampled straight to their view.
    pub(super) fn ensure_msaa(&mut self, width: u32, height: u32, format: wgpu::TextureFormat) {
        let samples = self.msaa_samples;
        if samples <= 1 {
            self.msaa = None;
            return;
        }
        if matches!(
            &self.msaa,
            Some(target)
                if target.width == width
                    && target.height == height
                    && target.format == format
                    && target.samples == samples
        ) {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("igui_backend_wgpu.msaa"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.msaa = Some(MsaaTarget {
            width,
            height,
            format,
            samples,
            view,
        });
    }

    /// Encodes and submits the render pass for the current frame.
    pub(super) fn render_frame(&mut self) {
        let Some((format, width, height, msaa_view, view)) = self.frame.as_ref().map(|frame| {
            (
                frame.format,
                frame.width,
                frame.height,
                frame.msaa_view.clone(),
                frame.view.clone(),
            )
        }) else {
            return;
        };

        // Build any effect pipeline the registered textures need for this
        // target format, before the pass borrows the caches immutably.
        let mut effects: Vec<TextureEffect> = self.texture_effects.values().copied().collect();
        effects.sort_by_key(|effect| *effect as u8);
        effects.dedup();
        for effect in effects {
            self.ensure_effect_pipeline(format, effect);
        }

        // Stage the vertices before borrowing the pipeline cache immutably.
        let vertex_buffer = self.stage_vertices();

        let Some(default_pipeline) = self.pipelines.get(&format) else {
            return;
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("igui_backend_wgpu.encoder"),
            });

        let clear = wgpu::Color {
            r: self.clear.r as f64,
            g: self.clear.g as f64,
            b: self.clear.b as f64,
            a: self.clear.a as f64,
        };

        {
            // With MSAA the multisampled texture is the attachment and resolves
            // into the frame view; without it the frame view is drawn directly
            // and must be stored.
            let (attachment, resolve_target, store) = match msaa_view.as_ref() {
                Some(target) => (target, Some(&view), wgpu::StoreOp::Discard),
                None => (&view, None, wgpu::StoreOp::Store),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("igui_backend_wgpu.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: attachment,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(buffer) = &vertex_buffer {
                pass.set_vertex_buffer(0, buffer.slice(..));
                let mut current: Option<*const wgpu::RenderPipeline> = None;
                for range in &self.ranges {
                    // A texture with an effect is drawn through that effect's
                    // pipeline; everything else uses the plain one.
                    let effect = match range.surface {
                        Surface::Texture(id) => self.effect_for(id),
                        _ => TextureEffect::None,
                    };
                    let pipeline = if effect == TextureEffect::None {
                        default_pipeline
                    } else {
                        self.effect_pipelines
                            .get(&(effect, format))
                            .unwrap_or(default_pipeline)
                    };
                    let key = pipeline as *const wgpu::RenderPipeline;
                    if current != Some(key) {
                        pass.set_pipeline(pipeline);
                        current = Some(key);
                    }
                    pass.set_bind_group(0, self.bind_group_for(range.surface), &[]);
                    match range.scissor {
                        Some(scissor) => {
                            pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3])
                        }
                        None => pass.set_scissor_rect(0, 0, width, height),
                    }
                    pass.draw(range.vertices.clone(), 0..1);
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
    }
}
