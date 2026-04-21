//! Internal types used during HTML DOM walking.

use std::collections::HashSet;

use crate::model::{TextDirection, ValueOrigin};

/// Supported whitespace handling modes for the HTML rendered subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HtmlWhitespaceMode {
    /// Collapse runs of whitespace into a single space.
    #[default]
    Collapse,
    /// Preserve spaces and line breaks.
    Preserve,
    /// Collapse spaces but preserve explicit line breaks.
    PreserveLineBreaks,
}

/// Effective rendering context inherited during DOM traversal.
#[derive(Debug, Clone)]
pub(crate) struct HtmlRenderContext {
    pub lang: Option<String>,
    pub lang_origin: Option<ValueOrigin>,
    pub text_direction: TextDirection,
    pub text_direction_origin: ValueOrigin,
    pub auto_direction: bool,
    pub whitespace_mode: HtmlWhitespaceMode,
    pub list_ordered_hint: Option<bool>,
}

impl Default for HtmlRenderContext {
    fn default() -> Self {
        Self {
            lang: None,
            lang_origin: None,
            text_direction: TextDirection::LeftToRight,
            text_direction_origin: ValueOrigin::Synthetic,
            auto_direction: false,
            whitespace_mode: HtmlWhitespaceMode::Collapse,
            list_ordered_hint: None,
        }
    }
}

/// Tracks the state of a table being parsed.
#[derive(Debug, Default)]
pub(crate) struct TableContext {
    /// Current row index (0-based).
    pub row: u32,
    /// Current column index (0-based), accounting for colspan/rowspan.
    pub col: u32,
    /// Whether the current row group should be treated as table headers.
    pub current_group_is_header: bool,
    /// Column occupancy grid: for each row, the set of columns already claimed
    /// by rowspan from previous rows. Key = row index, Value = set of col indices.
    pub occupied: std::collections::HashMap<u32, std::collections::HashSet<u32>>,
}

impl TableContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance `self.col` to the next unoccupied column in the current row.
    pub fn next_free_col(&mut self) {
        let occupied = self.occupied.get(&self.row);
        while occupied.is_some_and(|s| s.contains(&self.col)) {
            self.col += 1;
        }
    }

    /// Mark cells as occupied after processing a <td>/<th> with colspan/rowspan.
    pub fn mark_occupied(&mut self, row: u32, col: u32, rowspan: u32, colspan: u32) {
        for r in row..row + rowspan {
            for c in col..col + colspan {
                // Don't mark the current cell's own row — it's being processed now.
                if r > row {
                    self.occupied.entry(r).or_default().insert(c);
                }
            }
        }
    }
}

/// Tracks list nesting state.
#[derive(Debug, Clone)]
pub(crate) struct ListContext {
    /// Nesting depth (0 = top-level list).
    pub depth: u8,
    /// Whether this list is ordered (<ol>) or unordered (<ul>).
    pub ordered: bool,
}

/// Walking context passed through the recursive DOM traversal.
#[derive(Debug)]
pub(crate) struct WalkState {
    /// Monotonically increasing source order counter.
    pub source_order: u64,
    /// Current synthetic Y position (incremented per block element).
    pub y_position: f32,
    /// The Y step between block elements (normalized coordinates).
    pub y_step: f32,
    /// Stack of active list contexts (for nested lists).
    pub list_stack: Vec<ListContext>,
    /// Stack of active table contexts (for nested tables).
    pub table_stack: Vec<TableContext>,
    /// Current DOM recursion depth (to prevent stack overflow).
    pub dom_depth: u32,
    /// Whether the DOM depth limit has already been reported.
    pub depth_limit_hit: bool,
    /// Deduplicates warnings for unsupported rendered-subset CSS features.
    pub reported_render_limitations: HashSet<String>,
    /// Deduplicates warnings for recovered structure that comes from heuristics
    /// or CSS-driven fallbacks instead of native semantic markup.
    pub reported_heuristic_inferences: HashSet<String>,
}

impl WalkState {
    pub fn new() -> Self {
        Self {
            source_order: 0,
            y_position: 0.0,
            y_step: 0.02,
            list_stack: Vec::new(),
            table_stack: Vec::new(),
            dom_depth: 0,
            depth_limit_hit: false,
            reported_render_limitations: HashSet::new(),
            reported_heuristic_inferences: HashSet::new(),
        }
    }

    pub fn next_order(&mut self) -> u64 {
        let order = self.source_order;
        self.source_order += 1;
        order
    }

    pub fn advance_y(&mut self) -> f32 {
        let y = self.y_position;
        self.y_position += self.y_step;
        y
    }
}
