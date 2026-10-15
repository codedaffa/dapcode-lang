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

    // 4. Definition Provider (Go to Definition)
    context.subscriptions.push(
        vscode.languages.registerDefinitionProvider('dapcode', {
            async provideDefinition(document, position) {
                const res = await sendRequest('textDocument/definition', {
                    textDocument: { uri: document.uri.toString() },
                    position: { line: position.line, character: position.character },
                });
                if (res && res.uri && res.range) {
                    const targetUri = vscode.Uri.parse(res.uri);
                    const targetRange = new vscode.Range(
                        res.range.start.line,
                        res.range.start.character,
                        res.range.end.line,
                        res.range.end.character
                    );
                    return new vscode.Location(targetUri, targetRange);
                }
                return null;
            },
        })
    );

    // 5. Document Symbol Provider (Outline View & Breadcrumbs)
    function convertSymbol(sym) {
        const range = new vscode.Range(
            sym.range.start.line,
            sym.range.start.character,
            sym.range.end.line,
            sym.range.end.character
        );
        const selRange = sym.selectionRange
            ? new vscode.Range(
                  sym.selectionRange.start.line,
                  sym.selectionRange.start.character,
                  sym.selectionRange.end.line,
                  sym.selectionRange.end.character
              )
            : range;

        const docSym = new vscode.DocumentSymbol(
            sym.name,
            sym.detail || '',
            sym.kind || vscode.SymbolKind.Variable,
            range,
            selRange
        );

        if (Array.isArray(sym.children) && sym.children.length > 0) {
            docSym.children = sym.children.map(convertSymbol);
        }
        return docSym;
    }

    context.subscriptions.push(
        vscode.languages.registerDocumentSymbolProvider('dapcode', {
            async provideDocumentSymbols(document) {
                const res = await sendRequest('textDocument/documentSymbol', {
                    textDocument: { uri: document.uri.toString() },
                });
                if (Array.isArray(res)) {
                    return res.map(convertSymbol);
                }
                return [];
            },
        })
    );

    // 6. Code Action Provider (Quick Fixes & Typo Suggestions)
    context.subscriptions.push(
        vscode.languages.registerCodeActionsProvider(
            'dapcode',
            {
                async provideCodeActions(document, range, context) {
                    const res = await sendRequest('textDocument/codeAction', {
                        textDocument: { uri: document.uri.toString() },
                        range: {
                            start: { line: range.start.line, character: range.start.character },
                            end: { line: range.end.line, character: range.end.character },
                        },
                        context: {
                            diagnostics: (context.diagnostics || []).map((d) => ({
                                range: {
                                    start: { line: d.range.start.line, character: d.range.start.character },
                                    end: { line: d.range.end.line, character: d.range.end.character },
                                },
                                message: d.message,
                                severity: d.severity === vscode.DiagnosticSeverity.Error ? 1 : 2,
                            })),
                        },
                    });

                    if (Array.isArray(res)) {
                        return res.map((action) => {
                            const kind = action.kind
                                ? vscode.CodeActionKind.QuickFix
                                : undefined;
                            const ca = new vscode.CodeAction(action.title, kind);
                            ca.isPreferred = !!action.isPreferred;

                            if (action.edit && action.edit.changes) {
                                const we = new vscode.WorkspaceEdit();
                                for (const uriStr of Object.keys(action.edit.changes)) {
                                    const targetUri = vscode.Uri.parse(uriStr);
                                    const edits = action.edit.changes[uriStr];
                                    for (const e of edits) {
                                        const editRange = new vscode.Range(
                                            e.range.start.line,
                                            e.range.start.character,
                                            e.range.end.line,
                                            e.range.end.character
                                        );
                                        we.replace(targetUri, editRange, e.newText);
                                    }
                                }
                                ca.edit = we;
                            }
                            return ca;
                        });
                    }
                    return [];
                },
            },
            {
                providedCodeActionKinds: [
                    vscode.CodeActionKind.QuickFix,
                    vscode.CodeActionKind.SourceFixAll,
                ],
            }
        )
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

    function runInTerminal(terminal, compiler, subcommand, fileArg = '', extraArgs = '') {
        const isBash = (vscode.env.shell && /bash|sh|zsh/i.test(vscode.env.shell)) ||
                       (terminal.name && /bash|sh|zsh/i.test(terminal.name));
        
        let comp = compiler;
        let file = fileArg;
        
        if (isBash) {
            comp = comp.replace(/\\/g, '/');
            if (file) file = file.replace(/\\/g, '/');
            const filePart = file ? ` "${file}"` : '';
            const extraPart = extraArgs ? ` ${extraArgs}` : '';
            terminal.sendText(`"${comp}" ${subcommand}${filePart}${extraPart}`);
        } else {
            const isCmd = (vscode.env.shell && /cmd\.exe/i.test(vscode.env.shell)) || 
                          (terminal.name && /cmd/i.test(terminal.name));
            const filePart = file ? ` "${file}"` : '';
            const extraPart = extraArgs ? ` ${extraArgs}` : '';
            if (isCmd) {
                terminal.sendText(`"${comp}" ${subcommand}${filePart}${extraPart}`);
            } else {
                terminal.sendText(`& "${comp}" ${subcommand}${filePart}${extraPart}`);
            }
        }
    }

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
            runInTerminal(terminal, compiler, 'run', filePath);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.test', () => {
            const editor = vscode.window.activeTextEditor;
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            if (editor && editor.document.languageId === 'dapcode') {
                runInTerminal(terminal, compiler, 'test', editor.document.fileName);
            } else {
                runInTerminal(terminal, compiler, 'test');
            }
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.repl', () => {
            const compiler = findCompilerPath();
            const terminal = vscode.window.createTerminal('DapCode REPL');
            terminal.show();
            runInTerminal(terminal, compiler, 'repl');
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.doc', () => {
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === 'dapcode') {
                runInTerminal(terminal, compiler, 'doc', editor.document.fileName, '--open');
            } else {
                runInTerminal(terminal, compiler, 'doc', '', '--open');
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
            runInTerminal(terminal, compiler, 'check', filePath);
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
            runInTerminal(terminal, compiler, 'build', filePath);
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
