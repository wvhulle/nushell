use crate::message_layout::status_text;
use crate::session::DiagnosticsSession;
use crate::severity_styles::SeverityStyles;
use reedline::StatusLine;

const FALLBACK_TERMINAL_ROWS: u16 = 24;

fn status_row_budget() -> usize {
    let (_, terminal_rows) = crossterm::terminal::size().unwrap_or((0, FALLBACK_TERMINAL_ROWS));
    usize::from(terminal_rows / 2).max(1)
}

pub struct LspStatusLine {
    session: DiagnosticsSession,
    severity_styles: SeverityStyles,
}

impl LspStatusLine {
    #[must_use]
    pub fn new(session: DiagnosticsSession, severity_styles: SeverityStyles) -> Self {
        Self {
            session,
            severity_styles,
        }
    }
}

impl StatusLine for LspStatusLine {
    fn status(
        &mut self,
        line: &str,
        _cursor: usize,
        columns: u16,
        use_ansi_coloring: bool,
    ) -> String {
        if line.is_empty() {
            return String::new();
        }
        status_text(
            &self.session.current_diagnostics(),
            usize::from(columns),
            status_row_budget(),
            &self.severity_styles,
            use_ansi_coloring,
        )
    }
}
