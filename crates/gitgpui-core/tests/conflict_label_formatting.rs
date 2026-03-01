use gitgpui_core::conflict_labels::{BaseLabelScenario, format_base_label};
use std::path::PathBuf;

#[test]
fn label_no_base() {
    let label = format_base_label(&BaseLabelScenario::NoBase);
    assert_eq!(label, "empty tree");
}

#[test]
fn label_unique_base() {
    let label = format_base_label(&BaseLabelScenario::UniqueBase {
        commit_id: "0123456789abcdef".to_string(),
        path: PathBuf::from("src/lib.rs"),
    });
    assert_eq!(label, "0123456:src/lib.rs");
}

#[test]
fn label_unique_base_rename() {
    let label = format_base_label(&BaseLabelScenario::UniqueBaseRename {
        commit_id: "abcdef0123456789".to_string(),
        original_path: PathBuf::from("old/name.txt"),
    });
    assert_eq!(label, "abcdef0:old/name.txt");
}

#[test]
fn label_merged_ancestors() {
    let label = format_base_label(&BaseLabelScenario::MergedCommonAncestors {
        path: PathBuf::from("docs/guide.md"),
    });
    assert_eq!(label, "merged common ancestors:docs/guide.md");
}

#[test]
fn label_rebase_parent() {
    let label = format_base_label(&BaseLabelScenario::RebaseParent {
        description: "HEAD~3".to_string(),
    });
    assert_eq!(label, "parent of HEAD~3");
}
