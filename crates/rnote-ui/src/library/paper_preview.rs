//! A small picture of a new notebook's page: its shape, paper and template, or its cover.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use rnote_library::creation::{SetupChoice, SetupPattern};

/// The height of the preview, in pixels.
const HEIGHT: i32 = 180;

/// What the preview draws.
#[derive(Debug, Clone, Copy)]
struct Shown {
    choice: SetupChoice,
}

/// The preview, redrawn by [`PaperPreview::show`].
#[derive(Debug, Clone)]
pub(crate) struct PaperPreview {
    area: gtk4::DrawingArea,
    shown: Rc<Cell<Option<Shown>>>,
}

impl PaperPreview {
    pub(crate) fn new() -> Self {
        let area = gtk4::DrawingArea::builder()
            .content_height(HEIGHT)
            .margin_bottom(12)
            .build();
        let shown: Rc<Cell<Option<Shown>>> = Rc::new(Cell::new(None));
        let drawn = shown.clone();
        area.set_draw_func(move |_, cr, width, height| {
            if let Some(shown) = drawn.get() {
                draw(cr, f64::from(width), f64::from(height), &shown.choice);
            }
        });
        Self { area, shown }
    }

    pub(crate) fn widget(&self) -> &gtk4::DrawingArea {
        &self.area
    }

    /// Shows the page that `choice` makes.
    pub(crate) fn show(&self, choice: &SetupChoice) {
        self.shown.set(Some(Shown { choice: *choice }));
        self.area.queue_draw();
    }
}

fn rgba(argb: u32) -> (f64, f64, f64, f64) {
    let channel = |shift: u32| f64::from((argb >> shift) & 0xFF) / 255.0;
    (channel(16), channel(8), channel(0), channel(24))
}

fn draw(cr: &gtk4::cairo::Context, width: f64, height: f64, choice: &SetupChoice) {
    let setup = choice.setup();
    let scale = (height / setup.page_height_mm).min(width / setup.page_width_mm);
    let (page_width, page_height) = (setup.page_width_mm * scale, setup.page_height_mm * scale);
    let (left, top) = ((width - page_width) / 2.0, (height - page_height) / 2.0);
    cr.rectangle(left, top, page_width, page_height);
    let (red, green, blue, alpha) = rgba(setup.background_argb);
    cr.set_source_rgba(red, green, blue, alpha);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.5, 0.5, 0.5, 0.5);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
    if let Some(cover) = setup.cover {
        draw_cover(cr, (left, top, page_width, page_height), cover);
        return;
    }
    let _ = cr.save();
    cr.rectangle(left, top, page_width, page_height);
    cr.clip();
    let (red, green, blue, alpha) = rgba(setup.pattern_argb);
    cr.set_source_rgba(red, green, blue, alpha);
    cr.set_line_width(1.0);
    let step = (setup.pattern_size_mm * scale).max(3.0);
    let rows = (page_height / step).ceil() as usize;
    let columns = (page_width / step).ceil() as usize;
    match setup.pattern {
        SetupPattern::None => {}
        SetupPattern::Lines => {
            for row in 1..=rows {
                let y = top + row as f64 * step;
                cr.move_to(left, y);
                cr.line_to(left + page_width, y);
            }
            let _ = cr.stroke();
        }
        SetupPattern::Grid => {
            for row in 1..=rows {
                let y = top + row as f64 * step;
                cr.move_to(left, y);
                cr.line_to(left + page_width, y);
            }
            for column in 1..=columns {
                let x = left + column as f64 * step;
                cr.move_to(x, top);
                cr.line_to(x, top + page_height);
            }
            let _ = cr.stroke();
        }
        SetupPattern::Dots => {
            for row in 1..=rows {
                for column in 1..=columns {
                    let (x, y) = (left + column as f64 * step, top + row as f64 * step);
                    cr.arc(x, y, 0.8, 0.0, std::f64::consts::TAU);
                    let _ = cr.fill();
                }
            }
        }
    }
    let _ = cr.restore();
}

fn draw_cover(cr: &gtk4::cairo::Context, page: (f64, f64, f64, f64), cover: u32) {
    let (left, top, width, height) = page;
    let (red, green, blue, alpha) = rgba(cover);
    cr.set_source_rgba(red, green, blue, alpha);
    cr.rectangle(left, top, width, height);
    let _ = cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.18);
    cr.rectangle(left, top, width * 0.08, height);
    let _ = cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.86);
    cr.rectangle(
        left + width * 0.18,
        top + height * 0.18,
        width * 0.64,
        height * 0.12,
    );
    let _ = cr.fill();
}
