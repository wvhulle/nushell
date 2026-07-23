mod shared;

use shared::{run_diagnostics_repl, server_config};

const DEFAULT_COMMAND: &str = "nu-lint --lsp";
const DEFAULT_LANGUAGE_ID: &str = "nushell";
const DEFAULT_SCHEME: &str = "repl";

fn main() -> std::io::Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let argument_or =
        |index: usize, default: &'static str| arguments.get(index).map_or(default, String::as_str);
    let command: Vec<_> = argument_or(0, DEFAULT_COMMAND).split_whitespace().collect();
    run_diagnostics_repl(
        vec![server_config(
            &command,
            argument_or(1, DEFAULT_LANGUAGE_ID),
            argument_or(2, DEFAULT_SCHEME),
        )],
        "let x = echo hello",
    )
}
