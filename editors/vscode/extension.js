const vscode = require('vscode');
const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

let lspProcess = null;
let diagnosticCollection = null;
let messageBuffer = '';
let reqId = 1;
const pendingRequests = new Map();
let outputChannel = null;

function log(msg) {
    if (outputChannel) {
        outputChannel.appendLine(`[DapCode] ${msg}`);
    }
}

function findCompilerPath() {
    const config = vscode.workspace.getConfiguration('dapcode');
    const customPath = config.get('compilerPath');
    if (customPath && customPath !== 'dapcodec' && fs.existsSync(customPath)) {
        return customPath;
    }

    // Search inside workspace folders
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders) {
        for (const folder of workspaceFolders) {
            const candidates = [
                path.join(folder.uri.fsPath, 'target', 'release', 'dapcodec.exe'),
                path.join(folder.uri.fsPath, 'target', 'debug', 'dapcodec.exe'),
                path.join(folder.uri.fsPath, 'target', 'release', 'dapcodec'),
                path.join(folder.uri.fsPath, 'target', 'debug', 'dapcodec'),
            ];
            for (const c of candidates) {
                if (fs.existsSync(c)) {
                    return c;
                }
            }
        }
    }

    return customPath || 'dapcodec';
}

function sendLspMessage(msg) {
    if (!lspProcess || lspProcess.killed) return;
    const body = JSON.stringify(msg);
    const header = `Content-Length: ${Buffer.byteLength(body, 'utf8')}\r\n\r\n`;
    try {
        lspProcess.stdin.write(header + body);
    } catch (err) {
        log(`Gagal mengirim pesan ke LSP: ${err.message}`);
    }
}

function sendRequest(method, params) {
    return new Promise((resolve) => {
        if (!lspProcess || lspProcess.killed) {
            return resolve(null);
        }
        const id = ++reqId;
        pendingRequests.set(id, resolve);
        sendLspMessage({
            jsonrpc: '2.0',
            id: id,
            method: method,
            params: params,
        });

        // 4-second timeout fallback
        setTimeout(() => {
            if (pendingRequests.has(id)) {
                pendingRequests.delete(id);
                resolve(null);
            }
        }, 4000);
    });
}

function handleLspOutput(chunk) {
    messageBuffer += chunk.toString('utf8');

    while (true) {
        const headerEnd = messageBuffer.indexOf('\r\n\r\n');
        if (headerEnd === -1) break;

        const header = messageBuffer.slice(0, headerEnd);
        const match = header.match(/Content-Length:\s*(\d+)/i);
        if (!match) {
            // Corrupt or unrecognized header, discard line
            messageBuffer = messageBuffer.slice(headerEnd + 4);
            continue;
        }

        const length = parseInt(match[1], 10);
        const bodyStart = headerEnd + 4;
        if (messageBuffer.length < bodyStart + length) {
            // Incomplete body, wait for next chunk
            break;
        }

        const bodyStr = messageBuffer.slice(bodyStart, bodyStart + length);
        messageBuffer = messageBuffer.slice(bodyStart + length);

        try {
            const data = JSON.parse(bodyStr);
            if (data.method === 'textDocument/publishDiagnostics') {
                const uri = vscode.Uri.parse(data.params.uri);
                const diags = (data.params.diagnostics || []).map((d) => {
                    const range = new vscode.Range(
                        d.range.start.line,
                        d.range.start.character,
                        d.range.end.line,
                        d.range.end.character
                    );
                    const severity =
                        d.severity === 1
                            ? vscode.DiagnosticSeverity.Error
                            : vscode.DiagnosticSeverity.Warning;
                    return new vscode.Diagnostic(range, d.message, severity);
                });
                diagnosticCollection.set(uri, diags);
            } else if (data.id && pendingRequests.has(data.id)) {
                const resolve = pendingRequests.get(data.id);
                pendingRequests.delete(data.id);
                resolve(data.result);
            }
        } catch (err) {
            log(`Gagal menguraikan respons LSP: ${err.message}`);
        }
    }
}

function startLspServer(context) {
    const config = vscode.workspace.getConfiguration('dapcode');
    if (!config.get('enableLsp', true)) {
        log('LSP dinonaktifkan dalam pengaturan.');
        return;
    }

    stopLspServer();

    const compilerPath = findCompilerPath();
    log(`Memulai proses LSP dengan binary: ${compilerPath} lsp`);

    try {
        lspProcess = spawn(compilerPath, ['lsp'], {
            stdio: ['pipe', 'pipe', 'pipe'],
            windowsHide: true,
        });

        lspProcess.stdout.on('data', handleLspOutput);
        lspProcess.stderr.on('data', (err) => {
            log(`LSP stderr: ${err.toString()}`);
        });

        lspProcess.on('error', (err) => {
            log(`Gagal meluncurkan binary dapcodec lsp: ${err.message}`);
        });

        lspProcess.on('exit', (code, signal) => {
            log(`LSP Server berhenti dengan kode: ${code}, signal: ${signal}`);
            lspProcess = null;
        });

        // Send initialize request
        sendRequest('initialize', {
            processId: process.pid,
            rootUri: vscode.workspace.workspaceFolders
                ? vscode.workspace.workspaceFolders[0].uri.toString()
                : null,
            capabilities: {},
        }).then(() => {
            log('LSP Server berhasil diinisialisasi!');
            // Sync all open dap documents
            vscode.workspace.textDocuments.forEach((doc) => {
                if (doc.languageId === 'dapcode') {
                    sendLspMessage({
                        jsonrpc: '2.0',
                        method: 'textDocument/didOpen',
                        params: {
                            textDocument: {
                                uri: doc.uri.toString(),
                                languageId: 'dapcode',
                                version: doc.version,
                                text: doc.getText(),
                            },
                        },
                    });
                }
            });
        });
    } catch (e) {
        log(`Pengecualian saat memulai LSP: ${e.message}`);
    }
}

function stopLspServer() {
    if (lspProcess) {
        try {
            sendLspMessage({ jsonrpc: '2.0', id: 9999, method: 'shutdown' });
            lspProcess.kill();
        } catch (_) {}
        lspProcess = null;
    }
    pendingRequests.clear();
    messageBuffer = '';
}

/**
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
    outputChannel = vscode.window.createOutputChannel('DapCode');
    diagnosticCollection = vscode.languages.createDiagnosticCollection('dapcode');
    context.subscriptions.push(outputChannel);
    context.subscriptions.push(diagnosticCollection);

    log('Ekstensi DapCode Language Support aktif!');

    // Start LSP server
    startLspServer(context);

    // 1. Hover Provider
    context.subscriptions.push(
        vscode.languages.registerHoverProvider('dapcode', {
            async provideHover(document, position) {
                const res = await sendRequest('textDocument/hover', {
                    textDocument: { uri: document.uri.toString() },
                    position: { line: position.line, character: position.character },
                });
                if (res && res.contents && res.contents.value) {
                    return new vscode.Hover(new vscode.MarkdownString(res.contents.value));
                }
                return null;
            },
        })
    );

    // 2. Completion Provider
    context.subscriptions.push(
        vscode.languages.registerCompletionItemProvider(
            'dapcode',
            {
                async provideCompletionItems(document, position) {
                    const res = await sendRequest('textDocument/completion', {
                        textDocument: { uri: document.uri.toString() },
                        position: { line: position.line, character: position.character },
                    });
                    if (Array.isArray(res)) {
                        return res.map((item) => {
                            const comp = new vscode.CompletionItem(
                                item.label,
                                item.kind || vscode.CompletionItemKind.Keyword
                            );
                            comp.detail = item.detail;
                            if (item.insertTextFormat === 2) {
                                comp.insertText = new vscode.SnippetString(item.insertText);
                            } else {
                                comp.insertText = item.insertText;
                            }
                            return comp;
                        });
                    }
                    return null;
                },
            },
            '.',
            ':',
            '"'
        )
    );

    // 3. Document Formatting Provider
    context.subscriptions.push(
        vscode.languages.registerDocumentFormattingEditProvider('dapcode', {
            async provideDocumentFormattingEdits(document) {
                const res = await sendRequest('textDocument/formatting', {
                    textDocument: { uri: document.uri.toString() },
                });
                if (Array.isArray(res) && res.length > 0) {
                    const edit = res[0];
                    const fullRange = new vscode.Range(
                        0,
                        0,
                        document.lineCount,
                        document.lineAt(document.lineCount - 1).text.length
                    );
                    return [new vscode.TextEdit(fullRange, edit.newText)];
                }
                return [];
            },
        })
    );

    // Document sync events
    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument((doc) => {
            if (doc.languageId === 'dapcode') {
                sendLspMessage({
                    jsonrpc: '2.0',
                    method: 'textDocument/didOpen',
                    params: {
                        textDocument: {
                            uri: doc.uri.toString(),
                            languageId: 'dapcode',
                            version: doc.version,
                            text: doc.getText(),
                        },
                    },
                });
            }
        })
    );

    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument((event) => {
            if (event.document.languageId === 'dapcode') {
                sendLspMessage({
                    jsonrpc: '2.0',
                    method: 'textDocument/didChange',
                    params: {
                        textDocument: {
                            uri: event.document.uri.toString(),
                            version: event.document.version,
                        },
                        contentChanges: [{ text: event.document.getText() }],
                    },
                });
            }
        })
    );

    context.subscriptions.push(
        vscode.workspace.onDidCloseTextDocument((doc) => {
            if (doc.languageId === 'dapcode') {
                diagnosticCollection.delete(doc.uri);
            }
        })
    );

    // Commands
    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.run', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            const filePath = editor.document.fileName;
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            terminal.sendText(`& "${compiler}" run "${filePath}"`);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.test', () => {
            const editor = vscode.window.activeTextEditor;
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            if (editor && editor.document.languageId === 'dapcode') {
                const filePath = editor.document.fileName;
                terminal.sendText(`& "${compiler}" test "${filePath}"`);
            } else {
                terminal.sendText(`& "${compiler}" test`);
            }
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.repl', () => {
            const compiler = findCompilerPath();
            const terminal = vscode.window.createTerminal('DapCode REPL');
            terminal.show();
            terminal.sendText(`& "${compiler}" repl`);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.doc', () => {
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === 'dapcode') {
                const filePath = editor.document.fileName;
                terminal.sendText(`& "${compiler}" doc "${filePath}" --open`);
            } else {
                terminal.sendText(`& "${compiler}" doc --open`);
            }
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.check', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            const filePath = editor.document.fileName;
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            terminal.sendText(`& "${compiler}" check "${filePath}"`);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.build', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            const filePath = editor.document.fileName;
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            terminal.sendText(`& "${compiler}" build "${filePath}"`);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.restartLsp', () => {
            startLspServer(context);
            vscode.window.showInformationMessage('DapCode Language Server telah dimulai ulang.');
        })
    );
}

function deactivate() {
    stopLspServer();
}

module.exports = {
    activate,
    deactivate,
};
