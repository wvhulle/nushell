$env.config.show_banner = false
$env.config.history.max_size = 0
$env.PROMPT_COMMAND = {|| "demo" }
$env.PROMPT_COMMAND_RIGHT = {|| "" }
$env.config.diagnostics.servers = [
    { command: [nu-lint lsp], language_id: nushell, uri_scheme: repl }
]
$env.config.keybindings = [
    {
        name: diagnostic_fix_menu
        modifier: alt
        keycode: char_f
        mode: [emacs]
        event: { send: menu, name: diagnostic_fix_menu }
    }
]
