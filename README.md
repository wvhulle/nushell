# nu-inline-diagnostics

Fork of [Nushell](https://github.com/nushell/nushell) that shows LSP diagnostics inline in the REPL as you type. See [`crates/nu-diagnostics`](./crates/nu-diagnostics/README.md) for features, recordings, usage and architecture.

> The installed binary is still called `nu`. `nu-inline-diagnostics` is only the package/flake name.

_Disclaimer: This fork was made to make it easier for people to experiment with inline diagnostics in the terminal for Nushell. As far as I know this is not too common yet. For the most-up-to date mainline Nushell features and development, please refer to the original repository. Otherwise, go ahead and play around with this fork._

## Installation

Install a language protocol server (LSP server). I recommend `nu-lint`:

```bash
cargo install nu-lint
```

Build the main binary `nu` with a Rust compiler toolchain:

- For Nix users: `nix run .` (or add this repo's flake to your system dependencies)
- Otherwise, if you are on a more mainstream machine, install `rustup` and:

```bash
cargo install --git https://github.com/wvhulle/nushell
```

Keep in mind this will overwrite any other `nu` binary you have there (such as the official one).

Then configure a language server as described in [usage](./crates/nu-diagnostics/README.md#usage).

You can file issues related to incorrect lints and fixes at [nu-lint](https://codeberg.org/wvhulle/nu-lint).
