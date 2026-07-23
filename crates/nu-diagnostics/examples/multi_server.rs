mod shared;

use shared::{run_diagnostics_repl, server_config};

fn main() -> std::io::Result<()> {
    run_diagnostics_repl(
        vec![
            server_config(&["nu-lint", "--lsp"], "nushell", "repl"),
            server_config(&["ast-grep", "lsp"], "bash", "file"),
        ],
        "let x = echo hello; git status",
    )
}
