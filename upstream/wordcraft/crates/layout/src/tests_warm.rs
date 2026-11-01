//! office overlay (wordcraft/0142): [`warm`] in slices leaves [`layout`]'s result unchanged.
use super::*;
use wordcraft_doc::props::NumRef;
use wordcraft_doc::{Pos, Table};

/// A long document with what goes into a paragraph's cache key: list numbers in the body and in a
/// table cell, a page field, and a two-column section.
fn mixed() -> Document {
    let text: Vec<String> =
        (0..400).map(|i| format!("Paragraph {i}: the quick brown fox jumps over the lazy dog {} times, then rests.", i * 7)).collect();
    let mut d = Document::from_text(&text.join("\n"));
    let num = d.numbering.add_list(wordcraft_doc::ListKind::Numbered);
    let listed = |p: &mut wordcraft_doc::props::ParaProps| p.numbering = Some(NumRef { num, level: 0 });
    d.format_paragraphs(&Pos::body(10, 0), &Pos::body(40, 0), &listed).unwrap();
    let mut t = Table::new(2, 2, 400.0);
    let mut cell = Paragraph::with_text("A numbered cell", Default::default());
    cell.props.numbering = Some(NumRef { num, level: 0 });
    t.rows[0].cells[0].blocks = vec![wordcraft_doc::para_block(cell)];
    d.body.insert(25, Arc::new(Block::Table(t)));
    let page = InlineObject::Field { instr: "PAGE".into(), result: String::new(), locked: false, dirty: false, code: false, form: String::new() };
    d.para_mut(StoryRef::Body, &Path::top(100)).unwrap().insert_object(0, page, &Default::default()).unwrap();
    let mut s = wordcraft_doc::section::SectionProps::default();
    s.columns.count = 2;
    d.para_mut(StoryRef::Body, &Path::top(200)).unwrap().section = Some(Box::new(s));
    d
}

/// Warm `d` one block per slice (a zero budget), then lay it out: (layout, slices, paragraphs the
/// warm-up shaped, paragraphs the layout still had to).
fn sliced(d: &Document, opts: &LayoutOptions) -> (DocLayout, usize, u64, u64) {
    let (mut cache, mut w, mut slices) = (LayoutCache::new(), Warm::default(), 1);
    while !warm(d, &mut cache, opts, &mut w, 0.0) {
        slices += 1;
    }
    let shaped = cache.misses;
    let l = layout(d, &mut cache, opts);
    (l, slices, shaped, cache.misses - shaped)
}

#[test]
fn warm_in_slices_gives_the_same_layout() {
    let d = mixed();
    // Proofing on as in the editors: spelling marks are part of a paragraph's layout.
    for opts in [LayoutOptions::default(), LayoutOptions { proofing: true, ..Default::default() }] {
        let cold = layout(&d, &mut LayoutCache::new(), &opts);
        assert!(cold.pages.len() > 5);
        let (l, slices, shaped, missed) = sliced(&d, &opts);
        assert_eq!(format!("{:?}", cold.pages), format!("{:?}", l.pages));
        assert_eq!(cold.index, l.index);
        assert!(slices > 300, "one block per slice: {slices}");
        // The pass finds everything the warm-up shaped, table cells and list numbers included:
        // only the paragraph with a page field (its key holds the page) is left to it.
        assert!(shaped > 400, "warm-up shaped {shaped}");
        assert_eq!(missed, 1, "the pass still laid out {missed} paragraphs");
    }
}

#[test]
fn warm_leaves_other_views_to_the_layout() {
    let d = mixed();
    let opts = LayoutOptions { view: ViewMode::Web, web_width: 360.0, ..Default::default() };
    let mut cache = LayoutCache::new();
    assert!(warm(&d, &mut cache, &opts, &mut Warm::default(), 0.0));
    assert!(cache.is_empty());
}
