//! Heading detector — identifies headings from font-size outliers.
//!
//! # Algorithm
//!
//! 1. From the cumulative [`FontHistogram`], determine the body text
//!    font size (most frequent size).
//! 2. For each unhinted text primitive, check if its font size is
//!    significantly larger than body text AND in the top percentile of
//!    all font sizes.
//! 3. If so, assign a `Heading` hint. The heading level is derived
//!    from the font-size rank: the largest distinct size seen maps to
//!    `H1`, second-largest to `H2`, up to `H6`.
//! 4. Bold text receives a confidence boost (bold headings are more
//!    reliable than size-only headings).
//!
//! # Limitations
//!
//! - Font size alone is a weak signal. Many documents use the same
//!   font size for headings and body text, relying on bold or color
//!   instead. This detector is conservative to avoid false positives.
//! - The heading level mapping assumes a monotonic relationship between
//!   font size and heading level, which is true for ~90% of documents
//!   but not universally.
//! - Short text is penalised: a single word in large font is less
//!   likely to be a heading than a full line.

mod config;
mod detect;
mod helpers;
#[cfg(test)]
mod tests;

pub use config::HeadingDetector;
