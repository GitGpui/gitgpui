use super::*;

pub(super) fn model(host: &PopoverHost) -> ContextMenuModel {
    model_for_whitespace_mode(host.diff_whitespace_mode)
}

fn model_for_whitespace_mode(mode: DiffWhitespaceMode) -> ContextMenuModel {
    let show_whitespace = mode == DiffWhitespaceMode::Show;
    let next_mode = mode.toggled();

    ContextMenuModel::new(vec![
        ContextMenuItem::Header("Diff actions".into()),
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: "Show whitespace changes".into(),
            icon: show_whitespace.then_some("icons/check.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::SetDiffWhitespaceMode { mode: next_mode }),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_toggles_whitespace_mode() {
        let model = model_for_whitespace_mode(DiffWhitespaceMode::Show);

        assert!(model.items.iter().any(|item| {
            matches!(
                item,
                ContextMenuItem::Entry {
                    label,
                    icon,
                    action,
                    ..
                } if label.as_ref() == "Show whitespace changes"
                    && icon
                        .as_ref()
                        .is_some_and(|icon| icon.as_ref() == "icons/check.svg")
                    && matches!(
                        action.as_ref(),
                        ContextMenuAction::SetDiffWhitespaceMode {
                            mode: DiffWhitespaceMode::Ignore
                        }
                    )
            )
        }));

        let model = model_for_whitespace_mode(DiffWhitespaceMode::Ignore);
        assert!(model.items.iter().any(|item| {
            matches!(
                item,
                ContextMenuItem::Entry {
                    label,
                    icon,
                    action,
                    ..
                } if label.as_ref() == "Show whitespace changes"
                    && icon.is_none()
                    && matches!(
                        action.as_ref(),
                        ContextMenuAction::SetDiffWhitespaceMode {
                            mode: DiffWhitespaceMode::Show
                        }
                    )
            )
        }));
    }
}
