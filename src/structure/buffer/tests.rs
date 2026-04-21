use super::*;
use crate::model::{BoundingBox, PrimitiveKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal text primitive on the given page.
fn text_prim(page: u32, order: u64, content: &str) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.05, order as f32 * 0.05, 0.9, 0.04),
        page,
        order,
    )
}

/// Create a text primitive with a specific font size (for histogram tests).
fn text_prim_sized(page: u32, order: u64, font_size: f32) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: "text".to_string(),
            font_size,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.05, 0.1, 0.9, 0.04),
        page,
        order,
    )
}

/// Create a line primitive on the given page.
fn line_prim(page: u32, order: u64) -> Primitive {
    use crate::model::Point;
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(0.1, 0.5),
            end: Point::new(0.9, 0.5),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(0.1, 0.5, 0.8, 0.001),
        page,
        order,
    )
}

// ---------------------------------------------------------------------------
// Single-page document
// ---------------------------------------------------------------------------

#[test]
fn single_page_flush_on_finish() {
    let mut buffer = PageBuffer::with_defaults();

    // Ingest 3 primitives on page 0 — no flush triggered.
    assert!(buffer.ingest(text_prim(0, 0, "Hello")).is_none());
    assert!(buffer.ingest(text_prim(0, 1, "world")).is_none());
    assert!(buffer.ingest(text_prim(0, 2, "!")).is_none());

    assert_eq!(buffer.buffered_count(), 3);
    assert_eq!(buffer.current_page(), 0);

    // Finish — should produce one flush with all 3 primitives.
    let flush = buffer.finish().expect("expected flush on finish");
    assert_eq!(flush.page, 0);
    assert_eq!(flush.primitives.len(), 3);
    assert!(flush.page_dims.is_none());
}

// ---------------------------------------------------------------------------
// Multi-page document
// ---------------------------------------------------------------------------

#[test]
fn multi_page_flushes_on_boundary() {
    let mut buffer = PageBuffer::with_defaults();

    // Page 0: 2 primitives
    assert!(buffer.ingest(text_prim(0, 0, "p0a")).is_none());
    assert!(buffer.ingest(text_prim(0, 1, "p0b")).is_none());

    // Page 1 starts — page 0 should flush.
    let flush0 = buffer
        .ingest(text_prim(1, 2, "p1a"))
        .expect("expected flush on page boundary");
    assert_eq!(flush0.page, 0);
    assert_eq!(flush0.primitives.len(), 2);

    // More on page 1
    assert!(buffer.ingest(text_prim(1, 3, "p1b")).is_none());
    assert!(buffer.ingest(text_prim(1, 4, "p1c")).is_none());

    // Page 2 starts — page 1 should flush.
    let flush1 = buffer
        .ingest(text_prim(2, 5, "p2a"))
        .expect("expected flush on page boundary");
    assert_eq!(flush1.page, 1);
    assert_eq!(flush1.primitives.len(), 3);

    // Finish — page 2 should flush.
    let flush2 = buffer.finish().expect("expected flush on finish");
    assert_eq!(flush2.page, 2);
    assert_eq!(flush2.primitives.len(), 1);
}

// ---------------------------------------------------------------------------
// Memory contract: buffer is empty after flush
// ---------------------------------------------------------------------------

#[test]
fn buffer_empty_after_flush() {
    let mut buffer = PageBuffer::with_defaults();

    buffer.ingest(text_prim(0, 0, "a"));
    buffer.ingest(text_prim(0, 1, "b"));

    // Force flush by sending page 1.
    let flush = buffer.ingest(text_prim(1, 2, "c")).unwrap();
    assert_eq!(flush.primitives.len(), 2);

    // Buffer should now hold only the page-1 primitive.
    assert_eq!(buffer.buffered_count(), 1);
    assert_eq!(buffer.current_page(), 1);
}

// ---------------------------------------------------------------------------
// Empty document
// ---------------------------------------------------------------------------

#[test]
fn empty_document_produces_no_flush() {
    let buffer = PageBuffer::with_defaults();
    assert!(buffer.finish().is_none());
}

// ---------------------------------------------------------------------------
// Out-of-order page numbers
// ---------------------------------------------------------------------------

#[test]
fn out_of_order_page_emits_warning() {
    let mut buffer = PageBuffer::with_defaults();

    buffer.ingest(text_prim(1, 0, "page1"));
    // Send a primitive for page 0 — out of order.
    assert!(buffer.ingest(text_prim(0, 1, "page0_late")).is_none());

    assert_eq!(buffer.warnings().len(), 1);
    assert!(
        buffer.warnings()[0]
            .message
            .contains("Out-of-order page number")
    );

    // The out-of-order primitive should be buffered on the current page (1).
    assert_eq!(buffer.buffered_count(), 2);
    assert_eq!(buffer.current_page(), 1);

    let flush = buffer.finish().unwrap();
    assert_eq!(flush.page, 1);
    assert_eq!(flush.primitives.len(), 2);
}

// ---------------------------------------------------------------------------
// Page dimensions
// ---------------------------------------------------------------------------

#[test]
fn page_dims_attached_to_flush() {
    use crate::model::geometry::RawBox;

    let mut buffer = PageBuffer::with_defaults();

    buffer.set_page_dims(PageDimensions::new(
        RawBox::new(0.0, 0.0, 612.0, 792.0),
        None,
        0,
    ));
    buffer.ingest(text_prim(0, 0, "with dims"));

    let flush = buffer.finish().unwrap();
    assert!(flush.page_dims.is_some());
    let dims = flush.page_dims.unwrap();
    assert!((dims.effective_width - 612.0).abs() < f32::EPSILON);
    assert!((dims.effective_height - 792.0).abs() < f32::EPSILON);
}

#[test]
fn page_dims_reset_after_flush() {
    use crate::model::geometry::RawBox;

    let mut buffer = PageBuffer::with_defaults();

    buffer.set_page_dims(PageDimensions::new(
        RawBox::new(0.0, 0.0, 612.0, 792.0),
        None,
        0,
    ));
    buffer.ingest(text_prim(0, 0, "page0"));

    // Flush page 0 by starting page 1.
    let flush0 = buffer.ingest(text_prim(1, 1, "page1")).unwrap();
    assert!(flush0.page_dims.is_some());

    // Page 1 should NOT carry page 0's dimensions.
    let flush1 = buffer.finish().unwrap();
    assert!(flush1.page_dims.is_none());
}

// ---------------------------------------------------------------------------
// Font histogram
// ---------------------------------------------------------------------------

#[test]
fn font_histogram_tracks_sizes() {
    let mut buffer = PageBuffer::with_defaults();

    // 5 primitives at 12pt (body), 1 at 18pt (heading), 1 at 24pt (title).
    for i in 0..5 {
        buffer.ingest(text_prim_sized(0, i, 12.0));
    }
    buffer.ingest(text_prim_sized(0, 5, 18.0));
    buffer.ingest(text_prim_sized(0, 6, 24.0));

    let hist = buffer.font_histogram();
    assert_eq!(hist.total(), 7);
    assert_eq!(hist.distinct_sizes(), 3);
    assert!((hist.body_font_size().unwrap() - 12.0).abs() < f32::EPSILON);
}

#[test]
fn font_histogram_top_percentile() {
    let mut buffer = PageBuffer::with_defaults();

    // 90 primitives at 12pt, 8 at 16pt, 2 at 24pt.
    for i in 0..90 {
        buffer.ingest(text_prim_sized(0, i, 12.0));
    }
    for i in 90..98 {
        buffer.ingest(text_prim_sized(0, i, 16.0));
    }
    for i in 98..100 {
        buffer.ingest(text_prim_sized(0, i, 24.0));
    }

    let hist = buffer.font_histogram();

    // 24pt: 2/100 = 2% → should be in top 10%
    assert!(hist.is_in_top_percentile(24.0, 0.10));

    // 16pt: (8+2)/100 = 10% → should be in top 10% (boundary)
    assert!(hist.is_in_top_percentile(16.0, 0.10));

    // 12pt: 100/100 = 100% → should NOT be in top 10%
    assert!(!hist.is_in_top_percentile(12.0, 0.10));
}

#[test]
fn font_histogram_survives_page_flush() {
    let mut buffer = PageBuffer::with_defaults();

    // Page 0: 3 text primitives at 12pt.
    for i in 0..3 {
        buffer.ingest(text_prim_sized(0, i, 12.0));
    }

    // Flush page 0 by starting page 1.
    buffer.ingest(text_prim_sized(1, 3, 18.0));

    // Histogram should still have the page-0 data plus the page-1 primitive.
    assert_eq!(buffer.font_histogram().total(), 4);
    assert_eq!(buffer.font_histogram().distinct_sizes(), 2);
}

// ---------------------------------------------------------------------------
// Non-text primitives
// ---------------------------------------------------------------------------

#[test]
fn non_text_primitives_buffered_correctly() {
    let mut buffer = PageBuffer::with_defaults();

    buffer.ingest(text_prim(0, 0, "text"));
    buffer.ingest(line_prim(0, 1));
    buffer.ingest(text_prim(0, 2, "more text"));

    // Only text primitives should be in the histogram (check before finish consumes self).
    assert_eq!(buffer.font_histogram().total(), 2);

    let flush = buffer.finish().unwrap();
    assert_eq!(flush.primitives.len(), 3);
}

// ---------------------------------------------------------------------------
// Max primitives warning
// ---------------------------------------------------------------------------

#[test]
fn max_primitives_warning_emitted() {
    let config = StructureConfig {
        max_primitives_per_page: 3,
        ..Default::default()
    };
    let mut buffer = PageBuffer::new(config);

    // Ingest 4 primitives — warning should fire at the 4th (index 3).
    for i in 0..4 {
        buffer.ingest(text_prim(0, i, "x"));
    }

    assert_eq!(buffer.warnings().len(), 1);
    assert!(
        buffer.warnings()[0]
            .message
            .contains("exceeds max_primitives_per_page")
    );

    // But all 4 should be buffered (we don't drop primitives).
    assert_eq!(buffer.buffered_count(), 4);
}

// ---------------------------------------------------------------------------
// Page gap (non-consecutive pages)
// ---------------------------------------------------------------------------

#[test]
fn non_consecutive_pages_flush_correctly() {
    let mut buffer = PageBuffer::with_defaults();

    buffer.ingest(text_prim(0, 0, "page0"));

    // Jump from page 0 to page 5.
    let flush0 = buffer.ingest(text_prim(5, 1, "page5")).unwrap();
    assert_eq!(flush0.page, 0);
    assert_eq!(flush0.primitives.len(), 1);

    let flush5 = buffer.finish().unwrap();
    assert_eq!(flush5.page, 5);
    assert_eq!(flush5.primitives.len(), 1);
}

// ---------------------------------------------------------------------------
// take_warnings
// ---------------------------------------------------------------------------

#[test]
fn take_warnings_clears_internal_vec() {
    let mut buffer = PageBuffer::with_defaults();

    buffer.ingest(text_prim(1, 0, "page1"));
    buffer.ingest(text_prim(0, 1, "out_of_order"));

    let warnings = buffer.take_warnings();
    assert_eq!(warnings.len(), 1);
    assert!(buffer.warnings().is_empty());
}

// ---------------------------------------------------------------------------
// Monitoring counters
// ---------------------------------------------------------------------------

#[test]
fn pages_flushed_counter_increments() {
    let mut buffer = PageBuffer::with_defaults();

    assert_eq!(buffer.pages_flushed(), 0);

    buffer.ingest(text_prim(0, 0, "a"));
    buffer.ingest(text_prim(0, 1, "b"));

    // Cross page boundary → flushes page 0.
    let _ = buffer.ingest(text_prim(1, 2, "c"));
    assert_eq!(buffer.pages_flushed(), 1);

    // Cross again.
    let _ = buffer.ingest(text_prim(2, 3, "d"));
    assert_eq!(buffer.pages_flushed(), 2);

    // finish flushes the last page.
    let _ = buffer.finish();
    // Can't check after finish (consumed), but pages_flushed was 2 + 1 = 3.
}

#[test]
fn total_primitives_counter() {
    let mut buffer = PageBuffer::with_defaults();

    assert_eq!(buffer.total_primitives(), 0);

    buffer.ingest(text_prim(0, 0, "a"));
    buffer.ingest(text_prim(0, 1, "b"));
    buffer.ingest(text_prim(0, 2, "c"));

    assert_eq!(buffer.total_primitives(), 3);

    // Cross page.
    let _ = buffer.ingest(text_prim(1, 3, "d"));
    assert_eq!(buffer.total_primitives(), 4);
}

#[test]
fn peak_page_primitives_tracks_maximum() {
    let mut buffer = PageBuffer::with_defaults();

    // Page 0: 5 primitives.
    for i in 0..5 {
        buffer.ingest(text_prim(0, i, "x"));
    }

    // Page 1: 2 primitives (flushes page 0 first).
    let _ = buffer.ingest(text_prim(1, 5, "y"));
    assert_eq!(buffer.peak_page_primitives(), 5);

    let _ = buffer.ingest(text_prim(1, 6, "z"));

    // Page 2: 1 primitive (flushes page 1).
    let _ = buffer.ingest(text_prim(2, 7, "w"));
    assert_eq!(buffer.peak_page_primitives(), 5); // Still 5 from page 0.
}
