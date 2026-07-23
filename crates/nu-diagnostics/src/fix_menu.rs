use crate::fix_entries::{FixEffect, FixEntry, applied_buffer_edits, fix_entries, fix_menu_row};
use crate::session::DiagnosticsSession;
use nu_ansi_term::Style;
use reedline::{
    Completer, Editor, Highlighter, Menu, MenuEvent, MenuSettings, Painter, Suggestion,
    UndoBehavior,
};

pub const DIAGNOSTIC_FIX_MENU_NAME: &str = "diagnostic_fix_menu";
const VISIBLE_FIX_ROWS: usize = 10;

#[derive(Clone, Debug)]
pub struct FixMenuAppearance {
    pub marker: String,
    pub text_style: Style,
    pub selected_text_style: Style,
}

impl Default for FixMenuAppearance {
    fn default() -> Self {
        Self {
            marker: "| ".to_string(),
            text_style: Style::new().italic(),
            selected_text_style: Style::new().italic().bold(),
        }
    }
}

pub struct DiagnosticFixMenu {
    settings: MenuSettings,
    text_style: Style,
    selected_text_style: Style,
    session: DiagnosticsSession,
    preview_highlighter: Box<dyn Highlighter>,
    is_active: bool,
    entries: Vec<FixEntry>,
    entry_values: Vec<Suggestion>,
    entries_buffer_text: String,
    selected_index: usize,
    scroll_offset: usize,
}

impl DiagnosticFixMenu {
    #[must_use]
    pub fn new(
        session: DiagnosticsSession,
        preview_highlighter: Box<dyn Highlighter>,
        appearance: FixMenuAppearance,
    ) -> Self {
        Self {
            settings: MenuSettings::default()
                .with_name(DIAGNOSTIC_FIX_MENU_NAME)
                .with_marker(&appearance.marker),
            text_style: appearance.text_style,
            selected_text_style: appearance.selected_text_style,
            session,
            preview_highlighter,
            is_active: false,
            entries: Vec::new(),
            entry_values: Vec::new(),
            entries_buffer_text: String::new(),
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    fn open_with_current_fixes(&mut self) {
        let snapshot = self.session.buffer_snapshot();
        let server_actions = self.session.request_code_actions(&snapshot);
        self.entries = fix_entries(
            server_actions,
            &snapshot.text,
            self.preview_highlighter.as_ref(),
        );
        self.entry_values = self
            .entries
            .iter()
            .map(|entry| Suggestion {
                value: entry.title.clone(),
                ..Suggestion::default()
            })
            .collect();
        self.entries_buffer_text = snapshot.text;
        self.reset_position();
        self.is_active = !self.entries.is_empty();
    }

    fn select_index(&mut self, selected_index: usize) {
        self.selected_index = selected_index;
        self.scroll_offset = self
            .scroll_offset
            .min(selected_index)
            .max((selected_index + 1).saturating_sub(VISIBLE_FIX_ROWS));
    }

    fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.select_index((self.selected_index + 1) % self.entries.len());
        }
    }

    fn select_previous(&mut self) {
        if !self.entries.is_empty() {
            let entry_count = self.entries.len();
            self.select_index((self.selected_index + entry_count - 1) % entry_count);
        }
    }
}

impl Menu for DiagnosticFixMenu {
    fn settings(&self) -> &MenuSettings {
        &self.settings
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    fn clear_input(&mut self) {}

    fn menu_event(&mut self, event: MenuEvent) {
        match event {
            MenuEvent::Activate(_) => self.open_with_current_fixes(),
            MenuEvent::Deactivate | MenuEvent::Edit(_) => self.is_active = false,
            MenuEvent::NextElement | MenuEvent::MoveDown => self.select_next(),
            MenuEvent::PreviousElement | MenuEvent::MoveUp => self.select_previous(),
            _ => {}
        }
    }

    fn can_quick_complete(&self) -> bool {
        false
    }

    fn can_partially_complete(
        &mut self,
        _values_updated: bool,
        _editor: &mut Editor,
        _completer: &mut dyn Completer,
    ) -> bool {
        false
    }

    fn update_values(&mut self, _editor: &mut Editor, _completer: &mut dyn Completer) {}

    fn reset_position(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn update_working_details(
        &mut self,
        _editor: &mut Editor,
        _completer: &mut dyn Completer,
        _painter: &Painter,
    ) {
    }

    fn replace_in_buffer(&self, editor: &mut Editor) {
        let Some(entry) = self.entries.get(self.selected_index) else {
            return;
        };
        match &entry.effect {
            FixEffect::BufferEdits(edits) => {
                if editor.get_buffer() != self.entries_buffer_text {
                    return;
                }
                let edited = applied_buffer_edits(&self.entries_buffer_text, edits);
                editor.edit_buffer(
                    |line_buffer| {
                        line_buffer.set_buffer(edited.text);
                        line_buffer.set_insertion_point(edited.cursor);
                    },
                    UndoBehavior::CreateUndoPoint,
                );
            }
            FixEffect::ServerCommand {
                server_index,
                command,
                arguments,
            } => {
                self.session.execute_server_command(
                    *server_index,
                    command.clone(),
                    arguments.clone(),
                );
            }
        }
    }

    fn menu_required_lines(&self, _terminal_columns: u16) -> u16 {
        u16::try_from(self.entries.len().min(VISIBLE_FIX_ROWS)).unwrap_or(u16::MAX)
    }

    fn menu_string(&self, available_lines: u16, use_ansi_coloring: bool) -> String {
        let visible_rows = usize::from(available_lines).min(VISIBLE_FIX_ROWS);
        let rows: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(visible_rows)
            .map(|(index, entry)| {
                let is_selected = index == self.selected_index;
                let row_style = if is_selected {
                    self.selected_text_style
                } else {
                    self.text_style
                };
                fix_menu_row(entry, is_selected, row_style, use_ansi_coloring)
            })
            .collect();
        rows.join("\r\n")
    }

    fn min_rows(&self) -> u16 {
        self.menu_required_lines(0)
    }

    fn get_values(&self) -> &[Suggestion] {
        &self.entry_values
    }
}
