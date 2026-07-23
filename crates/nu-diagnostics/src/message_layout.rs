use crate::severity_styles::{SeverityStyles, diagnostic_severity};
use lsp_types::{Diagnostic, NumberOrString};
use nu_ansi_term::Style;
use unicode_width::UnicodeWidthStr;

fn message_with_code(diagnostic: &Diagnostic) -> String {
    match &diagnostic.code {
        Some(NumberOrString::String(code)) => format!("{} [{code}]", diagnostic.message),
        Some(NumberOrString::Number(code)) => format!("{} [{code}]", diagnostic.message),
        None => diagnostic.message.clone(),
    }
}

fn wrapped_message_rows(message: &str, max_width: usize) -> Vec<String> {
    message
        .split_whitespace()
        .fold(Vec::new(), |mut rows: Vec<String>, word| {
            match rows.last_mut() {
                Some(row) if row.width() + 1 + word.width() <= max_width => {
                    row.push(' ');
                    row.push_str(word);
                }
                _ => rows.push(word.to_string()),
            }
            rows
        })
}

fn right_aligned_row(row: &str, columns: usize, style: Style, use_ansi_coloring: bool) -> String {
    let padding = " ".repeat(columns.saturating_sub(row.width()));
    if use_ansi_coloring {
        format!("{padding}{}", style.paint(row))
    } else {
        format!("{padding}{row}")
    }
}

pub(crate) fn status_text(
    diagnostics: &[Diagnostic],
    columns: usize,
    max_rows: usize,
    severity_styles: &SeverityStyles,
    use_ansi_coloring: bool,
) -> String {
    let columns = columns.max(1);
    let rows: Vec<_> = diagnostics
        .iter()
        .flat_map(|diagnostic| {
            let style = severity_styles.style_of(diagnostic_severity(diagnostic));
            wrapped_message_rows(&message_with_code(diagnostic), columns)
                .into_iter()
                .map(move |row| right_aligned_row(&row, columns, style, use_ansi_coloring))
        })
        .take(max_rows)
        .collect();
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warning_with_message(message: &str) -> Diagnostic {
        Diagnostic {
            message: message.to_string(),
            ..Diagnostic::default()
        }
    }

    #[test]
    fn long_messages_wrap_at_word_boundaries() {
        assert_eq!(
            wrapped_message_rows("alpha beta gamma", 10),
            vec!["alpha beta", "gamma"]
        );
    }

    #[test]
    fn words_wider_than_the_terminal_keep_their_own_row() {
        assert_eq!(
            wrapped_message_rows("a incomprehensibilities b", 5),
            vec!["a", "incomprehensibilities", "b"]
        );
    }

    #[test]
    fn rows_are_right_aligned_to_the_terminal_width() {
        let diagnostics = [warning_with_message("short")];
        assert_eq!(
            status_text(&diagnostics, 8, 5, &SeverityStyles::default(), false),
            "   short"
        );
    }

    #[test]
    fn output_is_capped_to_the_row_budget() {
        let diagnostics = [
            warning_with_message("first"),
            warning_with_message("second"),
            warning_with_message("third"),
        ];
        let status = status_text(&diagnostics, 10, 2, &SeverityStyles::default(), false);
        assert_eq!(status.lines().count(), 2);
        assert!(status.contains("first") && status.contains("second"));
    }

    #[test]
    fn diagnostics_keep_their_order_across_servers() {
        let diagnostics = [
            warning_with_message("server one"),
            warning_with_message("server two"),
        ];
        let status = status_text(&diagnostics, 20, 5, &SeverityStyles::default(), false);
        let rows: Vec<_> = status.lines().map(str::trim).collect();
        assert_eq!(rows, vec!["server one", "server two"]);
    }

    #[test]
    fn diagnostic_codes_follow_the_message() {
        let diagnostic = Diagnostic {
            code: Some(NumberOrString::String("redundant_echo".to_string())),
            ..warning_with_message("drop it")
        };
        assert_eq!(message_with_code(&diagnostic), "drop it [redundant_echo]");
    }

    #[test]
    fn rows_use_the_configured_severity_style() {
        let custom_style = nu_ansi_term::Color::Cyan.bold();
        let diagnostics = [Diagnostic {
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            ..warning_with_message("broken")
        }];
        let severity_styles = SeverityStyles {
            error: custom_style,
            ..SeverityStyles::default()
        };
        let status = status_text(&diagnostics, 6, 5, &severity_styles, true);
        assert_eq!(status, custom_style.paint("broken").to_string());
    }

    #[test]
    fn no_diagnostics_draw_no_status() {
        assert_eq!(
            status_text(&[], 10, 5, &SeverityStyles::default(), true),
            ""
        );
    }
}
