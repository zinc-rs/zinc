import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration("zinc");
  const configuredPath = config.get<string>("serverPath");
  const serverCommand = configuredPath && configuredPath.length > 0 ? configuredPath : "zinc_lsp";

  const serverOptions: ServerOptions = {
    command: serverCommand,
    args: [],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "zinc" }],
  };

  client = new LanguageClient("zincLsp", "Zinc LSP", serverOptions, clientOptions);
  context.subscriptions.push(client.start());
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) return undefined;
  return client.stop();
}
