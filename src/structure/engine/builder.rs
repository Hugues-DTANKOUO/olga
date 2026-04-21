use super::StructureEngine;
use crate::structure::detectors::{
    HeadingDetector, ListDetector, ParagraphDetector, StructureDetector, TableDetector,
};
use crate::structure::layout::{HeuristicLayoutAnalyzer, LayoutAnalyzer};
use crate::structure::types::StructureConfig;

impl StructureEngine {
    /// Create a new engine with the given configuration and no detectors.
    ///
    /// Without detectors, the engine relies entirely on format-derived hints.
    /// Call [`with_detector`](Self::with_detector) or
    /// [`with_default_detectors`](Self::with_default_detectors) to add
    /// heuristic detection for unhinted pages.
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config,
            detectors: Vec::new(),
            layout_analyzer: None,
        }
    }

    /// Register a heuristic detector.
    ///
    /// Detectors run on each page after reading-order correction and before
    /// assembly. They only emit hints for primitives that lack format-derived
    /// hints (checked by [`apply_detectors`]).
    ///
    /// Multiple detectors can be registered; when they produce hints for the
    /// same primitive, the highest-confidence hint wins.
    pub fn with_detector(mut self, detector: Box<dyn StructureDetector>) -> Self {
        self.detectors.push(detector);
        self
    }

    /// Set the layout analyzer for geometric text block analysis.
    ///
    /// The layout analyzer runs on unhinted pages before detectors,
    /// producing `TextBox`es that provide spatial context for the
    /// detector pipeline.
    pub fn with_layout_analyzer(mut self, analyzer: Box<dyn LayoutAnalyzer>) -> Self {
        self.layout_analyzer = Some(analyzer);
        self
    }

    /// Register the default set of heuristic detectors:
    /// [`ParagraphDetector`], [`HeadingDetector`], [`ListDetector`], and
    /// [`TableDetector`], along with the default [`HeuristicLayoutAnalyzer`].
    ///
    /// This is the recommended configuration for processing documents that
    /// may include untagged PDF pages.
    pub fn with_default_detectors(self) -> Self {
        self.with_layout_analyzer(Box::new(HeuristicLayoutAnalyzer::default()))
            .with_detector(Box::new(ParagraphDetector::default()))
            .with_detector(Box::new(HeadingDetector::default()))
            .with_detector(Box::new(ListDetector::default()))
            .with_detector(Box::new(TableDetector::default()))
    }
}
