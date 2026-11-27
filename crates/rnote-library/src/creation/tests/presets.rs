use crate::creation::{
    DARK_LINES, DARK_PAPER, NotebookSetup, PAGE_SIZES, PAPERS, SPACINGS_MM, SetupChoice,
    SetupLayout, SetupPattern,
};

#[test]
fn the_tables_match_the_android_app() {
    let sizes: Vec<(f64, f64)> =
        PAGE_SIZES.iter().map(|size| (size.width_mm, size.height_mm)).collect();
    assert_eq!(
        sizes,
        [(297.0, 420.0), (210.0, 297.0), (148.0, 210.0), (215.9, 279.4), (215.9, 355.6)]
    );
    let papers: Vec<(u32, u32)> = PAPERS.iter().map(|paper| (paper.color, paper.lines)).collect();
    assert_eq!(
        papers,
        [
            (0xFFFF_FFFF, 0xFFC8_CCD6),
            (0xFFFB_F6E9, 0xFFC8_CCD6),
            (0xFFEE_EEEE, 0xFFC8_CCD6),
            (DARK_PAPER, DARK_LINES),
        ]
    );
    assert_eq!(SPACINGS_MM, [4.0, 5.0, 6.0, 8.0, 10.0]);
}

#[test]
fn the_default_setup_is_chosen_as_a4_dark_grid_5mm() {
    let choice = SetupChoice::from_setup(&NotebookSetup::default());

    assert_eq!(
        choice,
        SetupChoice {
            size: 1,
            landscape: false,
            layout: SetupLayout::FixedSize,
            paper: 3,
            pattern: SetupPattern::Grid,
            spacing: 1,
            cover: None,
        }
    );
    assert_eq!(choice.setup(), NotebookSetup::default());
}

#[test]
fn a_landscape_letter_setup_round_trips() {
    let setup = NotebookSetup {
        page_width_mm: 279.4,
        page_height_mm: 215.9,
        layout: SetupLayout::Infinite,
        background_argb: 0xFFFB_F6E9,
        pattern: SetupPattern::Lines,
        pattern_argb: 0xFFC8_CCD6,
        pattern_size_mm: 8.0,
        cover: Some(0xFF3F_8F6B),
    };

    let choice = SetupChoice::from_setup(&setup);

    assert_eq!((choice.size, choice.landscape, choice.paper, choice.spacing), (3, true, 1, 3));
    assert_eq!(choice.setup(), setup);
}

#[test]
fn values_outside_the_tables_fall_back_to_the_defaults() {
    let setup = NotebookSetup {
        page_width_mm: 100.0,
        page_height_mm: 100.0,
        background_argb: 0xFF12_3456,
        pattern_size_mm: 7.0,
        ..NotebookSetup::default()
    };

    let choice = SetupChoice::from_setup(&setup);

    assert_eq!((choice.size, choice.paper, choice.spacing), (1, 3, 1));
}
