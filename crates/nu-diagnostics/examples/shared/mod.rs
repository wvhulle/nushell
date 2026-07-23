use nu_diagnostics::{
    DIAGNOSTIC_FIX_MENU_NAME, DiagnosticFixMenu, DiagnosticHighlighter, DiagnosticsRequest,
    DiagnosticsSession, DocumentScheme, FixMenuAppearance, LspServerConfig, LspStatusLine,
    SeverityStyles,
};
use reedline::{
    DefaultPrompt, DefaultPromptSegment, Emacs, ExampleHighlighter, KeyCode, KeyModifiers,
    Reedline, ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};
use std::{env, io};

pub fn server_config(command: &[&str], language_id: &str, scheme_name: &str) -> LspServerConfig {
    LspServerConfig {
        command: command.iter().map(ToString::to_string).collect(),
        language_id: language_id.to_string(),
        document_scheme: DocumentScheme::from_scheme_name(scheme_name),
    }
}

fn fix_menu_keybindings() -> Emacs {
    let open_fix_menu = || ReedlineEvent::Menu(DIAGNOSTIC_FIX_MENU_NAME.to_string());
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(KeyModifiers::CONTROL, KeyCode::Char('.'), open_fix_menu());
    keybindings.add_binding(KeyModifiers::ALT, KeyCode::Char('f'), open_fix_menu());
    keybindings.add_binding(KeyModifiers::NONE, KeyCode::Tab, ReedlineEvent::MenuNext);
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );
    Emacs::new(keybindings)
}

fn syntax_highlighter() -> Box<ExampleHighlighter> {
    Box::new(ExampleHighlighter::new(vec![
        "echo".to_string(),
        "let".to_string(),
        "print".to_string(),
    ]))
}

pub fn run_diagnostics_repl(servers: Vec<LspServerConfig>, try_input: &str) -> io::Result<()> {
    let mut line_editor = Reedline::create()
        .use_kitty_keyboard_enhancement(true)
        .with_edit_mode(Box::new(fix_menu_keybindings()));
    let repaint_signal = line_editor.repaint_signal();
    let session = DiagnosticsSession::start(
        DiagnosticsRequest {
            servers,
            root_directory: env::current_dir()?,
        },
        move || repaint_signal.request_repaint(),
    );
    let mut line_editor = line_editor
        .with_highlighter(Box::new(DiagnosticHighlighter::new(
            syntax_highlighter(),
            session.clone(),
            SeverityStyles::default(),
        )))
        .with_status_line(Box::new(LspStatusLine::new(
            session.clone(),
            SeverityStyles::default(),
        )))
        .with_menu(ReedlineMenu::EngineCompleter(Box::new(
            DiagnosticFixMenu::new(session, syntax_highlighter(), FixMenuAppearance::default()),
        )));
    let prompt = DefaultPrompt::new(
        DefaultPromptSegment::Basic("diagnostics".to_string()),
        DefaultPromptSegment::Empty,
    );

    println!("Try typing: {try_input}");
    println!("Fix menu: ctrl+. or alt+f, tab to move, enter to apply. Leave with ctrl+d.");
    loop {
        match line_editor.read_line(&prompt)? {
            Signal::Success(submitted_line) => println!("submitted: {submitted_line}"),
            Signal::CtrlD => return Ok(()),
            _ => {}
        }
    }
}
