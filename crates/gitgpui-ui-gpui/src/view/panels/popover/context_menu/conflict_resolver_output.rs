use super::*;

pub(super) fn model(
    cursor_line: usize,
    selected_text: &Option<String>,
    has_source_a: bool,
    has_source_b: bool,
    has_source_c: bool,
    is_three_way: bool,
) -> ContextMenuModel {
    let has_selection = selected_text.is_some();
    let copy_text = selected_text.clone().unwrap_or_default();
    let cut_text = copy_text.clone();

    let line_label = cursor_line + 1; // 1-based display

    let (label_a, label_b, label_c): (&str, &str, &str) = if is_three_way {
        ("Base", "Ours", "Theirs")
    } else {
        ("Ours", "Theirs", "")
    };

    let mut items = vec![
        ContextMenuItem::Entry {
            label: "Copy".into(),
            icon: None,
            shortcut: Some("Ctrl+C".into()),
            disabled: !has_selection,
            action: Box::new(ContextMenuAction::CopyText { text: copy_text }),
        },
        ContextMenuItem::Entry {
            label: "Cut".into(),
            icon: None,
            shortcut: Some("Ctrl+X".into()),
            disabled: !has_selection,
            action: Box::new(ContextMenuAction::ConflictResolverOutputCut { text: cut_text }),
        },
        ContextMenuItem::Entry {
            label: "Paste".into(),
            icon: None,
            shortcut: Some("Ctrl+V".into()),
            disabled: false,
            action: Box::new(ContextMenuAction::ConflictResolverOutputPaste),
        },
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: format!("Pick line {line_label} from {label_a}").into(),
            icon: Some("A".into()),
            shortcut: None,
            disabled: !has_source_a,
            action: Box::new(ContextMenuAction::ConflictResolverOutputPickLine {
                line_ix: cursor_line,
                choice: if is_three_way {
                    conflict_resolver::ConflictChoice::Base
                } else {
                    conflict_resolver::ConflictChoice::Ours
                },
            }),
        },
        ContextMenuItem::Entry {
            label: format!("Pick line {line_label} from {label_b}").into(),
            icon: Some("B".into()),
            shortcut: None,
            disabled: !has_source_b,
            action: Box::new(ContextMenuAction::ConflictResolverOutputPickLine {
                line_ix: cursor_line,
                choice: if is_three_way {
                    conflict_resolver::ConflictChoice::Ours
                } else {
                    conflict_resolver::ConflictChoice::Theirs
                },
            }),
        },
    ];

    if is_three_way {
        items.push(ContextMenuItem::Entry {
            label: format!("Pick line {line_label} from {label_c}").into(),
            icon: Some("C".into()),
            shortcut: None,
            disabled: !has_source_c,
            action: Box::new(ContextMenuAction::ConflictResolverOutputPickLine {
                line_ix: cursor_line,
                choice: conflict_resolver::ConflictChoice::Theirs,
            }),
        });
    }

    ContextMenuModel::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_builds_all_entries_three_way() {
        let selected = Some("hello".to_string());
        let m = model(2, &selected, true, true, true, true);

        // Copy, Cut, Paste, Separator, Pick A, Pick B, Pick C
        assert_eq!(m.items.len(), 7);

        // Copy should be enabled with selection
        match &m.items[0] {
            ContextMenuItem::Entry {
                label, disabled, ..
            } => {
                assert_eq!(label.as_ref(), "Copy");
                assert!(!*disabled);
            }
            _ => panic!("expected Copy entry"),
        }

        // Pick line 3 from Base (A in three-way)
        match &m.items[4] {
            ContextMenuItem::Entry {
                label,
                disabled,
                icon,
                ..
            } => {
                assert_eq!(label.as_ref(), "Pick line 3 from Base");
                assert_eq!(icon.as_ref().map(|s| s.as_ref()), Some("A"));
                assert!(!*disabled);
            }
            _ => panic!("expected Pick A entry"),
        }

        // Pick line 3 from Theirs (C in three-way)
        match &m.items[6] {
            ContextMenuItem::Entry {
                label,
                disabled,
                icon,
                ..
            } => {
                assert_eq!(label.as_ref(), "Pick line 3 from Theirs");
                assert_eq!(icon.as_ref().map(|s| s.as_ref()), Some("C"));
                assert!(!*disabled);
            }
            _ => panic!("expected Pick C entry"),
        }
    }

    #[test]
    fn model_two_way_omits_pick_c() {
        let m = model(0, &None, true, true, false, false);

        // Copy, Cut, Paste, Separator, Pick A, Pick B (no Pick C)
        assert_eq!(m.items.len(), 6);

        // Copy/Cut disabled without selection
        match &m.items[0] {
            ContextMenuItem::Entry { disabled, .. } => assert!(*disabled),
            _ => panic!("expected entry"),
        }
        match &m.items[1] {
            ContextMenuItem::Entry { disabled, .. } => assert!(*disabled),
            _ => panic!("expected entry"),
        }

        // Pick A label in two-way uses "Ours"
        match &m.items[4] {
            ContextMenuItem::Entry { label, .. } => {
                assert_eq!(label.as_ref(), "Pick line 1 from Ours");
            }
            _ => panic!("expected Pick A entry"),
        }

        // Pick B label in two-way uses "Theirs"
        match &m.items[5] {
            ContextMenuItem::Entry { label, .. } => {
                assert_eq!(label.as_ref(), "Pick line 1 from Theirs");
            }
            _ => panic!("expected Pick B entry"),
        }
    }

    #[test]
    fn pick_disabled_when_source_unavailable() {
        let m = model(5, &None, false, true, false, true);

        // Pick A disabled
        match &m.items[4] {
            ContextMenuItem::Entry { disabled, .. } => assert!(*disabled),
            _ => panic!("expected entry"),
        }
        // Pick B enabled
        match &m.items[5] {
            ContextMenuItem::Entry { disabled, .. } => assert!(!*disabled),
            _ => panic!("expected entry"),
        }
        // Pick C disabled
        match &m.items[6] {
            ContextMenuItem::Entry { disabled, .. } => assert!(*disabled),
            _ => panic!("expected entry"),
        }
    }
}
