const vscode = require('vscode');
const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

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

    // 7. Debug Adapter Protocol (DAP) Factory
    context.subscriptions.push(
        vscode.debug.registerDebugAdapterDescriptorFactory('dapcode', {
            createDebugAdapterDescriptor(session) {
                const compilerPath = findCompilerPath();
                log(`Memulai DAP server dengan: ${compilerPath} dap`);
                return new vscode.DebugAdapterExecutable(compilerPath, ['dap']);
            },
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.debug', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            vscode.debug.startDebugging(undefined, {
                type: 'dapcode',
                name: 'DapCode: Debug Berkas Aktif',
                request: 'launch',
                program: editor.document.fileName,
                stopOnEntry: false,
            });
        })
    );

    // 8. Web Playground & Canvas 2D Live Preview
    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.openPlayground', () => {
            const column = vscode.window.activeTextEditor
                ? vscode.ViewColumn.Beside
                : vscode.ViewColumn.One;

            if (playgroundPanel) {
                playgroundPanel.reveal(column);
                triggerPlaygroundCompilation();
                return;
            }

            playgroundPanel = vscode.window.createWebviewPanel(
                'dapcodePlayground',
                'DapCode Live Playground & Canvas 2D',
                column,
                {
                    enableScripts: true,
                    retainContextWhenHidden: true,
                }
            );

            playgroundPanel.webview.html = getPlaygroundWebviewHtml();

            playgroundPanel.webview.onDidReceiveMessage(
                async (message) => {
                    switch (message.command) {
                        case 'requestCompile':
                            triggerPlaygroundCompilation();
                            break;
                        case 'toggleAutoCompile':
                            autoCompileEnabled = !!message.enabled;
                            break;
                        case 'openExternal': {
                            const compiler = findCompilerPath();
                            const terminal =
                                vscode.window.activeTerminal ||
                                vscode.window.createTerminal('DapCode Playground');
                            terminal.show();
                            runInTerminal(terminal, compiler, 'playground', '', '--open');
                            break;
                        }
                        case 'applyPreset': {
                            const editor = vscode.window.activeTextEditor;
                            if (editor && editor.document.languageId === 'dapcode') {
                                const fullRange = new vscode.Range(
                                    editor.document.positionAt(0),
                                    editor.document.positionAt(editor.document.getText().length)
                                );
                                await editor.edit((editBuilder) => {
                                    editBuilder.replace(fullRange, message.code);
                                });
                                triggerPlaygroundCompilation(editor);
                            } else {
                                const doc = await vscode.workspace.openTextDocument({
                                    language: 'dapcode',
                                    content: message.code,
                                });
                                await vscode.window.showTextDocument(doc, vscode.ViewColumn.One);
                                triggerPlaygroundCompilation();
                            }
                            break;
                        }
                    }
                },
                null,
                context.subscriptions
            );

            playgroundPanel.onDidDispose(
                () => {
                    playgroundPanel = null;
                },
                null,
                context.subscriptions
            );

            // Initial compilation on open
            triggerPlaygroundCompilation();
        })
    );

    // Auto-compile on save
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((document) => {
            if (playgroundPanel && autoCompileEnabled && document.languageId === 'dapcode') {
                triggerPlaygroundCompilation();
            }
        })
    );

    // Auto-compile on editor switch
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor((editor) => {
            if (playgroundPanel && autoCompileEnabled && editor && editor.document.languageId === 'dapcode') {
                triggerPlaygroundCompilation(editor);
            }
        })
    );

    // Auto-compile debounced on document edit
    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument((event) => {
            if (
                playgroundPanel &&
                autoCompileEnabled &&
                event.document.languageId === 'dapcode' &&
                event.contentChanges.length > 0
            ) {
                if (compileDebounceTimer) {
                    clearTimeout(compileDebounceTimer);
                }
                compileDebounceTimer = setTimeout(() => {
                    triggerPlaygroundCompilation();
                }, 600);
            }
        })
    );
}

// Global Playground State & Helpers
let playgroundPanel = null;
let autoCompileEnabled = true;
let compileDebounceTimer = null;

function compileDapCodeToWasm(code) {
    return new Promise((resolve) => {
        const compilerPath = findCompilerPath();
        const tempDir = os.tmpdir();
        const randId = Math.random().toString(36).substring(2, 9);
        const tempDap = path.join(tempDir, `dap_preview_${randId}.dap`);
        const tempWasm = path.join(tempDir, `dap_preview_${randId}.wasm`);

        try {
            fs.writeFileSync(tempDap, code, 'utf8');
        } catch (err) {
            return resolve({ success: false, error: `Gagal menulis berkas sementara: ${err.message}` });
        }

        const proc = spawn(compilerPath, ['build', tempDap, '--wasm', '-o', tempWasm]);
        let stdout = '';
        let stderr = '';

        proc.stdout.on('data', (d) => {
            stdout += d.toString('utf8');
        });
        proc.stderr.on('data', (d) => {
            stderr += d.toString('utf8');
        });

        proc.on('close', (codeStatus) => {
            try {
                if (fs.existsSync(tempDap)) fs.unlinkSync(tempDap);
            } catch (_) {}

            if (codeStatus === 0 && fs.existsSync(tempWasm)) {
                try {
                    const wasmBytes = fs.readFileSync(tempWasm);
                    const b64 = wasmBytes.toString('base64');
                    fs.unlinkSync(tempWasm);
                    resolve({ success: true, wasmBase64: b64, size: wasmBytes.length });
                } catch (err) {
                    resolve({ success: false, error: `Gagal membaca biner WASM: ${err.message}` });
                }
            } else {
                try {
                    if (fs.existsSync(tempWasm)) fs.unlinkSync(tempWasm);
                } catch (_) {}
                resolve({
                    success: false,
                    error: stderr || stdout || 'Kompilasi WebAssembly gagal tanpa pesan keluaran.',
                });
            }
        });

        proc.on('error', (err) => {
            try {
                if (fs.existsSync(tempDap)) fs.unlinkSync(tempDap);
            } catch (_) {}
            resolve({
                success: false,
                error: `Gagal menjalankan kompilator '${compilerPath}': ${err.message}`,
            });
        });
    });
}

async function triggerPlaygroundCompilation(targetEditor) {
    if (!playgroundPanel) return;
    const editor = targetEditor || vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'dapcode') {
        return;
    }

    const code = editor.document.getText();
    const filename = path.basename(editor.document.fileName || 'tanpa_nama.dap');

    playgroundPanel.webview.postMessage({
        command: 'compiling',
        filename: filename,
    });

    const t0 = Date.now();
    const result = await compileDapCodeToWasm(code);
    const durationMs = Date.now() - t0;

    if (result.success) {
        playgroundPanel.webview.postMessage({
            command: 'loadWasm',
            wasmBase64: result.wasmBase64,
            size: result.size,
            filename: filename,
            compileTimeMs: durationMs,
        });
    } else {
        playgroundPanel.webview.postMessage({
            command: 'compileError',
            error: result.error,
            filename: filename,
            compileTimeMs: durationMs,
        });
    }
}

function getPlaygroundWebviewHtml() {
    return `<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DapCode Live Playground & Canvas 2D</title>
    <style>
        :root {
            --bg-primary: #0a0e17;
            --bg-secondary: #121826;
            --bg-card: rgba(18, 24, 38, 0.75);
            --border-color: rgba(56, 189, 248, 0.18);
            --border-bright: #38bdf8;
            --text-main: #f1f5f9;
            --text-muted: #94a3b8;
            --accent-cyan: #38bdf8;
            --accent-blue: #3b82f6;
            --accent-green: #10b981;
            --accent-amber: #f59e0b;
            --accent-rose: #f43f5e;
        }
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            background-color: var(--bg-primary);
            color: var(--text-main);
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            font-size: 13px;
            display: flex;
            flex-direction: column;
            min-height: 100vh;
            padding: 12px;
            gap: 10px;
        }
        .header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 8px 14px;
            backdrop-filter: blur(8px);
        }
        .header-title {
            display: flex;
            align-items: center;
            gap: 8px;
            font-weight: 700;
            font-size: 14px;
            color: var(--text-main);
        }
        .header-badge {
            background: rgba(56, 189, 248, 0.15);
            border: 1px solid rgba(56, 189, 248, 0.35);
            color: var(--accent-cyan);
            padding: 2px 8px;
            border-radius: 9999px;
            font-size: 11px;
            font-family: monospace;
        }
        .status-container {
            display: flex;
            align-items: center;
            gap: 6px;
            font-size: 12px;
        }
        .dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: var(--accent-green);
            box-shadow: 0 0 8px var(--accent-green);
        }
        .dot.compiling {
            background: var(--accent-amber);
            box-shadow: 0 0 8px var(--accent-amber);
            animation: pulse 1s infinite;
        }
        .dot.error {
            background: var(--accent-rose);
            box-shadow: 0 0 8px var(--accent-rose);
        }
        @keyframes pulse {
            0% { opacity: 0.4; }
            50% { opacity: 1; }
            100% { opacity: 0.4; }
        }
        .toolbar {
            display: flex;
            flex-wrap: wrap;
            align-items: center;
            gap: 8px;
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 8px 12px;
        }
        button, select {
            background: rgba(30, 41, 59, 0.85);
            border: 1px solid rgba(56, 189, 248, 0.25);
            color: var(--text-main);
            padding: 5px 11px;
            border-radius: 6px;
            font-size: 12px;
            cursor: pointer;
            transition: all 0.15s ease;
            outline: none;
        }
        button:hover, select:hover {
            background: rgba(56, 189, 248, 0.2);
            border-color: var(--accent-cyan);
        }
        button.primary {
            background: linear-gradient(135deg, #0284c7, #2563eb);
            border: none;
            color: white;
            font-weight: 600;
        }
        button.primary:hover {
            box-shadow: 0 0 12px rgba(56, 189, 248, 0.5);
        }
        button.active {
            background: rgba(16, 185, 129, 0.2);
            border-color: var(--accent-green);
            color: var(--accent-green);
        }
        .canvas-card {
            background: #000000;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            overflow: hidden;
            position: relative;
            display: flex;
            justify-content: center;
            align-items: center;
            box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
        }
        canvas {
            display: block;
            width: 100%;
            height: auto;
            max-height: 480px;
            object-fit: contain;
            cursor: crosshair;
        }
        .hud-overlay {
            position: absolute;
            top: 10px;
            right: 12px;
            display: flex;
            gap: 8px;
            pointer-events: none;
        }
        .hud-tag {
            background: rgba(10, 14, 23, 0.75);
            border: 1px solid rgba(56, 189, 248, 0.3);
            border-radius: 4px;
            padding: 2px 7px;
            font-size: 11px;
            font-family: monospace;
            color: var(--accent-cyan);
            backdrop-filter: blur(4px);
        }
        .metrics-bar {
            display: flex;
            flex-wrap: wrap;
            gap: 16px;
            font-size: 11px;
            color: var(--text-muted);
            padding: 4px 6px;
        }
        .metric-item span {
            color: var(--text-main);
            font-family: monospace;
            font-weight: 600;
        }
        .error-card {
            background: rgba(244, 63, 94, 0.1);
            border: 1px solid rgba(244, 63, 94, 0.4);
            border-radius: 6px;
            padding: 10px;
            color: #fca5a5;
            font-family: monospace;
            font-size: 11px;
            white-space: pre-wrap;
            display: none;
            max-height: 140px;
            overflow-y: auto;
        }
        .console-panel {
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 8px 12px;
            display: flex;
            flex-direction: column;
            gap: 6px;
            flex: 1;
            min-height: 100px;
        }
        .console-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            font-size: 11px;
            color: var(--text-muted);
            text-transform: uppercase;
            font-weight: 600;
        }
        .console-output {
            background: rgba(0, 0, 0, 0.4);
            border-radius: 4px;
            padding: 8px;
            font-family: monospace;
            font-size: 11px;
            color: #e2e8f0;
            overflow-y: auto;
            max-height: 120px;
            min-height: 48px;
            white-space: pre-wrap;
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="header-title">
            <span>🚀 DapCode Live Preview</span>
            <span class="header-badge" id="active-filename">tanpa_nama.dap</span>
        </div>
        <div class="status-container">
            <span class="dot" id="status-dot"></span>
            <span id="status-text">Siap</span>
        </div>
    </div>

    <div class="toolbar">
        <button class="primary" id="btn-compile">⚡ Kompilasi</button>
        <button id="btn-rerun">▶ Jalankan Ulang</button>
        <button class="active" id="btn-autocompile">🔄 Auto: ON</button>
        <select id="preset-select">
            <option value="tata_surya">🌌 Tata Surya Orbit 2D</option>
            <option value="spiral_archimedes">🌀 Spiral Archimedes</option>
            <option value="partikel_pantul">⚡ Partikel Pantul Fisika</option>
            <option value="diagram_batang">📊 Diagram Batang Statistik</option>
        </select>
        <button id="btn-apply-preset">📥 Ke Editor</button>
        <button id="btn-external">🌐 Buka di Browser</button>
    </div>

    <div class="canvas-card">
        <canvas id="dap-canvas" width="800" height="480"></canvas>
        <div class="hud-overlay">
            <div class="hud-tag" id="coords-hud">X: 0, Y: 0</div>
            <div class="hud-tag" id="fps-hud">60 FPS</div>
        </div>
    </div>

    <div class="metrics-bar">
        <div class="metric-item">⏱️ Kompilasi: <span id="metric-compile">-</span></div>
        <div class="metric-item">⚡ Render: <span id="metric-render">-</span></div>
        <div class="metric-item">📦 WASM: <span id="metric-size">-</span></div>
        <div class="metric-item">🎯 Instruksi: <span id="metric-cmds">0</span></div>
    </div>

    <div class="error-card" id="error-card"></div>

    <div class="console-panel">
        <div class="console-header">
            <span>🖥️ Konsol Cetak</span>
            <button style="padding: 2px 6px; font-size: 10px;" id="btn-clear-console">Bersihkan</button>
        </div>
        <div class="console-output" id="console-output">Menunggu kompilasi kode sumber...</div>
    </div>

    <script>
        const vscode = acquireVsCodeApi();
        const canvas = document.getElementById("dap-canvas");
        const ctx = canvas.getContext("2d");
        const statusDot = document.getElementById("status-dot");
        const statusText = document.getElementById("status-text");
        const activeFilename = document.getElementById("active-filename");
        const coordsHud = document.getElementById("coords-hud");
        const fpsHud = document.getElementById("fps-hud");
        const metricCompile = document.getElementById("metric-compile");
        const metricRender = document.getElementById("metric-render");
        const metricSize = document.getElementById("metric-size");
        const metricCmds = document.getElementById("metric-cmds");
        const errorCard = document.getElementById("error-card");
        const consoleOutput = document.getElementById("console-output");

        const btnCompile = document.getElementById("btn-compile");
        const btnRerun = document.getElementById("btn-rerun");
        const btnAutoCompile = document.getElementById("btn-autocompile");
        const presetSelect = document.getElementById("preset-select");
        const btnApplyPreset = document.getElementById("btn-apply-preset");
        const btnExternal = document.getElementById("btn-external");
        const btnClearConsole = document.getElementById("btn-clear-console");

        let currentWasmInstance = null;
        let autoCompile = true;
        let commandCount = 0;
        let lastFrameTime = performance.now();
        let frameCount = 0;
        let fpsTimer = performance.now();

        const PRESETS = {
            tata_surya: \`// 🌌 Tata Surya Orbit 2D
fungsi render() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    cx := w / 2.0
    cy := h / 2.0
    kanvas_bersihkan(0.04, 0.05, 0.1, 1.0)
    kanvas_warna_garis(1.0, 1.0, 1.0, 0.15)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_lingkaran(cx, cy, 70.0)
    kanvas_bingkai_lingkaran(cx, cy, 120.0)
    kanvas_bingkai_lingkaran(cx, cy, 170.0)
    kanvas_warna_isi(1.0, 0.75, 0.1, 1.0)
    kanvas_lingkaran(cx, cy, 32.0)
    p1_x := cx + 70.0 * cos(0.8)
    p1_y := cy + 70.0 * sin(0.8)
    kanvas_warna_isi(0.3, 0.8, 1.0, 1.0)
    kanvas_lingkaran(p1_x, p1_y, 10.0)
    p2_x := cx + 120.0 * cos(2.3)
    p2_y := cy + 120.0 * sin(2.3)
    kanvas_warna_isi(1.0, 0.45, 0.3, 1.0)
    kanvas_lingkaran(p2_x, p2_y, 13.0)
    p3_x := cx + 170.0 * cos(4.2)
    p3_y := cy + 170.0 * sin(4.2)
    kanvas_warna_isi(0.7, 0.3, 0.95, 1.0)
    kanvas_lingkaran(p3_x, p3_y, 16.0)
    kanvas_warna_isi(0.08, 0.12, 0.22, 0.85)
    kanvas_persegi(20.0, 20.0, 250.0, 65.0)
    kanvas_warna_garis(0.22, 0.65, 0.95, 0.75)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_persegi(20.0, 20.0, 250.0, 65.0)
    kanvas_warna_isi(0.95, 0.98, 1.0, 1.0)
    kanvas_teks("DapCode 2D Canvas Engine", 32.0, 44.0, 14.0)
    kanvas_warna_isi(0.4, 0.85, 1.0, 0.9)
    kanvas_teks("Live Preview VS Code Webview", 32.0, 66.0, 12.0)
    cetak("Tata Surya 2D berhasil dirender dari WebAssembly!")
}
fungsi utama() -> u64 {
    render()
    kembali 1
}\`,
            spiral_archimedes: \`// 🌀 Pola Spiral Archimedes
fungsi render() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    cx := w / 2.0
    cy := h / 2.0
    kanvas_bersihkan(0.02, 0.04, 0.08, 1.0)
    kanvas_tebal_garis(2.0)
    theta := 0.0
    a := 2.5
    b := 1.8
    selama theta < 25.0 {
        r := a + b * theta
        x := cx + r * cos(theta)
        y := cy + r * sin(theta)
        red := 0.5 + 0.5 * sin(theta * 0.5)
        green := 0.5 + 0.5 * cos(theta * 0.3)
        blue := 0.95
        kanvas_warna_isi(red, green, blue, 0.8)
        kanvas_lingkaran(x, y, 4.0 + (theta * 0.15))
        theta = theta + 0.15
    }
    kanvas_warna_isi(1.0, 1.0, 1.0, 0.95)
    kanvas_teks("Pola Spiral Archimedes", 25.0, 40.0, 15.0)
    cetak("Pola Spiral Archimedes selesai dirender!")
}
fungsi utama() -> u64 {
    render()
    kembali 1
}\`,
            partikel_pantul: \`// ⚡ Partikel Pantul & Medan Energi 2D
fungsi render() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    kanvas_bersihkan(0.03, 0.03, 0.06, 1.0)
    kanvas_warna_garis(0.2, 0.3, 0.5, 0.4)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_persegi(15.0, 15.0, w - 30.0, h - 30.0)
    i := 0
    selama i < 40 {
        idx := i as f64
        x := 40.0 + (idx * 18.0)
        y := (h / 2.0) + 120.0 * sin(idx * 0.35)
        r := 0.3 + 0.7 * sin(idx * 0.2)
        g := 0.5 + 0.5 * cos(idx * 0.3)
        b := 0.9
        kanvas_warna_isi(r, g, b, 0.85)
        kanvas_lingkaran(x, y, 6.0 + 3.0 * sin(idx * 0.5))
        kanvas_warna_garis(r, g, b, 0.25)
        kanvas_garis(x, y, w / 2.0, h / 2.0)
        i = i + 1
    }
    kanvas_warna_isi(1.0, 0.9, 0.2, 1.0)
    kanvas_lingkaran(w / 2.0, h / 2.0, 18.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 0.95)
    kanvas_teks("Simulasi Partikel Pantul & Medan Energi 2D", 25.0, 40.0, 14.0)
    cetak("40 partikel energi berhasil diproses!")
}
fungsi utama() -> u64 {
    render()
    kembali 1
}\`,
            diagram_batang: \`// 📊 Diagram Batang Statistik
fungsi render() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    kanvas_bersihkan(0.04, 0.06, 0.1, 1.0)
    kanvas_warna_garis(0.25, 0.35, 0.5, 0.8)
    kanvas_tebal_garis(2.0)
    kanvas_garis(60.0, h - 60.0, w - 40.0, h - 60.0)
    kanvas_garis(60.0, 60.0, 60.0, h - 60.0)
    v1 := 180.0
    v2 := 290.0
    v3 := 120.0
    v4 := 340.0
    v5 := 230.0
    kanvas_warna_isi(0.2, 0.7, 1.0, 0.85)
    kanvas_persegi(90.0, (h - 60.0) - v1, 55.0, v1)
    kanvas_warna_isi(0.3, 0.9, 0.6, 0.85)
    kanvas_persegi(180.0, (h - 60.0) - v2, 55.0, v2)
    kanvas_warna_isi(1.0, 0.8, 0.2, 0.85)
    kanvas_persegi(270.0, (h - 60.0) - v3, 55.0, v3)
    kanvas_warna_isi(0.9, 0.3, 0.5, 0.85)
    kanvas_persegi(360.0, (h - 60.0) - v4, 55.0, v4)
    kanvas_warna_isi(0.7, 0.4, 0.95, 0.85)
    kanvas_persegi(450.0, (h - 60.0) - v5, 55.0, v5)
    kanvas_warna_isi(1.0, 1.0, 1.0, 0.9)
    kanvas_teks("Q1", 105.0, h - 40.0, 13.0)
    kanvas_teks("Q2", 195.0, h - 40.0, 13.0)
    kanvas_teks("Q3", 285.0, h - 40.0, 13.0)
    kanvas_teks("Q4", 375.0, h - 40.0, 13.0)
    kanvas_teks("Q5", 465.0, h - 40.0, 13.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("Statistik Performa Sistem (WASM Canvas)", 70.0, 45.0, 16.0)
    cetak("Diagram batang statistik berhasil dirender!")
}
fungsi utama() -> u64 {
    render()
    kembali 1
}\`
        };

        function appendLog(msg) {
            consoleOutput.textContent += (consoleOutput.textContent ? "\\n" : "") + msg;
            consoleOutput.scrollTop = consoleOutput.scrollHeight;
        }

        // Host imports WebAssembly
        function createHostImports(getMemory) {
            return {
                kanvas_bersihkan: (r, g, b, a) => {
                    commandCount++;
                    ctx.fillStyle = \`rgba(\${Math.round(r * 255)}, \${Math.round(g * 255)}, \${Math.round(b * 255)}, \${a})\`;
                    ctx.fillRect(0, 0, canvas.width, canvas.height);
                },
                kanvas_warna_isi: (r, g, b, a) => {
                    commandCount++;
                    ctx.fillStyle = \`rgba(\${Math.round(r * 255)}, \${Math.round(g * 255)}, \${Math.round(b * 255)}, \${a})\`;
                },
                kanvas_warna_garis: (r, g, b, a) => {
                    commandCount++;
                    ctx.strokeStyle = \`rgba(\${Math.round(r * 255)}, \${Math.round(g * 255)}, \${Math.round(b * 255)}, \${a})\`;
                },
                kanvas_tebal_garis: (t) => {
                    commandCount++;
                    ctx.lineWidth = t;
                },
                kanvas_persegi: (x, y, w, h) => {
                    commandCount++;
                    ctx.fillRect(x, y, w, h);
                },
                kanvas_bingkai_persegi: (x, y, w, h) => {
                    commandCount++;
                    ctx.strokeRect(x, y, w, h);
                },
                kanvas_lingkaran: (x, y, r) => {
                    commandCount++;
                    ctx.beginPath();
                    ctx.arc(x, y, r, 0, Math.PI * 2);
                    ctx.fill();
                },
                kanvas_bingkai_lingkaran: (x, y, r) => {
                    commandCount++;
                    ctx.beginPath();
                    ctx.arc(x, y, r, 0, Math.PI * 2);
                    ctx.stroke();
                },
                kanvas_garis: (x1, y1, x2, y2) => {
                    commandCount++;
                    ctx.beginPath();
                    ctx.moveTo(x1, y1);
                    ctx.lineTo(x2, y2);
                    ctx.stroke();
                },
                kanvas_titik: (x, y, u) => {
                    commandCount++;
                    ctx.beginPath();
                    ctx.arc(x, y, u / 2, 0, Math.PI * 2);
                    ctx.fill();
                },
                kanvas_teks: (ptr, len, x, y, u) => {
                    commandCount++;
                    const mem = getMemory();
                    const str = mem ? new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len)) : "";
                    ctx.font = \`\${Math.round(u)}px sans-serif\`;
                    ctx.fillText(str, x, y);
                },
                kanvas_lebar: () => 800.0,
                kanvas_tinggi: () => 480.0,
                cetak_u64: (n) => appendLog(n.toString()),
                cetak_f64: (f) => appendLog(f.toString()),
                cetak_teks: (ptr, len) => {
                    const mem = getMemory();
                    const str = mem ? new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len)) : "";
                    appendLog(str);
                }
            };
        }

        async function runWasmBytes(wasmBytes) {
            commandCount = 0;
            let memRef = null;
            const imports = {
                env: createHostImports(() => memRef)
            };

            const tStart = performance.now();
            try {
                const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
                currentWasmInstance = instance;
                memRef = instance.exports.memory;

                if (instance.exports.utama) {
                    instance.exports.utama();
                } else if (instance.exports.render) {
                    instance.exports.render();
                }

                const tDuration = (performance.now() - tStart).toFixed(2);
                metricRender.textContent = \`\${tDuration} ms\`;
                metricCmds.textContent = commandCount.toString();
                statusDot.className = "dot";
                statusText.textContent = "Berhasil";
                errorCard.style.display = "none";
            } catch (err) {
                statusDot.className = "dot error";
                statusText.textContent = "Galat Runtime";
                errorCard.textContent = "Galat Eksekusi WASM: " + err.message;
                errorCard.style.display = "block";
            }
        }

        // Listener pesan dari Extension host
        window.addEventListener("message", (event) => {
            const msg = event.data;
            if (!msg) return;

            if (msg.filename) {
                activeFilename.textContent = msg.filename;
            }

            switch (msg.command) {
                case "compiling":
                    statusDot.className = "dot compiling";
                    statusText.textContent = "Mengompilasi...";
                    break;
                case "loadWasm": {
                    const binaryStr = atob(msg.wasmBase64);
                    const bytes = new Uint8Array(binaryStr.length);
                    for (let i = 0; i < binaryStr.length; i++) {
                        bytes[i] = binaryStr.charCodeAt(i);
                    }
                    metricSize.textContent = (msg.size / 1024).toFixed(2) + " KB";
                    metricCompile.textContent = (msg.compileTimeMs || 0) + " ms";
                    runWasmBytes(bytes);
                    break;
                }
                case "compileError":
                    statusDot.className = "dot error";
                    statusText.textContent = "Galat Kompilasi";
                    errorCard.textContent = msg.error;
                    errorCard.style.display = "block";
                    metricCompile.textContent = (msg.compileTimeMs || 0) + " ms";
                    break;
            }
        });

        // Mouse tracking HUD
        canvas.addEventListener("mousemove", (e) => {
            const rect = canvas.getBoundingClientRect();
            const scaleX = canvas.width / rect.width;
            const scaleY = canvas.height / rect.height;
            const x = Math.round((e.clientX - rect.left) * scaleX);
            const y = Math.round((e.clientY - rect.top) * scaleY);
            coordsHud.textContent = \`X: \${x}, Y: \${y}\`;
        });

        // FPS loop
        function updateFps() {
            frameCount++;
            const now = performance.now();
            if (now - fpsTimer >= 1000) {
                fpsHud.textContent = \`\${frameCount} FPS\`;
                frameCount = 0;
                fpsTimer = now;
            }
            requestAnimationFrame(updateFps);
        }
        requestAnimationFrame(updateFps);

        // Buttons
        btnCompile.addEventListener("click", () => {
            vscode.postMessage({ command: "requestCompile" });
        });

        btnRerun.addEventListener("click", () => {
            if (currentWasmInstance) {
                commandCount = 0;
                const tStart = performance.now();
                if (currentWasmInstance.exports.utama) {
                    currentWasmInstance.exports.utama();
                } else if (currentWasmInstance.exports.render) {
                    currentWasmInstance.exports.render();
                }
                metricRender.textContent = (performance.now() - tStart).toFixed(2) + " ms";
                metricCmds.textContent = commandCount.toString();
            } else {
                vscode.postMessage({ command: "requestCompile" });
            }
        });

        btnAutoCompile.addEventListener("click", () => {
            autoCompile = !autoCompile;
            if (autoCompile) {
                btnAutoCompile.classList.add("active");
                btnAutoCompile.textContent = "🔄 Auto: ON";
            } else {
                btnAutoCompile.classList.remove("active");
                btnAutoCompile.textContent = "🔄 Auto: OFF";
            }
            vscode.postMessage({ command: "toggleAutoCompile", enabled: autoCompile });
        });

        btnApplyPreset.addEventListener("click", () => {
            const key = presetSelect.value;
            const code = PRESETS[key];
            if (code) {
                vscode.postMessage({ command: "applyPreset", code: code });
            }
        });

        btnExternal.addEventListener("click", () => {
            vscode.postMessage({ command: "openExternal" });
        });

        btnClearConsole.addEventListener("click", () => {
            consoleOutput.textContent = "";
        });
    </script>
</body>
</html>`;
}

function deactivate() {
    stopLspServer();
}

module.exports = {
    activate,
    deactivate,
};
