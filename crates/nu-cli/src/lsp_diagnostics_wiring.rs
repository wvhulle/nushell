use crate::{NuHighlighter, reedline_config::get_style};
use nu_ansi_term::Style;
use nu_color_config::StyleComputer;
use nu_diagnostics::{
    DIAGNOSTIC_FIX_MENU_NAME, DiagnosticFixMenu, DiagnosticHighlighter, DiagnosticsRequest,
    DiagnosticsSession, DocumentScheme, FixMenuAppearance, LspServerConfig, LspStatusLine,
    SeverityStyles,
};
use nu_protocol::{
    Config, Span, Value,
    engine::{EngineState, Stack},
};
use reedline::{Reedline, ReedlineMenu};
use std::sync::Arc;

#[derive(Default)]
pub(crate) enum LspDiagnosticsState {
    #[default]
    Inactive,
    Active(DiagnosticsSession),
}

fn configured_servers(config: &Config) -> Vec<LspServerConfig> {
    config
        .diagnostics
        .servers
        .iter()
        .map(|server| LspServerConfig {
            command: server.command.clone(),
            language_id: server.language_id.clone(),
            document_scheme: DocumentScheme::from_scheme_name(&server.uri_scheme),
        })
        .collect()
}

fn severity_styles(engine_state: &EngineState, stack: &Stack, config: &Config) -> SeverityStyles {
    let style_computer = StyleComputer::from_config(engine_state, stack);
    let nothing = Value::nothing(Span::unknown());
    let configured_or = |style_name: &str, fallback: Style| {
        if config.color_config.contains_key(style_name) {
            style_computer.compute(style_name, &nothing)
        } else {
            fallback
        }
    };
    let fallback = SeverityStyles::default();
    SeverityStyles {
        error: configured_or("diagnostic_error", fallback.error),
        warning: configured_or("diagnostic_warning", fallback.warning),
        information: configured_or("diagnostic_info", fallback.information),
        hint: configured_or("diagnostic_hint", fallback.hint),
    }
}

fn fix_menu_appearance(config: &Config) -> FixMenuAppearance {
    let fallback = FixMenuAppearance::default();
    let Some(menu) = config
        .menus
        .iter()
        .find(|menu| menu.name.to_expanded_string("", config) == DIAGNOSTIC_FIX_MENU_NAME)
    else {
        return fallback;
    };
    let style_named = |style_name, fallback_style| {
        let Value::Record { val: record, .. } = &menu.style else {
            return fallback_style;
        };
        get_style(record, style_name, menu.style.span()).unwrap_or(fallback_style)
    };
    FixMenuAppearance {
        marker: menu.marker.to_expanded_string("", config),
        text_style: style_named("text", fallback.text_style),
        selected_text_style: style_named("selected_text", fallback.selected_text_style),
    }
}

fn current_session(
    diagnostics_state: &LspDiagnosticsState,
    request: DiagnosticsRequest,
    line_editor: &mut Reedline,
) -> DiagnosticsSession {
    if let LspDiagnosticsState::Active(session) = diagnostics_state
        && session.serves_request(&request)
    {
        return session.clone();
    }
    let repaint_signal = line_editor.repaint_signal();
    DiagnosticsSession::start(request, move || repaint_signal.request_repaint())
}

pub(crate) fn attach_lsp_diagnostics(
    mut line_editor: Reedline,
    diagnostics_state: &mut LspDiagnosticsState,
    engine_reference: &Arc<EngineState>,
    stack_arc: &Arc<Stack>,
) -> Reedline {
    let config = stack_arc.get_config(engine_reference);
    let servers = configured_servers(&config);
    let root_directory = engine_reference.cwd(Some(stack_arc));
    let enabled = nu_experimental::INLINE_DIAGNOSTICS.get() && !servers.is_empty();
    let (true, Ok(root_directory)) = (enabled, root_directory) else {
        *diagnostics_state = LspDiagnosticsState::Inactive;
        return line_editor;
    };
    let request = DiagnosticsRequest {
        servers,
        root_directory: root_directory.into_std_path_buf(),
    };
    let session = current_session(diagnostics_state, request, &mut line_editor);
    *diagnostics_state = LspDiagnosticsState::Active(session.clone());

    let severity_styles = severity_styles(engine_reference, stack_arc, &config);
    let nu_highlighter = || {
        Box::new(NuHighlighter::new(
            engine_reference.clone(),
            stack_arc.clone(),
        ))
    };
    line_editor
        .with_highlighter(Box::new(DiagnosticHighlighter::new(
            nu_highlighter(),
            session.clone(),
            severity_styles,
        )))
        .with_status_line(Box::new(LspStatusLine::new(
            session.clone(),
            severity_styles,
        )))
        .with_menu(ReedlineMenu::EngineCompleter(Box::new(
            DiagnosticFixMenu::new(session, nu_highlighter(), fix_menu_appearance(&config)),
        )))
}

pub(crate) fn detach_lsp_diagnostics(line_editor: Reedline) -> Reedline {
    line_editor.clear_menus().disable_status_line()
}
