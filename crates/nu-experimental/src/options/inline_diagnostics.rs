use crate::*;

/// Show diagnostics and fixes from the language servers in
/// `$env.config.diagnostics.servers` under the REPL input.
///
/// Nothing runs until a server is configured. Disabling this option ignores
/// that configuration, so no language server is started.
pub static INLINE_DIAGNOSTICS: ExperimentalOption = ExperimentalOption::new(&InlineDiagnostics);

// No documentation needed here since this type isn't public.
// The static above provides all necessary details.
struct InlineDiagnostics;

impl ExperimentalOptionMarker for InlineDiagnostics {
    const IDENTIFIER: &'static str = "inline-diagnostics";
    const DESCRIPTION: &'static str =
        "Shows diagnostics and fixes from configured language servers under the REPL input.";
    const STATUS: Status = Status::OptOut;
    const SINCE: Version = (0, 116, 2);
    const ISSUE: u32 = 0;
}
