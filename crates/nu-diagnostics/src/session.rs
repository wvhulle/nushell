use crate::fix_entries::ServerCodeAction;
use crate::lsp_position::{byte_range_of_lsp_range, lsp_range_of_byte_range};
use crate::server_process::{
    LspServerConfig, RepaintCallback, ServerLink, document_change_message,
};
use lsp_server::{Message, Request, RequestId, Response};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, Diagnostic, ExecuteCommandParams,
    PartialResultParams, TextDocumentIdentifier, WorkDoneProgressParams,
    request::{CodeActionRequest, ExecuteCommand, Request as _},
};
use nu_utils::time::Instant;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        Arc, PoisonError, RwLock,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

const CODE_ACTION_TIMEOUT: Duration = Duration::from_millis(200);
const SERVER_COMMAND_TIMEOUT: Duration = Duration::from_millis(500);
const FIRST_SESSION_REQUEST_ID: i32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticsRequest {
    pub servers: Vec<LspServerConfig>,
    pub root_directory: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BufferSnapshot {
    pub(crate) text: String,
    pub(crate) cursor: usize,
}

struct SessionCore {
    request: DiagnosticsRequest,
    buffer_snapshot: RwLock<BufferSnapshot>,
    document_version: AtomicI32,
    next_request_id: AtomicI32,
    servers: Vec<ServerLink>,
}

impl SessionCore {
    fn send_request(
        &self,
        server: &ServerLink,
        method: &str,
        params: impl serde::Serialize,
    ) -> RequestId {
        let request_id = RequestId::from(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        server.send_message(Message::Request(Request::new(
            request_id.clone(),
            method.to_string(),
            params,
        )));
        request_id
    }
}

fn response_within(
    server: &ServerLink,
    request_id: &RequestId,
    requested_at: Instant,
    timeout: Duration,
) -> Option<Response> {
    std::iter::from_fn(|| {
        let remaining = timeout.checked_sub(requested_at.elapsed())?;
        server.responses().recv_timeout(remaining).ok()
    })
    .find(|response| response.id == *request_id)
}

fn code_action_params(server: &ServerLink, snapshot: &BufferSnapshot) -> CodeActionParams {
    let diagnostics = server.current_diagnostics();
    let byte_range = diagnostics
        .iter()
        .map(|diagnostic| byte_range_of_lsp_range(&snapshot.text, &diagnostic.range))
        .find(|byte_range| byte_range.start <= snapshot.cursor && snapshot.cursor <= byte_range.end)
        .unwrap_or(snapshot.cursor..snapshot.cursor);
    CodeActionParams {
        text_document: TextDocumentIdentifier {
            uri: server.document_uri.clone(),
        },
        range: lsp_range_of_byte_range(&snapshot.text, &byte_range),
        context: CodeActionContext {
            diagnostics,
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }
}

fn server_code_actions(server_index: usize, response: Response) -> Vec<ServerCodeAction> {
    response
        .response_result
        .ok()
        .and_then(|result| serde_json::from_value::<Vec<CodeActionOrCommand>>(result).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => Some(ServerCodeAction {
                server_index,
                action,
            }),
            CodeActionOrCommand::Command(_) => None,
        })
        .collect()
}

#[derive(Clone)]
pub struct DiagnosticsSession {
    core: Arc<SessionCore>,
}

impl DiagnosticsSession {
    #[must_use]
    pub fn start(
        request: DiagnosticsRequest,
        request_repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let request_repaint: RepaintCallback = Arc::new(request_repaint);
        let servers = request
            .servers
            .iter()
            .filter_map(|config| {
                ServerLink::start(
                    config,
                    &request.root_directory,
                    Arc::clone(&request_repaint),
                )
                .inspect_err(|start_error| {
                    log::warn!("[{}] {start_error}", config.command.join(" "));
                })
                .ok()
            })
            .collect();
        Self {
            core: Arc::new(SessionCore {
                request,
                buffer_snapshot: RwLock::default(),
                document_version: AtomicI32::new(0),
                next_request_id: AtomicI32::new(FIRST_SESSION_REQUEST_ID),
                servers,
            }),
        }
    }

    #[must_use]
    pub fn serves_request(&self, request: &DiagnosticsRequest) -> bool {
        self.core.request == *request
    }

    pub(crate) fn publish_buffer(&self, text: &str, cursor: usize) {
        let mut snapshot = self
            .core
            .buffer_snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        snapshot.cursor = cursor;
        if snapshot.text == text {
            return;
        }
        text.clone_into(&mut snapshot.text);
        drop(snapshot);
        if text.is_empty() {
            self.core
                .servers
                .iter()
                .for_each(ServerLink::clear_diagnostics);
            return;
        }
        let version = self.core.document_version.fetch_add(1, Ordering::Relaxed) + 1;
        self.core.servers.iter().for_each(|server| {
            server.send_message(document_change_message(&server.document_uri, version, text));
        });
    }

    pub(crate) fn buffer_snapshot(&self) -> BufferSnapshot {
        self.core
            .buffer_snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn current_diagnostics(&self) -> Vec<Diagnostic> {
        self.core
            .servers
            .iter()
            .flat_map(ServerLink::current_diagnostics)
            .collect()
    }

    pub(crate) fn request_code_actions(&self, snapshot: &BufferSnapshot) -> Vec<ServerCodeAction> {
        let requested_at = Instant::now();
        let request_ids: Vec<_> = self
            .core
            .servers
            .iter()
            .map(|server| {
                self.core.send_request(
                    server,
                    CodeActionRequest::METHOD,
                    code_action_params(server, snapshot),
                )
            })
            .collect();
        self.core
            .servers
            .iter()
            .zip(&request_ids)
            .enumerate()
            .flat_map(|(server_index, (server, request_id))| {
                response_within(server, request_id, requested_at, CODE_ACTION_TIMEOUT)
                    .map(|response| server_code_actions(server_index, response))
                    .unwrap_or_default()
            })
            .collect()
    }

    pub(crate) fn execute_server_command(
        &self,
        server_index: usize,
        command: String,
        arguments: Vec<Value>,
    ) {
        let Some(server) = self.core.servers.get(server_index) else {
            return;
        };
        let request_id = self.core.send_request(
            server,
            ExecuteCommand::METHOD,
            ExecuteCommandParams {
                command,
                arguments,
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        );
        let completed =
            response_within(server, &request_id, Instant::now(), SERVER_COMMAND_TIMEOUT)
                .is_some_and(|response| response.response_result.is_ok());
        if !completed {
            log::warn!("a language server command did not complete");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{CodeAction, Command};

    fn action_titled(title: &str) -> CodeActionOrCommand {
        CodeActionOrCommand::CodeAction(CodeAction {
            title: title.to_string(),
            ..CodeAction::default()
        })
    }

    #[test]
    fn code_actions_keep_their_server_and_skip_bare_commands() {
        let bare_command = CodeActionOrCommand::Command(Command {
            title: "bare".to_string(),
            command: "bare".to_string(),
            arguments: None,
        });
        let response = Response::new_ok(1.into(), vec![action_titled("fix"), bare_command]);
        let actions = server_code_actions(4, response);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].server_index, 4);
        assert_eq!(actions[0].action.title, "fix");
    }

    #[test]
    fn null_code_action_results_mean_no_actions() {
        let response = Response::new_ok(1.into(), Value::Null);
        assert!(server_code_actions(0, response).is_empty());
    }
}
