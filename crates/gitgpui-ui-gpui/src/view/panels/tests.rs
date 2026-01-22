use super::*;

#[test]
fn commit_details_large_file_list_is_initially_capped() {
    let total_files = 12 + 15762;
    let shown = total_files.min(COMMIT_DETAILS_FILES_INITIAL_RENDER_LIMIT);
    let omitted = total_files.saturating_sub(shown);

    assert_eq!(shown, COMMIT_DETAILS_FILES_INITIAL_RENDER_LIMIT);
    assert_eq!(omitted, total_files - COMMIT_DETAILS_FILES_INITIAL_RENDER_LIMIT);
    assert_eq!(COMMIT_DETAILS_FILES_RENDER_CHUNK, 50);
}
