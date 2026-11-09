//! The text tool: typing, formatting and pasting text.

use super::*;

#[test]
fn typewriter_types_and_edits_text() {
    let mut document = plain_document();
    document.set_brush(RED, 4.0);
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    let tapped = document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    assert!(tapped.text_input, "tapping with the text tool should ask for the keyboard");

    document.handle_text("Hola");
    document.handle_key(KeyboardKey::BackSpace, false, false);
    let state = document.handle_text("!");

    assert!(state.modified, "got {state:?}");
    assert_eq!(texts(&document), ["Hol!"]);
    let red_pixels = count_dark_pixels(&mut document, 80.0, 80.0, 1.0, 200);
    assert!(red_pixels > 0, "the text should be rendered");
}

#[test]
fn text_format_applies_to_the_text_being_edited() {
    let mut document = plain_document();
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_text("Hola");
    let small = count_dark_pixels(&mut document, 80.0, 60.0, 1.0, 300);

    let format = TextFormat {
        font_family: "serif".to_owned(),
        font_size: 64.0,
        bold: true,
        italic: false,
        alignment: rnote_engine::strokes::textstroke::TextAlignment::Start,
    };
    document.set_text_format(&format);
    let large = count_dark_pixels(&mut document, 80.0, 60.0, 1.0, 300);

    assert!(large > small * 2, "bigger bold text should cover more: {small} -> {large}");
}

#[test]
fn pasted_text_becomes_a_text_box() {
    let mut document = plain_document();

    let state = document.paste_text("Pegado", Vector2::new(100.0, 100.0));

    assert!(state.modified, "got {state:?}");
    assert_eq!(texts(&document), ["Pegado"]);
}

#[test]
fn formats_only_the_selected_text() {
    let mut document = plain_document();
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_text("Hola mundo");
    // Shift+Home selects from the cursor back to the start of the line.
    document.handle_key(KeyboardKey::Home, true, false);

    document.toggle_text_emphasis(TextEmphasis::Bold);
    document.color_selected_text(RED);

    let attributes = text_attributes(&document);
    assert_eq!(attributes.len(), 2, "got {attributes:?}");
    assert!(attributes[0].starts_with("0..10 FontWeight(700)"), "got {attributes:?}");
    assert!(attributes[1].starts_with("0..10 TextColor"), "got {attributes:?}");
}

#[test]
fn ctrl_a_selects_all_the_text() {
    let mut document = plain_document();
    let spot = [PenSample { x: 100.0, y: 100.0, pressure: 1.0 }];
    document.handle_pen(PenPhase::Down, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_pen(PenPhase::Up, Tool::Typewriter, &spot, IDENTITY, VIEW);
    document.handle_text("abc");

    document.handle_key(KeyboardKey::Unicode('a'), false, true);
    document.toggle_text_emphasis(TextEmphasis::Italic);

    assert_eq!(texts(&document), ["abc"], "Ctrl+A must not type an 'a'");
    let attributes = text_attributes(&document);
    assert!(attributes.iter().any(|a| a.starts_with("0..3 Style(Italic)")), "got {attributes:?}");
}
