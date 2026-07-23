use crate::lsp_position::byte_range_of_lsp_range;
use lsp_types::{CodeAction, TextEdit};
use nu_ansi_term::{Style, ansi::RESET};
use reedline::Highlighter;
use serde_json::Value;
use std::ops::Range;

pub(crate) struct ServerCodeAction {
    pub(crate) server_index: usize,
    pub(crate) action: CodeAction,
}

#[derive(Clone, Debug)]
pub(crate) struct BufferEdit {
    pub(crate) byte_range: Range<usize>,
    pub(crate) original: String,
    pub(crate) original_styled: String,
    pub(crate) replacement: String,
    pub(crate) replacement_styled: String,
}

#[derive(Clone, Debug)]
pub(crate) enum FixEffect {
    BufferEdits(Vec<BufferEdit>),
    ServerCommand {
        server_index: usize,
        command: String,
        arguments: Vec<Value>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct FixEntry {
    pub(crate) title: String,
    pub(crate) effect: FixEffect,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EditedBuffer {
    pub(crate) text: String,
    pub(crate) cursor: usize,
}

fn action_text_edits(action: &CodeAction) -> Vec<TextEdit> {
    action
        .edit
        .as_ref()
        .and_then(|workspace_edit| workspace_edit.changes.as_ref())
        .and_then(|changes| changes.values().next().cloned())
        .unwrap_or_default()
}

fn struck_through_preview(original: &str, highlighter: &dyn Highlighter) -> String {
    let mut styled = highlighter.highlight(original, original.len());
    styled
        .buffer
        .iter_mut()
        .for_each(|(style, _)| *style = style.strikethrough());
    styled.render_simple()
}

fn buffer_edit(edit: TextEdit, buffer: &str, highlighter: &dyn Highlighter) -> BufferEdit {
    let byte_range = byte_range_of_lsp_range(buffer, &edit.range);
    let original = buffer
        .get(byte_range.clone())
        .unwrap_or_default()
        .to_string();
    BufferEdit {
        byte_range,
        original_styled: struck_through_preview(&original, highlighter),
        original,
        replacement_styled: highlighter
            .highlight(&edit.new_text, edit.new_text.len())
            .render_simple(),
        replacement: edit.new_text,
    }
}

fn fix_entry(
    server_action: ServerCodeAction,
    buffer: &str,
    highlighter: &dyn Highlighter,
) -> Option<FixEntry> {
    let ServerCodeAction {
        server_index,
        action,
    } = server_action;
    let edits = action_text_edits(&action);
    if !edits.is_empty() {
        return Some(FixEntry {
            title: action.title,
            effect: FixEffect::BufferEdits(
                edits
                    .into_iter()
                    .map(|edit| buffer_edit(edit, buffer, highlighter))
                    .collect(),
            ),
        });
    }
    let command = action.command?;
    Some(FixEntry {
        title: action.title,
        effect: FixEffect::ServerCommand {
            server_index,
            command: command.command,
            arguments: command.arguments.unwrap_or_default(),
        },
    })
}

pub(crate) fn fix_entries(
    server_actions: Vec<ServerCodeAction>,
    buffer: &str,
    highlighter: &dyn Highlighter,
) -> Vec<FixEntry> {
    server_actions
        .into_iter()
        .filter_map(|server_action| fix_entry(server_action, buffer, highlighter))
        .collect()
}

pub(crate) fn fix_menu_row(
    entry: &FixEntry,
    is_selected: bool,
    row_style: Style,
    use_ansi_coloring: bool,
) -> String {
    let indicator_text = if is_selected { "> " } else { "  " };
    let (indicator, title_style) = if use_ansi_coloring {
        (row_style.paint(indicator_text).to_string(), row_style)
    } else {
        (indicator_text.to_string(), Style::new())
    };
    let title_row = || {
        format!(
            "{indicator}{}{RESET}",
            title_style.paint(entry.title.as_str())
        )
    };
    let FixEffect::BufferEdits(edits) = &entry.effect else {
        return title_row();
    };
    let [edit] = edits.as_slice() else {
        return title_row();
    };
    let original = if use_ansi_coloring {
        &edit.original_styled
    } else {
        &edit.original
    };
    if edit.replacement.is_empty() {
        return format!("{indicator}{original}{RESET}");
    }
    let replacement = if use_ansi_coloring {
        &edit.replacement_styled
    } else {
        &edit.replacement
    };
    format!("{indicator}{original} → {replacement}{RESET}")
}

fn replace_byte_range(text: &mut String, byte_range: &Range<usize>, replacement: &str) {
    let in_bounds = byte_range.start <= byte_range.end
        && text.is_char_boundary(byte_range.start)
        && text.is_char_boundary(byte_range.end);
    if in_bounds {
        text.replace_range(byte_range.clone(), replacement);
    }
}

pub(crate) fn applied_buffer_edits(buffer: &str, edits: &[BufferEdit]) -> EditedBuffer {
    let mut ordered_edits: Vec<_> = edits.iter().collect();
    ordered_edits.sort_by_key(|edit| std::cmp::Reverse(edit.byte_range.start));
    let text = ordered_edits
        .iter()
        .fold(buffer.to_string(), |mut text, edit| {
            replace_byte_range(&mut text, &edit.byte_range, &edit.replacement);
            text
        });
    let cursor = ordered_edits
        .last()
        .map_or(buffer.len(), |edit| {
            edit.byte_range.start + edit.replacement.len()
        })
        .min(text.len());
    EditedBuffer { text, cursor }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Command, Position, Range as LspRange, Uri, WorkspaceEdit};
    use reedline::StyledText;
    use std::collections::HashMap;

    struct PlainHighlighter;

    impl Highlighter for PlainHighlighter {
        fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
            let mut styled = StyledText::new();
            styled.push((Style::new(), line.to_string()));
            styled
        }
    }

    fn plain_edit(byte_range: Range<usize>, replacement: &str) -> BufferEdit {
        BufferEdit {
            byte_range,
            original: String::new(),
            original_styled: String::new(),
            replacement: replacement.to_string(),
            replacement_styled: replacement.to_string(),
        }
    }

    fn action_with_edit(character_start: u32, character_end: u32, new_text: &str) -> CodeAction {
        let range = LspRange {
            start: Position::new(0, character_start),
            end: Position::new(0, character_end),
        };
        let document_uri: Uri = "repl:///repl.nu".parse().expect("static uri parses");
        CodeAction {
            title: "replace it".to_string(),
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(
                    document_uri,
                    vec![TextEdit {
                        range,
                        new_text: new_text.to_string(),
                    }],
                )])),
                ..WorkspaceEdit::default()
            }),
            ..CodeAction::default()
        }
    }

    #[test]
    fn edits_apply_from_the_end_so_earlier_offsets_stay_valid() {
        let edited = applied_buffer_edits(
            "abc def",
            &[plain_edit(0..3, "first"), plain_edit(4..7, "second")],
        );
        assert_eq!(edited.text, "first second");
    }

    #[test]
    fn cursor_lands_after_the_earliest_edit() {
        let edited = applied_buffer_edits(
            "abc def",
            &[plain_edit(4..7, "second"), plain_edit(0..3, "first")],
        );
        assert_eq!(edited.cursor, 5);
    }

    #[test]
    fn edits_inside_a_character_are_skipped_instead_of_panicking() {
        let edited = applied_buffer_edits("é", &[plain_edit(1..2, "x")]);
        assert_eq!(edited.text, "é");
    }

    #[test]
    fn edits_past_the_end_are_skipped_instead_of_panicking() {
        let edited = applied_buffer_edits("ab", &[plain_edit(5..9, "x")]);
        assert_eq!(edited.text, "ab");
    }

    #[test]
    fn non_ascii_buffers_edit_the_right_bytes() {
        let entries = fix_entries(
            vec![ServerCodeAction {
                server_index: 0,
                action: action_with_edit(2, 3, "y"),
            }],
            "é x",
            &PlainHighlighter,
        );
        let [
            FixEntry {
                effect: FixEffect::BufferEdits(edits),
                ..
            },
        ] = entries.as_slice()
        else {
            panic!("expected one edit-based fix");
        };
        assert_eq!(applied_buffer_edits("é x", edits).text, "é y");
    }

    #[test]
    fn command_actions_keep_their_server() {
        let action = CodeAction {
            title: "ignore rule".to_string(),
            command: Some(Command {
                title: "ignore rule".to_string(),
                command: "ignore".to_string(),
                arguments: None,
            }),
            ..CodeAction::default()
        };
        let entries = fix_entries(
            vec![ServerCodeAction {
                server_index: 3,
                action,
            }],
            "x",
            &PlainHighlighter,
        );
        assert!(matches!(
            entries.as_slice(),
            [FixEntry {
                effect: FixEffect::ServerCommand {
                    server_index: 3,
                    ..
                },
                ..
            }]
        ));
    }

    #[test]
    fn actions_with_no_effect_are_dropped() {
        let entries = fix_entries(
            vec![ServerCodeAction {
                server_index: 0,
                action: CodeAction::default(),
            }],
            "x",
            &PlainHighlighter,
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn single_replacements_preview_both_sides() {
        let entry = FixEntry {
            title: "replace".to_string(),
            effect: FixEffect::BufferEdits(vec![BufferEdit {
                original: "old".to_string(),
                ..plain_edit(0..3, "new")
            }]),
        };
        assert_eq!(
            fix_menu_row(&entry, true, Style::new(), false),
            format!("> old → new{RESET}")
        );
    }

    #[test]
    fn several_edits_show_the_title() {
        let entry = FixEntry {
            title: "rewrite all".to_string(),
            effect: FixEffect::BufferEdits(vec![plain_edit(0..1, "a"), plain_edit(1..2, "b")]),
        };
        assert!(fix_menu_row(&entry, false, Style::new(), false).contains("rewrite all"));
    }
}
