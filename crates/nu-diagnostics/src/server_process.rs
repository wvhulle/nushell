use crate::document_uri::{DocumentScheme, file_uri, repl_document_uri};
use crossbeam_channel::{Receiver, Sender, unbounded};
use lsp_server::{ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    ClientInfo, Diagnostic, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializedParams, PublishDiagnosticsParams, TextDocumentContentChangeEvent,
    TextDocumentItem, Uri, VersionedTextDocumentIdentifier, WorkspaceFolder,
    notification::{
        DidChangeTextDocument, DidOpenTextDocument, Initialized, Notification as _,
        PublishDiagnostics,
    },
    request::{Initialize, RegisterCapability, Request as _, UnregisterCapability},
};
use std::{
    fmt,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, PoisonError, RwLock},
    thread,
};

pub(crate) type RepaintCallback = Arc<dyn Fn() + Send + Sync>;

const INITIALIZE_REQUEST_ID: i32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LspServerConfig {
    pub command: Vec<String>,
    pub language_id: String,
    pub document_scheme: DocumentScheme,
}

#[derive(Debug)]
pub(crate) enum ServerStartError {
    EmptyCommand,
    InvalidDocumentUri,
    MissingStandardStream,
    Io(io::Error),
}

impl fmt::Display for ServerStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => formatter.write_str("the server command is empty"),
            Self::InvalidDocumentUri => {
                formatter.write_str("no valid document URI for this scheme and directory")
            }
            Self::MissingStandardStream => {
                formatter.write_str("the server process has no stdin or stdout")
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<io::Error> for ServerStartError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

struct KilledOnDrop {
    child: Child,
}

impl Drop for KilledOnDrop {
    fn drop(&mut self) {
        if self.child.kill().is_ok() {
            let _ = self.child.wait();
        }
    }
}

pub(crate) struct ServerLink {
    pub(crate) document_uri: Uri,
    outgoing_messages: Sender<Message>,
    responses: Receiver<Response>,
    diagnostics: Arc<RwLock<Vec<Diagnostic>>>,
    _process: KilledOnDrop,
}

impl ServerLink {
    pub(crate) fn start(
        config: &LspServerConfig,
        root_directory: &Path,
        request_repaint: RepaintCallback,
    ) -> Result<Self, ServerStartError> {
        let document_uri =
            repl_document_uri(&config.document_scheme, &config.language_id, root_directory)
                .ok_or(ServerStartError::InvalidDocumentUri)?;
        let (program, arguments) = config
            .command
            .split_first()
            .ok_or(ServerStartError::EmptyCommand)?;
        let mut child = Command::new(program)
            .args(arguments)
            .current_dir(root_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let process_streams = (child.stdin.take(), child.stdout.take());
        let process = KilledOnDrop { child };
        let (Some(server_input), Some(server_output)) = process_streams else {
            return Err(ServerStartError::MissingStandardStream);
        };

        let (outgoing_messages, pending_messages) = unbounded();
        let (response_sender, responses) = unbounded();
        let diagnostics = Arc::default();
        let connection = ServerConnection {
            server_input,
            server_output: BufReader::new(server_output),
            pending_messages,
            handshake: handshake_messages(config, &document_uri, root_directory),
            incoming: IncomingRouter {
                outgoing_messages: outgoing_messages.clone(),
                response_sender,
                diagnostics: Arc::clone(&diagnostics),
                request_repaint,
            },
        };
        let command_name = program.clone();
        thread::Builder::new()
            .name("nu-diagnostics-server".to_string())
            .spawn(move || {
                if let Err(error) = connection.run() {
                    log::warn!("[{command_name}] {error}");
                }
            })?;
        Ok(Self {
            document_uri,
            outgoing_messages,
            responses,
            diagnostics,
            _process: process,
        })
    }

    pub(crate) fn send_message(&self, message: Message) {
        let _ = self.outgoing_messages.send(message);
    }

    pub(crate) fn responses(&self) -> &Receiver<Response> {
        &self.responses
    }

    pub(crate) fn current_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn clear_diagnostics(&self) {
        store_diagnostics(&self.diagnostics, Vec::new());
    }
}

fn store_diagnostics(shared_diagnostics: &RwLock<Vec<Diagnostic>>, diagnostics: Vec<Diagnostic>) {
    *shared_diagnostics
        .write()
        .unwrap_or_else(PoisonError::into_inner) = diagnostics;
}

struct Handshake {
    initialize: Request,
    initialized: Notification,
    open_document: Notification,
}

fn handshake_messages(
    config: &LspServerConfig,
    document_uri: &Uri,
    root_directory: &Path,
) -> Handshake {
    let root_uri = file_uri(root_directory);
    #[allow(deprecated, reason = "older servers only read root_uri")]
    let initialize_params = InitializeParams {
        process_id: Some(std::process::id()),
        client_info: Some(ClientInfo {
            name: "nu-diagnostics".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        root_uri: root_uri.clone(),
        workspace_folders: root_uri.map(|uri| {
            vec![WorkspaceFolder {
                uri,
                name: "repl".to_string(),
            }]
        }),
        ..InitializeParams::default()
    };
    Handshake {
        initialize: Request::new(
            INITIALIZE_REQUEST_ID.into(),
            Initialize::METHOD.to_string(),
            initialize_params,
        ),
        initialized: Notification::new(Initialized::METHOD.to_string(), InitializedParams {}),
        open_document: Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: document_uri.clone(),
                    language_id: config.language_id.clone(),
                    version: 0,
                    text: String::new(),
                },
            },
        ),
    }
}

pub(crate) fn document_change_message(document_uri: &Uri, version: i32, text: &str) -> Message {
    Message::Notification(Notification::new(
        DidChangeTextDocument::METHOD.to_string(),
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: document_uri.clone(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        },
    ))
}

fn is_document_change(message: &Message) -> bool {
    matches!(
        message,
        Message::Notification(notification)
            if notification.method == DidChangeTextDocument::METHOD
    )
}

fn without_superseded_changes(messages: Vec<Message>) -> impl Iterator<Item = Message> {
    let latest_change = messages.iter().rposition(is_document_change);
    messages
        .into_iter()
        .enumerate()
        .filter(move |(index, message)| {
            !is_document_change(message) || Some(*index) == latest_change
        })
        .map(|(_, message)| message)
}

struct IncomingRouter {
    outgoing_messages: Sender<Message>,
    response_sender: Sender<Response>,
    diagnostics: Arc<RwLock<Vec<Diagnostic>>>,
    request_repaint: RepaintCallback,
}

impl IncomingRouter {
    fn route(&self, message: Message) {
        match message {
            Message::Response(response) => {
                let _ = self.response_sender.send(response);
            }
            Message::Notification(notification) => self.route_notification(notification),
            Message::Request(request) => {
                let _ = self
                    .outgoing_messages
                    .send(Message::Response(server_request_reply(request)));
            }
        }
    }

    fn route_notification(&self, notification: Notification) {
        if notification.method != PublishDiagnostics::METHOD {
            return;
        }
        let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
        else {
            return;
        };
        store_diagnostics(&self.diagnostics, params.diagnostics);
        (self.request_repaint)();
    }

    fn read_until_closed(self, mut server_output: impl BufRead) {
        std::iter::from_fn(|| Message::read(&mut server_output).ok().flatten())
            .for_each(|message| self.route(message));
        store_diagnostics(&self.diagnostics, Vec::new());
        (self.request_repaint)();
    }
}

fn server_request_reply(request: Request) -> Response {
    let Request { id, method, .. } = request;
    if method == RegisterCapability::METHOD || method == UnregisterCapability::METHOD {
        Response::new_ok(id, serde_json::Value::Null)
    } else {
        Response::new_err(
            id,
            ErrorCode::MethodNotFound as i32,
            format!("unsupported request: {method}"),
        )
    }
}

fn read_until_initialized(
    server_output: &mut impl BufRead,
    server_input: &mut impl Write,
) -> io::Result<()> {
    let initialize_id = RequestId::from(INITIALIZE_REQUEST_ID);
    loop {
        match Message::read(server_output)? {
            Some(Message::Response(response)) if response.id == initialize_id => return Ok(()),
            Some(Message::Request(request)) => {
                Message::Response(server_request_reply(request)).write(server_input)?;
            }
            Some(_) => {}
            None => return Err(io::ErrorKind::UnexpectedEof.into()),
        }
    }
}

struct ServerConnection {
    server_input: ChildStdin,
    server_output: BufReader<ChildStdout>,
    pending_messages: Receiver<Message>,
    handshake: Handshake,
    incoming: IncomingRouter,
}

impl ServerConnection {
    fn run(self) -> io::Result<()> {
        let Self {
            mut server_input,
            mut server_output,
            pending_messages,
            handshake,
            incoming,
        } = self;
        Message::Request(handshake.initialize).write(&mut server_input)?;
        read_until_initialized(&mut server_output, &mut server_input)?;
        Message::Notification(handshake.initialized).write(&mut server_input)?;
        Message::Notification(handshake.open_document).write(&mut server_input)?;

        thread::Builder::new()
            .name("nu-diagnostics-reader".to_string())
            .spawn(move || incoming.read_until_closed(server_output))?;

        pending_messages.iter().try_for_each(|first_message| {
            let batch = std::iter::once(first_message)
                .chain(pending_messages.try_iter())
                .collect();
            without_superseded_changes(batch)
                .try_for_each(|message| message.write(&mut server_input))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn change_of(text: &str) -> Message {
        let uri: Uri = "repl:///repl.nu".parse().expect("static uri parses");
        document_change_message(&uri, 1, text)
    }

    fn encoded(messages: Vec<Message>) -> Vec<u8> {
        let mut bytes = Vec::new();
        messages
            .iter()
            .try_for_each(|message| message.write(&mut bytes))
            .expect("writing to a vector succeeds");
        bytes
    }

    #[test]
    fn only_the_latest_document_change_is_sent() {
        let kept_request = Message::Request(Request::new(
            1.into(),
            "textDocument/codeAction".to_string(),
            (),
        ));
        let sent: Vec<_> = without_superseded_changes(vec![
            change_of("a"),
            kept_request,
            change_of("ab"),
            change_of("abc"),
        ])
        .collect();
        assert_eq!(sent.len(), 2);
        assert!(matches!(sent[0], Message::Request(_)));
        assert!(
            encoded(vec![sent[1].clone()]).ends_with(
                encoded(vec![change_of("abc")])
                    .rsplit(|byte| *byte == b'\n')
                    .next()
                    .expect("the message has a body")
            )
        );
    }

    #[test]
    fn capability_registration_is_acknowledged() {
        let reply = server_request_reply(Request::new(
            7.into(),
            RegisterCapability::METHOD.to_string(),
            (),
        ));
        assert!(reply.response_result.is_ok());
    }

    #[test]
    fn unknown_server_requests_are_refused() {
        let reply = server_request_reply(Request::new(
            7.into(),
            "workspace/configuration".to_string(),
            (),
        ));
        assert_eq!(
            reply.response_result.err().map(|error| error.code),
            Some(ErrorCode::MethodNotFound as i32)
        );
    }

    #[test]
    fn handshake_answers_server_requests_while_waiting() {
        let incoming = encoded(vec![
            Message::Request(Request::new(
                9.into(),
                RegisterCapability::METHOD.to_string(),
                (),
            )),
            Message::Response(Response::new_ok(
                INITIALIZE_REQUEST_ID.into(),
                serde_json::Value::Null,
            )),
        ]);
        let mut written = Vec::new();
        read_until_initialized(&mut Cursor::new(incoming), &mut written)
            .expect("the initialize response arrives");
        assert!(!written.is_empty());
    }

    #[test]
    fn handshake_fails_when_the_server_closes() {
        let mut written = Vec::new();
        let outcome = read_until_initialized(&mut Cursor::new(Vec::new()), &mut written);
        assert!(outcome.is_err());
    }
}
