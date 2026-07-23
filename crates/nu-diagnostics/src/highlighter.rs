use crate::session::DiagnosticsSession;
use crate::severity_styles::SeverityStyles;
use crate::underline_ranges::apply_diagnostic_underlines;
use reedline::{AbbrExpandContext, AutoPairContext, Highlighter, StyledText};

pub struct DiagnosticHighlighter {
    syntax_highlighter: Box<dyn Highlighter>,
    session: DiagnosticsSession,
    severity_styles: SeverityStyles,
}

impl DiagnosticHighlighter {
    #[must_use]
    pub fn new(
        syntax_highlighter: Box<dyn Highlighter>,
        session: DiagnosticsSession,
        severity_styles: SeverityStyles,
    ) -> Self {
        Self {
            syntax_highlighter,
            session,
            severity_styles,
        }
    }
}

impl Highlighter for DiagnosticHighlighter {
    fn highlight(&self, line: &str, cursor: usize) -> StyledText {
        self.session.publish_buffer(line, cursor);
        let mut styled_text = self.syntax_highlighter.highlight(line, cursor);
        apply_diagnostic_underlines(
            &mut styled_text,
            &self.session.current_diagnostics(),
            line,
            &self.severity_styles,
        );
        styled_text
    }

    fn should_expand_abbr(&self, line: &str, cursor: usize, context: AbbrExpandContext) -> bool {
        self.syntax_highlighter
            .should_expand_abbr(line, cursor, context)
    }

    fn should_auto_pair(&self, context: &AutoPairContext<'_>) -> bool {
        self.syntax_highlighter.should_auto_pair(context)
    }

    fn hint_closer_offsets(
        &self,
        line: &str,
        cursor: usize,
        candidate: &str,
        closers: &str,
    ) -> Option<Vec<usize>> {
        self.syntax_highlighter
            .hint_closer_offsets(line, cursor, candidate, closers)
    }
}
