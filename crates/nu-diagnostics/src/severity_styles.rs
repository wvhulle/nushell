use lsp_types::{Diagnostic, DiagnosticSeverity};
use nu_ansi_term::{Color, Style};

#[derive(Clone, Copy, Debug)]
pub struct SeverityStyles {
    pub error: Style,
    pub warning: Style,
    pub information: Style,
    pub hint: Style,
}

impl Default for SeverityStyles {
    fn default() -> Self {
        Self {
            error: Color::Red.normal(),
            warning: Color::Yellow.normal(),
            information: Color::Blue.normal(),
            hint: Color::DarkGray.normal(),
        }
    }
}

impl SeverityStyles {
    pub(crate) fn style_of(&self, severity: DiagnosticSeverity) -> Style {
        match severity {
            DiagnosticSeverity::ERROR => self.error,
            DiagnosticSeverity::WARNING => self.warning,
            DiagnosticSeverity::INFORMATION => self.information,
            _ => self.hint,
        }
    }
}

pub(crate) fn diagnostic_severity(diagnostic: &Diagnostic) -> DiagnosticSeverity {
    diagnostic.severity.unwrap_or(DiagnosticSeverity::WARNING)
}
