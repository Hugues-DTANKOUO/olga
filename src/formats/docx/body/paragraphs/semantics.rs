use std::collections::HashMap;

use crate::error::{Warning, WarningKind};
use crate::model::{HintKind, SemanticHint};

use super::super::state::{DocxWarningContext, HeadingLevelOrigin, ScopedIssueKind};
use super::super::warnings::{push_unique_warning, record_scoped_issue_summary};
use super::super::{NumberingLevel, ParagraphContext, ResolvedStyle};

pub(super) fn detect_heading_level(style: &ResolvedStyle) -> Option<(u8, HeadingLevelOrigin)> {
    if let Some(level) = style.outline_level {
        return Some((level + 1, HeadingLevelOrigin::OutlineLevel));
    }

    let name_lower = style.name.to_ascii_lowercase();
    if name_lower.starts_with("heading") || name_lower.starts_with("titre") {
        name_lower
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u8>()
            .ok()
            .map(|level| (level, HeadingLevelOrigin::StyleNameHeuristic))
    } else {
        None
    }
}

pub(super) fn resolve_docx_paragraph_container_hints(
    para: &ParagraphContext,
    styles: &HashMap<String, ResolvedStyle>,
    numbering: &HashMap<(String, u8), NumberingLevel>,
    warning_scope_key: &str,
    warning_scope: &str,
    warning_ctx: &mut DocxWarningContext<'_>,
    warn_on_degradations: bool,
) -> Vec<SemanticHint> {
    let style = para.style_id.as_ref().and_then(|id| styles.get(id));
    if warn_on_degradations
        && let Some(style_id) = &para.style_id
        && style.is_none()
    {
        push_unique_warning(
            warning_ctx.warnings,
            warning_ctx.warning_dedup_keys,
            format!("unknown_style:{warning_scope_key}:{style_id}"),
            Warning {
                kind: WarningKind::UnresolvedStyle,
                message: format!(
                    "DOCX {} paragraph references unknown style '{}'",
                    warning_scope, style_id
                ),
                page: Some(warning_ctx.current_page),
            },
        );
        record_scoped_issue_summary(
            warning_ctx.scoped_issue_summaries,
            warning_scope_key,
            warning_scope,
            warning_ctx.current_page,
            ScopedIssueKind::UnresolvedStyle,
        );
    }

    let heading_detection = style.and_then(detect_heading_level);
    if warn_on_degradations
        && let (Some(style), Some((level, HeadingLevelOrigin::StyleNameHeuristic))) =
            (style, heading_detection)
    {
        push_unique_warning(
            warning_ctx.warnings,
            warning_ctx.warning_dedup_keys,
            format!(
                "heading_style_heuristic:{warning_scope_key}:{}:{}:{level}",
                style.style_id, style.name
            ),
            Warning {
                kind: WarningKind::HeuristicInference,
                message: format!(
                    "DOCX {} paragraph style '{}' ('{}') was promoted to Heading {} via style-name heuristic because outlineLvl is absent",
                    warning_scope, style.style_id, style.name, level
                ),
                page: Some(warning_ctx.current_page),
            },
        );
        record_scoped_issue_summary(
            warning_ctx.scoped_issue_summaries,
            warning_scope_key,
            warning_scope,
            warning_ctx.current_page,
            ScopedIssueKind::HeadingStyleHeuristic,
        );
    }

    let list_info = para.num_id.as_ref().and_then(|num_id| {
        let level = para.num_level.unwrap_or(0);
        numbering.get(&(num_id.clone(), level))
    });

    let mut hints = Vec::new();
    if let Some((level, _)) = heading_detection {
        hints.push(SemanticHint::from_format(HintKind::Heading { level }));
    }

    if let Some(list) = list_info {
        hints.push(SemanticHint::from_format(HintKind::ListItem {
            depth: para.num_level.unwrap_or(0),
            ordered: list.is_ordered,
            list_group: None,
        }));
    }

    if hints.is_empty() {
        hints.push(SemanticHint::from_format(HintKind::Paragraph));
    }

    hints
}
