//! Emitting control visuals and debug bounds into a `DrawList`.

use super::*;
use crate::content::PaintEnv;
use crate::control::{control_of, control_visible};
use crate::debug::DebugDrawOptions;
use igui_core::{Rect, Transform2D, Vec2};
use igui_render::{PaintContext, TextAlign};
use igui_scene::SceneTree;

impl Ui {
    /// Emits the root viewport's control visuals into `ctx` in draw order.
    pub fn paint(&self, tree: &SceneTree, ctx: &mut PaintContext) {
        self.paint_viewport(tree, tree.root(), ctx);
    }

    /// Emits one viewport's control visuals into `ctx` in draw order — used for
    /// a non-root `Viewport` (`SubViewport`) rendered into its own target.
    pub fn paint_viewport(&self, tree: &SceneTree, viewport: NodeId, ctx: &mut PaintContext) {
        // Hold the root's text cache and measurer for the whole pass (created
        // empty / default if the tree has never been laid out).
        let root = crate::control::root_state(tree);
        let mut fallback = crate::control::LayoutCache::default();
        let mut cache_ref = root.map(|state| state.layout.borrow_mut());
        let cache: &mut crate::control::LayoutCache =
            cache_ref.as_deref_mut().unwrap_or(&mut fallback);
        let measurer_guard = root.map(|state| state.text_measurer.borrow());
        let measurer: &dyn TextMeasurer = match measurer_guard.as_ref() {
            Some(handle) => handle.as_ref(),
            None => &crate::control::DEFAULT_MEASURER,
        };
        // Every control draws under the clip and canvas transform its layout
        // resolved. Controls sharing both share one `Save`/`set_transform`/
        // `ClipRect` region, so an untransformed, unclipped UI costs no extra
        // commands (the common case stays byte-identical to before).
        let mut active: Option<(Transform2D, Option<Rect>)> = None;
        for item in tree.paint_items_in(viewport) {
            let id = item.id;
            let Some(control) = control_of(tree, id) else {
                // A world canvas item (no `Control` runtime). End any control
                // clip, then draw its built-in `Visual` under its transform.
                if active.is_some() {
                    ctx.restore();
                    active = None;
                }
                ctx.save();
                ctx.set_transform(item.transform);
                if let Some(visual) = tree.visual(id) {
                    igui_scene::paint_visual(ctx, &visual);
                }
                ctx.restore();
                continue;
            };
            // A node hidden at runtime (e.g. a router switch) is skipped even if
            // `SceneTree::update` has not run since it was hidden.
            if !control_visible(tree, id) {
                continue;
            }
            let clip = control.data.clip_rect;
            // Clipped away entirely: the intersection of its ancestors' clips
            // and its own rectangle is empty, so none of it can be seen.
            if clip.is_some_and(Rect::is_empty) {
                continue;
            }
            // A `Control` is a canvas item like a `Node2D`: it draws under
            // `canvas_transform * world_transform` (`item.transform`), so a
            // control parented to a `Node2D` follows the world (H2).
            let transform = item.transform;
            let region = (transform, clip);
            if active != Some(region) {
                if active.is_some() {
                    ctx.restore();
                    active = None;
                }
                if clip.is_some() || transform != Transform2D::IDENTITY {
                    ctx.save();
                    if transform != Transform2D::IDENTITY {
                        ctx.set_transform(transform);
                    }
                    if let Some(rect) = clip {
                        ctx.clip_rect(rect);
                    }
                    active = Some(region);
                }
            }
            let rect = control.data.rect;
            let state = self.state_for(tree, id);
            if let Some(content) = &control.content {
                let mut env = PaintEnv::new(ctx, measurer, cache);
                content.paint_behind(&mut env, rect, state);
                content.draw(id, &mut env, rect, state);
                content.paint_front(&mut env, rect, state);
            }
        }
        if active.is_some() {
            ctx.restore();
        }
    }

    /// Draws debug bounds plus `name#id` labels for every visible control.
    ///
    /// Emits ordinary backend-neutral commands, so any backend renders it.
    /// Typical use: paint the UI first, then call this so the yellow boxes sit
    /// on top of the components.
    pub fn paint_debug(
        &self,
        tree: &SceneTree,
        ctx: &mut PaintContext,
        options: &DebugDrawOptions,
    ) {
        for id in tree.iter_visible() {
            let Some(control) = control_of(tree, id) else {
                continue;
            };
            let rect = control.data.rect;
            ctx.stroke_rect(rect, options.width, options.border_color);

            let name = tree.get(id).map_or("", |node| node.name());
            let label = options.label(name, id);
            if label.is_empty() {
                continue;
            }
            let position = Vec2::new(
                rect.left() + options.label_offset.x,
                rect.top() + options.label_offset.y + options.font_size,
            );
            ctx.draw_text(
                label,
                position,
                options.font_size,
                TextAlign::Left,
                options.text_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::Control;
    use crate::layout::TextOptions;
    use crate::test_support::TestControl;
    use igui_core::{Color, Edges, FontWeight, Size, ViewportSize};
    use igui_render::DrawCommand;
    use igui_scene::SceneTree;

    fn panel() -> TestControl {
        TestControl::Panel {
            color: Color::RED,
            border: None,
        }
    }

    fn label(text: &str) -> TestControl {
        TestControl::Label {
            text: text.to_string(),
            font_size: 12.0,
            color: Color::WHITE,
            options: TextOptions::default(),
        }
    }

    fn add(
        tree: &mut SceneTree,
        parent: NodeId,
        mut data: ControlData,
        widget: TestControl,
    ) -> NodeId {
        data.anchors = Edges::ZERO;
        let (container, content) = widget.into_parts();
        let id = tree.add_control(parent, "test");
        tree.set_data(id, Control::new(data, container, content));
        id
    }

    fn rect(left: f32, top: f32, right: f32, bottom: f32) -> ControlData {
        ControlData {
            offsets: Edges::new(left, top, right, bottom),
            ..ControlData::default()
        }
    }

    fn paint(tree: &SceneTree) -> igui_render::DrawList {
        let mut ctx = PaintContext::new();
        crate::paint(tree, &mut ctx);
        ctx.into_draw_list()
    }

    fn clips(list: &igui_render::DrawList) -> Vec<Rect> {
        list.commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::ClipRect(rect) => Some(*rect),
                _ => None,
            })
            .collect()
    }

    /// Backward compatibility: a tree that never asks to clip is painted
    /// exactly as before — not a single extra command.
    #[test]
    fn a_tree_without_clips_emits_no_clip_commands() {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let container = add(&mut tree, root, ControlData::fill_parent(), panel());
        add(
            &mut tree,
            container,
            rect(0.0, 0.0, 100.0, 20.0),
            label("a"),
        );
        add(
            &mut tree,
            container,
            rect(0.0, 20.0, 100.0, 40.0),
            label("b"),
        );

        crate::layout(&mut tree, ViewportSize::new(Size::new(200.0, 200.0)));
        let list = paint(&tree);

        assert!(clips(&list).is_empty());
        assert!(!list
            .commands()
            .iter()
            .any(|command| matches!(command, DrawCommand::Save | DrawCommand::Restore)));
    }

    /// A clipped region costs one pair of commands no matter how much is inside
    /// it, and the clip is the region's own rectangle.
    #[test]
    fn a_clipped_region_pushes_one_clip_before_its_content() {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let container = add(&mut tree, root, ControlData::fill_parent(), panel());
        let mut clipper = rect(10.0, 10.0, 110.0, 60.0);
        clipper.clip = true;
        let clipper = add(&mut tree, container, clipper, panel());
        for index in 0..3 {
            let top = index as f32 * 20.0;
            add(
                &mut tree,
                clipper,
                rect(0.0, top, 100.0, top + 20.0),
                label("row"),
            );
        }

        crate::layout(&mut tree, ViewportSize::new(Size::new(200.0, 200.0)));
        let list = paint(&tree);

        let region = Rect::from_min_size(Vec2::new(10.0, 10.0), Size::new(100.0, 50.0));
        assert_eq!(clips(&list), vec![region]);
        assert_eq!(
            list.commands()
                .iter()
                .filter(|command| matches!(command, DrawCommand::Save))
                .count(),
            1
        );
        assert_eq!(
            list.commands()
                .iter()
                .filter(|command| matches!(command, DrawCommand::Restore))
                .count(),
            1
        );

        let push = list
            .commands()
            .iter()
            .position(|command| matches!(command, DrawCommand::ClipRect(_)))
            .unwrap();
        let first_row = list
            .commands()
            .iter()
            .position(|command| matches!(command, DrawCommand::DrawText { .. }))
            .unwrap();
        assert!(push < first_row, "the clip is pushed before the rows");
        assert!(matches!(list.commands().last(), Some(DrawCommand::Restore)));
    }

    /// Intersecting an inherited clip with a clipper that misses it leaves
    /// nothing to draw, so the whole subtree is dropped instead of being sent
    /// to the backend to be scissored away.
    #[test]
    fn a_subtree_clipped_away_emits_nothing() {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let container = add(&mut tree, root, ControlData::fill_parent(), panel());
        let mut outer = rect(0.0, 0.0, 100.0, 100.0);
        outer.clip = true;
        let outer = add(&mut tree, container, outer, panel());

        let mut far = rect(500.0, 500.0, 600.0, 600.0);
        far.clip = true;
        let far = add(&mut tree, outer, far, panel());
        add(&mut tree, far, rect(0.0, 0.0, 50.0, 20.0), label("hidden"));

        crate::layout(&mut tree, ViewportSize::new(Size::new(800.0, 800.0)));
        let list = paint(&tree);

        assert!(
            !list.commands().iter().any(|command| matches!(
                command,
                DrawCommand::DrawText { text, .. } if text == "hidden"
            )),
            "nothing of the clipped-away subtree is emitted"
        );
        assert_eq!(
            clips(&list),
            vec![Rect::from_min_size(Vec2::ZERO, Size::new(100.0, 100.0))],
            "only the real clip reaches the backend; the empty intersection \
             never does"
        );
    }

    /// A control that sits outside the clip is still painted — the backend
    /// scissor is what removes it — but the point is that it is tagged with the
    /// clip, not with nothing.
    #[test]
    fn a_control_outside_the_clip_still_draws_under_it() {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let container = add(&mut tree, root, ControlData::fill_parent(), panel());
        let mut clipper = rect(0.0, 0.0, 100.0, 100.0);
        clipper.clip = true;
        let clipper = add(&mut tree, container, clipper, panel());
        let sticking_out = add(
            &mut tree,
            clipper,
            rect(0.0, 0.0, 300.0, 20.0),
            label("overhang"),
        );

        crate::layout(&mut tree, ViewportSize::new(Size::new(400.0, 400.0)));
        let list = paint(&tree);

        assert_eq!(
            crate::control(&tree, sticking_out).unwrap().clip_rect,
            Some(Rect::from_min_size(Vec2::ZERO, Size::new(100.0, 100.0)))
        );
        assert!(list.commands().iter().any(|command| matches!(
            command,
            DrawCommand::DrawText { text, .. } if text == "overhang"
        )));
    }

    /// The weight on a label's `TextOptions` reaches the backend: a bold label
    /// paints `DrawText { weight: Bold }`.
    #[test]
    fn a_bold_label_paints_a_bold_text_command() {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let container = add(&mut tree, root, ControlData::fill_parent(), panel());
        let mut widget = label("bold");
        if let TestControl::Label { options, .. } = &mut widget {
            *options = options.weight(FontWeight::BOLD);
        }
        add(&mut tree, container, rect(0.0, 0.0, 100.0, 20.0), widget);

        crate::layout(&mut tree, ViewportSize::new(Size::new(200.0, 200.0)));
        let list = paint(&tree);

        let weight = list.commands().iter().find_map(|command| match command {
            DrawCommand::DrawText { text, weight, .. } if text == "bold" => Some(*weight),
            _ => None,
        });
        assert_eq!(weight, Some(FontWeight::BOLD));
    }

    /// One `igui_ui::paint` pass draws a world `Node2D` and a `Control`
    /// together, in tree order, applying the world transform to the world item
    /// only — the unified canvas-item walk (kills the old two-walk split).
    #[test]
    fn one_pass_paints_world_visuals_and_controls() {
        use igui_core::Vec2;
        use igui_scene::Visual;

        let mut tree = SceneTree::new();
        tree.set_viewport_size(Size::new(200.0, 200.0));
        let root = tree.root();
        let world = tree.add_node2d(root, "world");
        tree.set_position(world, Vec2::new(10.0, 10.0));
        tree.set_visual(
            world,
            Visual::Rect {
                size: Size::new(20.0, 20.0),
                color: Color::RED,
            },
        );
        add(&mut tree, root, rect(0.0, 0.0, 50.0, 50.0), panel());
        crate::layout(&mut tree, ViewportSize::new(Size::new(200.0, 200.0)));
        tree.update();

        let list = paint(&tree);
        let commands = list.commands();

        // The world item carries its own transform...
        assert!(
            commands.iter().any(|command| matches!(
                command,
                DrawCommand::SetTransform(t) if t.origin == Vec2::new(10.0, 10.0)
            )),
            "world transform was not applied"
        );

        // ...and both the world quad and the panel rect are painted, world first.
        let fills: Vec<Rect> = commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::FillRect { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(
            fills.len(),
            2,
            "expected a world quad and the root panel fill"
        );
        assert_eq!(fills[0].size, Size::new(20.0, 20.0));
        // The UI root is pinned to the viewport and painted after the world item.
        assert_eq!(fills[1].size, Size::new(200.0, 200.0));
    }

    /// A `Control` parented to a `Node2D` is a canvas item like any other: it
    /// resolves anchors against the node's origin and paints under the node's
    /// world transform, so a name tag / health bar follows its actor (H2).
    #[test]
    fn a_control_under_a_node2d_follows_the_world() {
        use igui_core::{Transform2D, Vec2};

        let mut tree = SceneTree::new();
        tree.set_viewport_size(Size::new(200.0, 200.0));
        let actor = tree.add_node2d(tree.root(), "actor");
        tree.set_position(actor, Vec2::new(40.0, 30.0));
        let tag = add(&mut tree, actor, rect(0.0, 0.0, 60.0, 20.0), label("name"));

        crate::layout(&mut tree, ViewportSize::new(Size::new(200.0, 200.0)));
        tree.update();

        // Anchors resolved against the node origin, not the viewport.
        assert_eq!(
            crate::control(&tree, tag).unwrap().rect,
            Rect::from_min_size(Vec2::ZERO, Size::new(60.0, 20.0))
        );

        let list = paint(&tree);
        assert!(
            list.commands().iter().any(|command| {
                matches!(command, DrawCommand::SetTransform(t)
                    if *t == Transform2D::from_translation(Vec2::new(40.0, 30.0)))
            }),
            "the node's world transform is applied to the control"
        );
    }

    /// A `Control` under a transformed `CanvasLayer` composites under the layer
    /// transform (previously the UI ignored it entirely).
    #[test]
    fn a_control_under_a_transformed_layer_is_offset() {
        use igui_core::{Transform2D, Vec2};

        let mut tree = SceneTree::new();
        tree.set_viewport_size(Size::new(200.0, 200.0));
        let layer = tree.add_canvas_layer(tree.root(), "hud");
        tree.set_canvas_layer_transform(layer, Transform2D::from_translation(Vec2::new(15.0, 5.0)));
        add(&mut tree, layer, rect(0.0, 0.0, 50.0, 20.0), panel());

        crate::layout(&mut tree, ViewportSize::new(Size::new(200.0, 200.0)));
        tree.update();

        let list = paint(&tree);
        assert!(list.commands().iter().any(|command| {
            matches!(command, DrawCommand::SetTransform(t)
                if *t == Transform2D::from_translation(Vec2::new(15.0, 5.0)))
        }));
    }
}
