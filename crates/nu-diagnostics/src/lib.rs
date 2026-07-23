#![doc = include_str!("../README.md")]

mod document_uri;
mod fix_entries;
mod fix_menu;
mod highlighter;
mod lsp_position;
mod message_layout;
mod server_process;
mod session;
mod severity_styles;
mod status_line;
mod underline_ranges;

pub use document_uri::DocumentScheme;
pub use fix_menu::{DIAGNOSTIC_FIX_MENU_NAME, DiagnosticFixMenu, FixMenuAppearance};
pub use highlighter::DiagnosticHighlighter;
pub use server_process::LspServerConfig;
pub use session::{DiagnosticsRequest, DiagnosticsSession};
pub use severity_styles::SeverityStyles;
pub use status_line::LspStatusLine;
