// PLAN: 1. Define LSP backend -> 2. Wire diagnostics -> 3. Add completion items
// Library choice: tower-lsp provides a minimal, async LSP server with tokio support.

use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    documents: Arc<RwLock<std::collections::HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "zinc-lsp".to_string(),
                version: Some("1.0.3".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Zinc LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents.write().await.insert(uri.clone(), text);
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents.write().await.insert(uri.clone(), change.text);
        }
        self.publish_diagnostics(uri).await;
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let items = vec![
            CompletionItem::new_simple("print".to_string(), "Print output".to_string()),
            CompletionItem::new_simple("let".to_string(), "Declare variable".to_string()),
            CompletionItem::new_simple("spider".to_string(), "HTTP client".to_string()),
            CompletionItem::new_simple("db".to_string(), "Database module".to_string()),
            CompletionItem::new_simple("fs".to_string(), "File system module".to_string()),
        ];
        Ok(Some(CompletionResponse::Array(items)))
    }
}

impl Backend {
    async fn publish_diagnostics(&self, uri: Url) {
        let text = match self.documents.read().await.get(&uri) {
            Some(t) => t.clone(),
            None => String::new(),
        };

        let diags = match zinc_core::transpile_with_error(&text) {
            Ok(_) => Vec::new(),
            Err(err) => {
                let line = err.line.saturating_sub(1);
                let column = err.column.saturating_sub(1);
                vec![Diagnostic {
                    range: Range {
                        start: Position {
                            line: line as u32,
                            character: column as u32,
                        },
                        end: Position {
                            line: line as u32,
                            character: (column + 1) as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("zinc".to_string()),
                    message: err.message,
                    ..Default::default()
                }]
            }
        };

        self.client
            .publish_diagnostics(uri, diags, None)
            .await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Arc::new(RwLock::new(std::collections::HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
