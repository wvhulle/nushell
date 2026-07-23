use crate::lsp_position::byte_range_of_lsp_range;
use crate::severity_styles::{SeverityStyles, diagnostic_severity};
use lsp_types::{Diagnostic, DiagnosticSeverity};
use nu_ansi_term::Color;
use reedline::StyledText;

fn severity_rank(severity: DiagnosticSeverity) -> u32 {
    match severity {
        DiagnosticSeverity::ERROR => 1,
        DiagnosticSeverity::WARNING => 2,
        DiagnosticSeverity::INFORMATION => 3,
        _ => 4,
    }
}

pub(crate) fn apply_diagnostic_underlines(
    styled_text: &mut StyledText,
    diagnostics: &[Diagnostic],
    buffer: &str,
    severity_styles: &SeverityStyles,
) {
    diagnostics.iter().for_each(|diagnostic| {
        let byte_range = byte_range_of_lsp_range(buffer, &diagnostic.range);
        if byte_range.start < byte_range.end {
            let severity = diagnostic_severity(diagnostic);
            styled_text.set_underline_color_range(
                byte_range.start,
                byte_range.end,
                severity_styles
                    .style_of(severity)
                    .foreground
                    .unwrap_or(Color::Default),
                severity_rank(severity),
            );
        }
    });
}
