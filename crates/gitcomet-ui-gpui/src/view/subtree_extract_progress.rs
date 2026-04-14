use super::color::with_alpha;
use crate::theme::AppTheme;
use gitcomet_state::model::{
    SubtreeExtractOpState, SubtreeExtractOpStatus, SubtreeExtractProgressStage,
};

pub(crate) fn subtree_extract_progress_title(op: &SubtreeExtractOpState) -> &'static str {
    if op.destination_repo.is_some() {
        "Extracting subtree…"
    } else {
        "Splitting subtree…"
    }
}

pub(crate) fn subtree_extract_progress_phase_label(op: &SubtreeExtractOpState) -> &'static str {
    match op.progress.stage {
        SubtreeExtractProgressStage::Splitting => "Splitting history",
        SubtreeExtractProgressStage::PreparingDestination => "Preparing destination",
        SubtreeExtractProgressStage::PublishingDestination => "Publishing destination",
    }
}

pub(crate) fn subtree_extract_progress_color(
    theme: AppTheme,
    op: &SubtreeExtractOpState,
) -> gpui::Rgba {
    match op.status {
        SubtreeExtractOpStatus::FinishedErr(_) => {
            with_alpha(theme.colors.text, if theme.is_dark { 0.78 } else { 0.62 })
        }
        _ => match op.progress.stage {
            SubtreeExtractProgressStage::Splitting => {
                with_alpha(theme.colors.text, if theme.is_dark { 0.42 } else { 0.34 })
            }
            SubtreeExtractProgressStage::PreparingDestination => {
                with_alpha(theme.colors.text, if theme.is_dark { 0.62 } else { 0.52 })
            }
            SubtreeExtractProgressStage::PublishingDestination => {
                with_alpha(theme.colors.text, if theme.is_dark { 0.78 } else { 0.62 })
            }
        },
    }
}

pub(crate) fn subtree_extract_progress_fill_ratio(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

pub(crate) fn subtree_extract_progress_target_label(op: &SubtreeExtractOpState) -> String {
    op.destination_repo
        .as_deref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| op.path.display().to_string())
}
