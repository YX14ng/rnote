//! What the "new notebook" dialogs offer: page sizes, papers and line spacings, the same in
//! every app, and the choice among them that a setup stands for.

use super::setup::{DARK_LINES, DARK_PAPER, NotebookSetup, SetupLayout, SetupPattern};

/// A standard page size, portrait, in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    /// Width in millimeters.
    pub width_mm: f64,
    /// Height in millimeters.
    pub height_mm: f64,
}

/// A paper color, with the color of its template lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paper {
    /// The paper, as ARGB.
    pub color: u32,
    /// The template drawn on it, as ARGB.
    pub lines: u32,
}

/// The lines on light paper.
const LIGHT_LINES: u32 = 0xFFC8_CCD6;

/// A3, A4, A5, Letter and Legal.
pub const PAGE_SIZES: [PageSize; 5] = [
    PageSize { width_mm: 297.0, height_mm: 420.0 },
    PageSize { width_mm: 210.0, height_mm: 297.0 },
    PageSize { width_mm: 148.0, height_mm: 210.0 },
    PageSize { width_mm: 215.9, height_mm: 279.4 },
    PageSize { width_mm: 215.9, height_mm: 355.6 },
];

/// White, cream, light gray and dark (StarNote's black).
pub const PAPERS: [Paper; 4] = [
    Paper { color: 0xFFFF_FFFF, lines: LIGHT_LINES },
    Paper { color: 0xFFFB_F6E9, lines: LIGHT_LINES },
    Paper { color: 0xFFEE_EEEE, lines: LIGHT_LINES },
    Paper { color: DARK_PAPER, lines: DARK_LINES },
];

/// The line spacings offered, in millimeters.
pub const SPACINGS_MM: [f64; 5] = [4.0, 5.0, 6.0, 8.0, 10.0];

/// A4, the dark paper and 5 mm: what a setup outside the tables shows as.
const DEFAULT_SIZE: usize = 1;
const DEFAULT_PAPER: usize = 3;
const DEFAULT_SPACING: usize = 1;

/// How close a size must be to one of the table, in millimeters.
const TOLERANCE_MM: f64 = 1.0;

/// What is chosen in a "new notebook" dialog, as indices into the tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupChoice {
    /// Index into [`PAGE_SIZES`].
    pub size: usize,
    /// Whether the page is wider than tall.
    pub landscape: bool,
    /// How the pages grow.
    pub layout: SetupLayout,
    /// Index into [`PAPERS`].
    pub paper: usize,
    /// The template.
    pub pattern: SetupPattern,
    /// Index into [`SPACINGS_MM`].
    pub spacing: usize,
    /// The cover color, as ARGB, or `None` to show the first page.
    pub cover: Option<u32>,
}

impl SetupChoice {
    /// The choice that `setup` stands for; what isn't in the tables shows as the default.
    #[must_use]
    pub fn from_setup(setup: &NotebookSetup) -> Self {
        let (short, long) = if setup.page_width_mm <= setup.page_height_mm {
            (setup.page_width_mm, setup.page_height_mm)
        } else {
            (setup.page_height_mm, setup.page_width_mm)
        };
        let size = PAGE_SIZES
            .iter()
            .position(|size| {
                (size.width_mm - short).abs() < TOLERANCE_MM
                    && (size.height_mm - long).abs() < TOLERANCE_MM
            })
            .unwrap_or(DEFAULT_SIZE);
        let paper = PAPERS
            .iter()
            .position(|paper| paper.color == setup.background_argb)
            .unwrap_or(DEFAULT_PAPER);
        let spacing = SPACINGS_MM
            .iter()
            .position(|spacing| (spacing - setup.pattern_size_mm).abs() < f64::EPSILON)
            .unwrap_or(DEFAULT_SPACING);
        Self {
            size,
            landscape: setup.page_width_mm > setup.page_height_mm,
            layout: setup.layout,
            paper,
            pattern: setup.pattern,
            spacing,
            cover: setup.cover,
        }
    }

    /// The setup of this choice.
    #[must_use]
    pub fn setup(&self) -> NotebookSetup {
        let size = PAGE_SIZES.get(self.size).unwrap_or(&PAGE_SIZES[DEFAULT_SIZE]);
        let paper = PAPERS.get(self.paper).unwrap_or(&PAPERS[DEFAULT_PAPER]);
        let spacing = SPACINGS_MM.get(self.spacing).unwrap_or(&SPACINGS_MM[DEFAULT_SPACING]);
        let (page_width_mm, page_height_mm) = if self.landscape {
            (size.height_mm, size.width_mm)
        } else {
            (size.width_mm, size.height_mm)
        };
        NotebookSetup {
            page_width_mm,
            page_height_mm,
            layout: self.layout,
            background_argb: paper.color,
            pattern: self.pattern,
            pattern_argb: paper.lines,
            pattern_size_mm: *spacing,
            cover: self.cover,
        }
    }
}
