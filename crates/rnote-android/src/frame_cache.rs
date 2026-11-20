//! The last frame, kept to build the next one by redrawing only where something changed.
//!
//! Drawing a frame whole means drawing every stroke on screen again, which on a full page
//! takes a good part of a second per second of erasing or dragging. When the view and the
//! page's look are the same, the strokes that came, went or changed are found by comparing
//! the strokes on screen with the previous frame's, and only the region they cover is
//! painted again, from scratch: desk, background, pages, pictures and ink, in the same order
//! as a whole frame, so it looks the same.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

use parry2d_f64::bounding_volume::{Aabb, BoundingVolume};
use parry2d_f64::math::Vector2;
use rnote_compose::shapes::Shapeable;
use rnote_engine::strokes::Stroke;

use crate::render::{Scene, Viewport, copy_surface_to_rgba, new_surface};

/// Beyond this share of the view changed, a frame is drawn whole.
const MOST_OF_THE_VIEW: f64 = 0.5;

/// Device pixels added around what changed: antialiasing reaches a little past the bounds.
const MARGIN_PX: f64 = 2.0;

/// The kept frame, without what the pen shows on top.
#[derive(Default)]
pub(crate) struct FrameCache {
    kept: RefCell<Option<Kept>>,
    // The pen's overlay is drawn on a copy, so the kept frame stays clean.
    scratch: RefCell<Option<cairo::ImageSurface>>,
    partial: Cell<usize>,
}

struct Kept {
    looks: Looks,
    surface: cairo::ImageSurface,
    // The strokes it shows: holding them keeps a changed stroke from ever reusing the same
    // allocation, so an unchanged pointer is an unchanged stroke.
    strokes: Vec<(Arc<Stroke>, Aabb)>,
}

/// What makes two frames comparable: the same view of the same-looking page.
#[derive(PartialEq)]
struct Looks {
    viewport: [u64; 3],
    layout: (u32, u32),
    doc_bounds: [u64; 4],
    // The page's format, background and layout, as their JSON: not all compare directly.
    page: String,
}

impl FrameCache {
    /// How many frames since the last whole one were built from the one before.
    #[cfg(test)]
    pub(crate) fn partial_frames(&self) -> usize {
        self.partial.get()
    }

    /// Renders `scene` into `out` (see [`crate::render::render_rgba`]).
    pub(crate) fn render(&self, scene: &Scene<'_>, out: &mut [u8]) -> anyhow::Result<()> {
        let looks = Looks::of(scene);
        let content = scene.content();
        let strokes: Vec<(Arc<Stroke>, Aabb)> = content
            .strokes
            .iter()
            .map(|layered| (Arc::clone(&layered.stroke), layered.stroke.bounds()))
            .collect();
        let mut kept = self.kept.borrow_mut();
        let reusable = kept.as_mut().filter(|kept| kept.looks == looks);
        match reusable.map(|kept| (changed_region(&kept.strokes, &strokes), kept)) {
            Some((None, kept)) => kept.strokes = strokes,
            Some((Some(changed), kept)) if small(changed, scene) => {
                scene.draw_base(
                    &kept.surface,
                    &content.strokes,
                    Some(pixel_aligned(changed, scene.viewport)),
                )?;
                kept.strokes = strokes;
                self.partial.set(self.partial.get() + 1);
            }
            _ => {
                let surface = new_surface(scene.layout.width, scene.layout.height)?;
                scene.draw_base(&surface, &content.strokes, None)?;
                *kept = Some(Kept { looks, surface, strokes });
                self.partial.set(0);
            }
        }
        let kept = kept.as_mut().expect("a frame is kept by now");
        self.finish(scene, kept, out)
    }

    /// Puts the pen's overlay on a copy of the kept frame and hands the pixels out.
    fn finish(&self, scene: &Scene<'_>, kept: &mut Kept, out: &mut [u8]) -> anyhow::Result<()> {
        let layout = scene.layout;
        let mut scratch = self.scratch.borrow_mut();
        let fits = scratch.as_ref().is_some_and(|surface| {
            u32::try_from(surface.width()) == Ok(layout.width)
                && u32::try_from(surface.height()) == Ok(layout.height)
        });
        if !fits {
            *scratch = Some(new_surface(layout.width, layout.height)?);
        }
        let copy = scratch.as_mut().expect("just made");
        {
            let cx = cairo::Context::new(&*copy)?;
            cx.set_operator(cairo::Operator::Source);
            kept.surface.flush();
            cx.set_source_surface(&kept.surface, 0.0, 0.0)?;
            cx.paint()?;
        }
        scene.draw_pen(copy)?;
        copy_surface_to_rgba(copy, out, layout)
    }
}

impl Looks {
    fn of(scene: &Scene<'_>) -> Self {
        let Viewport { x, y, zoom } = scene.viewport;
        let bounds = scene.doc_bounds;
        Self {
            viewport: [x, y, zoom].map(f64::to_bits),
            layout: (scene.layout.width, scene.layout.height),
            doc_bounds: [bounds.mins.x, bounds.mins.y, bounds.maxs.x, bounds.maxs.y]
                .map(f64::to_bits),
            page: serde_json::to_string(&scene.looks()).unwrap_or_default(),
        }
    }
}

/// Where `now` differs from `before`: the bounds of the strokes that came, went or changed.
fn changed_region(before: &[(Arc<Stroke>, Aabb)], now: &[(Arc<Stroke>, Aabb)]) -> Option<Aabb> {
    let mut gone: HashMap<*const Stroke, Aabb> =
        before.iter().map(|(stroke, bounds)| (Arc::as_ptr(stroke), *bounds)).collect();
    let mut changed: Option<Aabb> = None;
    let mut add =
        |bounds: Aabb| changed = Some(changed.map_or(bounds, |region| region.merged(&bounds)));
    for (stroke, bounds) in now {
        match gone.remove(&Arc::as_ptr(stroke)) {
            Some(old) if old == *bounds => {}
            Some(old) => {
                add(old);
                add(*bounds);
            }
            None => add(*bounds),
        }
    }
    for old in gone.into_values() {
        add(old);
    }
    changed
}

/// Whether redrawing `region` is cheaper than drawing the frame whole.
fn small(region: Aabb, scene: &Scene<'_>) -> bool {
    let view = scene.viewport_bounds();
    let Some(shown) = region.intersection(&view) else {
        return true;
    };
    let area = |bounds: Aabb| bounds.extents().x * bounds.extents().y;
    area(shown) <= MOST_OF_THE_VIEW * area(view)
}

/// `region` grown to whole device pixels, with a margin, in document coordinates.
fn pixel_aligned(region: Aabb, viewport: Viewport) -> Aabb {
    let to_px = |doc: f64, origin: f64| (doc - origin) * viewport.zoom;
    let to_doc = |px: f64, origin: f64| origin + px / viewport.zoom;
    let left = (to_px(region.mins.x, viewport.x) - MARGIN_PX).floor();
    let top = (to_px(region.mins.y, viewport.y) - MARGIN_PX).floor();
    let right = (to_px(region.maxs.x, viewport.x) + MARGIN_PX).ceil();
    let bottom = (to_px(region.maxs.y, viewport.y) + MARGIN_PX).ceil();
    Aabb::new(
        Vector2::new(to_doc(left, viewport.x), to_doc(top, viewport.y)),
        Vector2::new(to_doc(right, viewport.x), to_doc(bottom, viewport.y)),
    )
}
