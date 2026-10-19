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

    // Search in user cargo bin and well-known paths across system
    const homeDir = os.homedir();
    const globalCandidates = [
        path.join(homeDir, '.cargo', 'bin', 'dapcodec.exe'),
        'D:\\dapcode-lang\\target\\release\\dapcodec.exe',
        'D:\\dapcode-lang\\target\\debug\\dapcodec.exe',
        path.join(homeDir, '.cargo', 'bin', 'dapcodec'),
    ];
    for (const c of globalCandidates) {
        if (fs.existsSync(c)) {
            return c;
        }
    }

    return customPath || 'dapcodec';
}

let lastKnownDapFilePath = null;
let lastKnownDapDocument = null;

function getActiveOrVisibleDapDocument(target) {
    if (target) {
        if (target.document) {
            lastKnownDapDocument = target.document;
            lastKnownDapFilePath = target.document.fileName;
            return target.document;
        }
        if (target.fileName && typeof target.getText === 'function') {
            lastKnownDapDocument = target;
            lastKnownDapFilePath = target.fileName;
            return target;
        }
        if (target.fsPath) {
            const found = vscode.workspace.textDocuments.find((d) => d.fileName === target.fsPath);
            if (found) {
                lastKnownDapDocument = found;
                lastKnownDapFilePath = found.fileName;
                return found;
            }
        }
    }
    const activeEditor = vscode.window.activeTextEditor;
    if (activeEditor && (activeEditor.document.languageId === 'dapcode' || (activeEditor.document.fileName && activeEditor.document.fileName.endsWith('.dap')))) {
        lastKnownDapDocument = activeEditor.document;
        lastKnownDapFilePath = activeEditor.document.fileName;
        return activeEditor.document;
    }
    const visibleEditor = vscode.window.visibleTextEditors.find(
        (e) => e.document.languageId === 'dapcode' || (e.document.fileName && e.document.fileName.endsWith('.dap'))
    );
    if (visibleEditor) {
        lastKnownDapDocument = visibleEditor.document;
        lastKnownDapFilePath = visibleEditor.document.fileName;
        return visibleEditor.document;
    }
    if (lastKnownDapDocument && !lastKnownDapDocument.isClosed) {
        return lastKnownDapDocument;
    }
    const openDoc = vscode.workspace.textDocuments.find(
        (d) => d.languageId === 'dapcode' || (d.fileName && d.fileName.endsWith('.dap'))
    );
    if (openDoc) {
        lastKnownDapDocument = openDoc;
        lastKnownDapFilePath = openDoc.fileName;
        return openDoc;
    }
    if (activeEditor && activeEditor.document) {
        return activeEditor.document;
    }
    return null;
}

function getTargetDapFilePath(uri) {
    if (uri) {
        if (typeof uri === 'string') return uri;
        if (uri.fsPath) return uri.fsPath;
    }
    const doc = getActiveOrVisibleDapDocument();
    if (doc && doc.fileName) {
        lastKnownDapFilePath = doc.fileName;
        return doc.fileName;
    }

    if (lastKnownDapFilePath && fs.existsSync(lastKnownDapFilePath)) {
        return lastKnownDapFilePath;
    }

    const openDoc = vscode.workspace.textDocuments.find(
        (d) => d.languageId === 'dapcode' || (d.fileName && d.fileName.endsWith('.dap'))
    );
    if (openDoc) {
        lastKnownDapFilePath = openDoc.fileName;
        return openDoc.fileName;
    }

    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders && workspaceFolders.length > 0) {
        for (const wf of workspaceFolders) {
            const root = wf.uri.fsPath;
            const candidates = [
                path.join(root, 'src', 'utama.dap'),
                path.join(root, 'examples', 'kalkulator_mobile_app', 'src', 'utama.dap'),
                path.join(root, 'examples', 'demo_kanvas.dap'),
                path.join(root, 'examples', 'demo_uji.dap'),
                path.join(root, 'utama.dap'),
            ];
            for (const c of candidates) {
                if (fs.existsSync(c)) {
                    lastKnownDapFilePath = c;
                    return c;
                }
            }
        }
    }
    return null;
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

    function detectTerminalShell(terminal) {
        // 1. Cek opsi pembuatan shell jika ada
        const creationShell = (terminal && terminal.creationOptions && terminal.creationOptions.shellPath
            ? String(terminal.creationOptions.shellPath)
            : ''
        ).toLowerCase();
        if (/pwsh|powershell/.test(creationShell)) return 'powershell';
        if (/bash|sh|zsh|wsl|git/.test(creationShell)) return 'bash';
        if (/cmd\.exe|cmd/.test(creationShell)) return 'cmd';

        const termName = (terminal && terminal.name ? terminal.name : '').toLowerCase();

        // 2. Cek nama terminal aktif secara langsung jika spesifik shell
        if (/pwsh|powershell/.test(termName)) {
            return 'powershell';
        }
        if (/bash|sh|zsh|wsl|git/.test(termName)) {
            return 'bash';
        }
        if (/cmd|command\s*prompt/.test(termName)) {
            return 'cmd';
        }

        // 3. Jika nama terminal kustom (seperti "DapCode", "DapCode Mobile"), periksa profile default
        try {
            const defaultProfile = vscode.workspace.getConfiguration('terminal.integrated.defaultProfile').get('windows') || '';
            const profileStr = String(defaultProfile).toLowerCase();
            if (/pwsh|powershell/.test(profileStr)) return 'powershell';
            if (/bash|sh|zsh|wsl|git/.test(profileStr)) return 'bash';
            if (/cmd|command\s*prompt/.test(profileStr)) return 'cmd';
        } catch (_) {}

        const envShell = (vscode.env.shell || '').toLowerCase();
        if (/pwsh|powershell/.test(envShell)) return 'powershell';
        if (/bash|sh|zsh|wsl|git/.test(envShell)) return 'bash';
        if (/cmd\.exe|cmd/.test(envShell)) return 'cmd';

        return process.platform === 'win32' ? 'powershell' : 'bash';
    }

    function runInTerminal(terminal, compiler, ...args) {
        const shellType = detectTerminalShell(terminal);
        const comp = compiler.replace(/\\/g, '/');

        const formattedArgs = args
            .filter((arg) => arg !== undefined && arg !== null && arg !== '')
            .map((arg) => {
                const str = String(arg);
                // Jika argumen adalah path berkas atau mengandung spasi/karakter pemisah direktori
                if (str.includes('\\') || str.includes('/') || str.includes(' ')) {
                    const normalized = str.replace(/\\/g, '/');
                    return `"${normalized}"`;
                }
                return str;
            });

        const argsStr = formattedArgs.length > 0 ? ' ' + formattedArgs.join(' ') : '';

        if (shellType === 'powershell') {
            terminal.sendText(`& "${comp}"${argsStr}`);
        } else {
            // Bash atau CMD
            terminal.sendText(`"${comp}"${argsStr}`);
        }
    }

    // Commands
    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.run', (uri) => {
            const filePath = getTargetDapFilePath(uri);
            if (!filePath) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            runInTerminal(terminal, compiler, 'run', filePath);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.test', (uri) => {
            const filePath = getTargetDapFilePath(uri);
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            if (filePath) {
                runInTerminal(terminal, compiler, 'test', filePath);
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
        vscode.commands.registerCommand('dapcode.doc', (uri) => {
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            const filePath = getTargetDapFilePath(uri);
            if (filePath) {
                runInTerminal(terminal, compiler, 'doc', filePath, '--open');
            } else {
                runInTerminal(terminal, compiler, 'doc', '', '--open');
            }
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.check', (uri) => {
            const filePath = getTargetDapFilePath(uri);
            if (!filePath) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            runInTerminal(terminal, compiler, 'check', filePath);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.build', (uri) => {
            const filePath = getTargetDapFilePath(uri);
            if (!filePath) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode');
            terminal.show();
            runInTerminal(terminal, compiler, 'build', filePath);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.buildMobile', (uri) => {
            const filePath = getTargetDapFilePath(uri) || lastKnownDapFilePath;
            if (!filePath || !fs.existsSync(filePath)) {
                vscode.window.showWarningMessage('Buka atau pilih berkas DapCode (.dap) di samping Playground untuk diekspor ke aplikasi mobile.');
                return;
            }
            const compiler = findCompilerPath();
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('DapCode Mobile');
            terminal.show();
            runInTerminal(terminal, compiler, 'mobile', 'build', filePath);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.previewMobileGpu', (uri) => {
            const filePath = getTargetDapFilePath(uri);
            if (!filePath) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode (.dap) yang aktif.');
                return;
            }
            const compiler = findCompilerPath();
            let terminal = vscode.window.terminals.find((t) => t.name === 'DapCode Mobile Simulator');
            if (!terminal) {
                terminal = vscode.window.createTerminal('DapCode Mobile Simulator');
            }
            terminal.show();
            runInTerminal(terminal, compiler, 'mobile', 'preview', filePath);
        })
    );

    // Status bar item for direct mobile preview & hot-reload
    const mobileStatusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 90);
    mobileStatusItem.command = 'dapcode.previewMobileGpu';
    mobileStatusItem.text = '$(device-mobile) DapCode Mobile';
    mobileStatusItem.tooltip = 'Jalankan Pratinjau Mobile Direct GPU (60 FPS & Hot-Reload Otomatis) [Ctrl+F5]';
    context.subscriptions.push(mobileStatusItem);

    function updateMobileStatusBar() {
        const editor = vscode.window.activeTextEditor;
        if (editor && (editor.document.languageId === 'dapcode' || editor.document.fileName.endsWith('.dap'))) {
            mobileStatusItem.show();
        } else {
            mobileStatusItem.hide();
        }
    }
    context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(updateMobileStatusBar));
    updateMobileStatusBar();

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
        vscode.commands.registerCommand('dapcode.debug', (uri) => {
            const filePath = getTargetDapFilePath(uri);
            if (!filePath) {
                vscode.window.showErrorMessage('Tidak ada berkas DapCode yang aktif.');
                return;
            }
            vscode.debug.startDebugging(undefined, {
                type: 'dapcode',
                name: 'DapCode: Debug Berkas Aktif',
                request: 'launch',
                program: filePath,
                stopOnEntry: false,
            });
        })
    );

    // 8. Web Playground & Canvas 2D Live Preview
    context.subscriptions.push(
        vscode.commands.registerCommand('dapcode.openPlayground', (uri) => {
            const column = vscode.window.activeTextEditor
                ? vscode.ViewColumn.Beside
                : vscode.ViewColumn.One;

            if (playgroundPanel) {
                playgroundPanel.reveal(column);
                playgroundPanel.webview.html = getPlaygroundWebviewHtml();
                triggerPlaygroundCompilation(uri);
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
                        case 'buildMobile': {
                            let targetPath = (message && message.filePath) || lastKnownDapFilePath;
                            if (!targetPath || !fs.existsSync(targetPath)) {
                                let doc = getActiveOrVisibleDapDocument();
                                targetPath = doc ? doc.fileName : getTargetDapFilePath();
                            }

                            if (targetPath && fs.existsSync(targetPath)) {
                                vscode.commands.executeCommand('dapcode.buildMobile', vscode.Uri.file(targetPath));
                            } else {
                                vscode.window.showWarningMessage(
                                    'Buka berkas DapCode (.dap) yang valid di editor samping untuk dikompilasi ke aplikasi mobile native.'
                                );
                            }
                            break;
                        }
                        case 'openPresetAsNew': {
                            try {
                                const newDoc = await vscode.workspace.openTextDocument({
                                    language: 'dapcode',
                                    content: message.code,
                                });
                                await vscode.window.showTextDocument(newDoc, vscode.ViewColumn.One);
                                vscode.window.showInformationMessage(
                                    `Contoh '${message.title || 'Template Kanvas'}' telah dibuka di tab dokumen baru.`
                                );
                                triggerPlaygroundCompilation(newDoc);
                            } catch (err) {
                                vscode.window.showErrorMessage(`Gagal membuka dokumen baru: ${err.message}`);
                            }
                            break;
                        }
                        case 'applyPreset': {
                            const doc = getActiveOrVisibleDapDocument();
                            const editor = vscode.window.visibleTextEditors.find((e) => e.document === doc) || vscode.window.activeTextEditor;
                            if (editor && (editor.document.languageId === 'dapcode' || editor.document.fileName.endsWith('.dap'))) {
                                const answer = await vscode.window.showWarningMessage(
                                    `Apakah Anda yakin ingin menimpa isi berkas '${path.basename(editor.document.fileName)}' dengan contoh template? Tindakan ini akan menggantikan seluruh kode Anda saat ini.`,
                                    { modal: true },
                                    'Ya, Gantikan Kode',
                                    'Buka di Berkas Baru'
                                );
                                if (answer === 'Buka di Berkas Baru') {
                                    const newDoc = await vscode.workspace.openTextDocument({
                                        language: 'dapcode',
                                        content: message.code,
                                    });
                                    await vscode.window.showTextDocument(newDoc, vscode.ViewColumn.One);
                                    triggerPlaygroundCompilation(newDoc);
                                    return;
                                }
                                if (answer !== 'Ya, Gantikan Kode') {
                                    return;
                                }
                                const fullRange = new vscode.Range(
                                    editor.document.positionAt(0),
                                    editor.document.positionAt(editor.document.getText().length)
                                );
                                await editor.edit((editBuilder) => {
                                    editBuilder.replace(fullRange, message.code);
                                });
                                triggerPlaygroundCompilation(editor);
                            } else {
                                const newDoc = await vscode.workspace.openTextDocument({
                                    language: 'dapcode',
                                    content: message.code,
                                });
                                await vscode.window.showTextDocument(newDoc, vscode.ViewColumn.One);
                                triggerPlaygroundCompilation(newDoc);
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
            triggerPlaygroundCompilation(uri);
        })
    );

    // Auto-compile on save
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((document) => {
            if (playgroundPanel && autoCompileEnabled && (document.languageId === 'dapcode' || (document.fileName && document.fileName.endsWith('.dap')))) {
                triggerPlaygroundCompilation();
            }
        })
    );

    // Auto-compile on editor switch
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor((editor) => {
            if (playgroundPanel && autoCompileEnabled && editor && (editor.document.languageId === 'dapcode' || (editor.document.fileName && editor.document.fileName.endsWith('.dap')))) {
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
                (event.document.languageId === 'dapcode' || (event.document.fileName && event.document.fileName.endsWith('.dap'))) &&
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

function compileDapCodeToWasm(code, filePath) {
    return new Promise((resolve) => {
        const compilerPath = findCompilerPath();
        let targetDir = os.tmpdir();

        if (filePath && fs.existsSync(filePath)) {
            try {
                const dir = path.dirname(filePath);
                fs.accessSync(dir, fs.constants.W_OK);
                targetDir = dir;
            } catch (_) {
                targetDir = os.tmpdir();
            }
        }

        const randId = Math.random().toString(36).substring(2, 9);
        const tempDap = path.join(targetDir, `.dap_preview_${randId}.dap`);
        const tempWasm = path.join(os.tmpdir(), `dap_preview_${randId}.wasm`);

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
    const doc = getActiveOrVisibleDapDocument(targetEditor);
    if (!doc) {
        playgroundPanel.webview.postMessage({
            command: 'noActiveDoc',
            filename: 'Pilih Berkas .dap',
        });
        return;
    }

    const code = doc.getText();
    const filename = path.basename(doc.fileName || 'tanpa_nama.dap');
    const filepath = doc.fileName || '';

    playgroundPanel.webview.postMessage({
        command: 'compiling',
        filename: filename,
        filepath: filepath,
    });

    const t0 = Date.now();
    const result = await compileDapCodeToWasm(code, filepath);
    const durationMs = Date.now() - t0;

    if (result.success) {
        playgroundPanel.webview.postMessage({
            command: 'loadWasm',
            wasmBase64: result.wasmBase64,
            size: result.size,
            filename: filename,
            filepath: filepath,
            compileTimeMs: durationMs,
        });
    } else {
        playgroundPanel.webview.postMessage({
            command: 'compileError',
            error: result.error,
            filename: filename,
            filepath: filepath,
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
            padding: 8px;
            gap: 8px;
            overflow-x: hidden;
            width: 100%;
            max-width: 100%;
            box-sizing: border-box;
        }
        .header {
            display: flex;
            flex-direction: column;
            gap: 8px;
            background: var(--bg-card);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 10px 12px;
            backdrop-filter: blur(8px);
            width: 100%;
            box-sizing: border-box;
        }
        .header-brand {
            display: flex;
            align-items: center;
            gap: 8px;
            font-weight: 700;
            font-size: 14px;
            color: var(--text-main);
            line-height: 1.3;
            width: 100%;
            padding-bottom: 8px;
            border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        }
        .header-sub {
            display: flex;
            align-items: center;
            flex-wrap: wrap;
            gap: 8px;
            width: 100%;
            padding-top: 2px;
        }
        .header-badge {
            background: rgba(56, 189, 248, 0.12);
            border: 1px solid rgba(56, 189, 248, 0.3);
            color: var(--accent-cyan);
            padding: 3px 8px;
            border-radius: 9999px;
            font-size: 11px;
            font-family: monospace;
            max-width: 180px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }
        .status-container {
            display: inline-flex;
            align-items: center;
            gap: 6px;
            font-size: 11px;
            font-weight: 600;
            white-space: nowrap;
            flex-shrink: 0;
            background: rgba(15, 23, 42, 0.7);
            border: 1px solid rgba(255, 255, 255, 0.1);
            padding: 3px 8px;
            border-radius: 9999px;
            transition: all 0.2s ease;
            max-width: 100%;
        }
        .status-container span:last-child {
            white-space: nowrap;
        }
        .status-container.error {
            background: rgba(244, 63, 94, 0.15);
            border-color: rgba(244, 63, 94, 0.4);
            color: #fda4af;
        }
        .status-container.compiling {
            background: rgba(245, 158, 11, 0.15);
            border-color: rgba(245, 158, 11, 0.4);
            color: #fde68a;
        }
        .status-container.success {
            background: rgba(16, 185, 129, 0.15);
            border-color: rgba(16, 185, 129, 0.4);
            color: #6ee7b7;
        }
        .status-container.library {
            background: rgba(56, 189, 248, 0.15);
            border-color: rgba(56, 189, 248, 0.4);
            color: #7dd3fc;
        }
        .dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: var(--accent-green);
            box-shadow: 0 0 8px var(--accent-green);
            flex-shrink: 0;
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
        .dot.library {
            background: var(--accent-cyan);
            box-shadow: 0 0 8px var(--accent-cyan);
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
            padding: 5px 10px;
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
        .canvas-viewport {
            width: 100%;
            display: flex;
            justify-content: center;
            align-items: center;
            background: #020617;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 12px;
            min-height: 240px;
            overflow: hidden;
            box-sizing: border-box;
            position: relative;
        }
        .device-frame-mockup {
            position: relative;
            display: flex;
            justify-content: center;
            align-items: center;
            max-width: 100%;
            transition: all 0.2s ease-out;
        }
        .device-frame-mockup.with-bezel {
            border: 10px solid #1e293b;
            border-radius: 28px;
            box-shadow: 0 16px 40px rgba(0, 0, 0, 0.8), 0 0 0 1px rgba(255, 255, 255, 0.08);
            padding: 8px;
            background: #090d16;
        }
        .device-notch {
            display: none;
            position: absolute;
            top: 2px;
            left: 50%;
            transform: translateX(-50%);
            width: 70px;
            height: 12px;
            background: #1e293b;
            border-radius: 0 0 8px 8px;
            z-index: 10;
        }
        .device-frame-mockup.with-bezel .device-notch {
            display: block;
        }
        canvas#dap-canvas {
            display: block;
            max-width: 100%;
            height: auto;
            max-height: 56vh;
            object-fit: contain;
            border-radius: 6px;
            background: #000000;
            box-shadow: 0 4px 16px rgba(0, 0, 0, 0.6);
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
            background: rgba(244, 63, 94, 0.12);
            border: 1px solid rgba(244, 63, 94, 0.45);
            border-radius: 8px;
            padding: 10px 12px;
            color: #fecdd3;
            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
            font-size: 11px;
            line-height: 1.5;
            white-space: pre-wrap;
            word-break: break-word;
            overflow-wrap: anywhere;
            display: none;
            max-height: 180px;
            overflow-y: auto;
            overflow-x: hidden;
            width: 100%;
            max-width: 100%;
            box-sizing: border-box;
            box-shadow: 0 4px 12px rgba(244, 63, 94, 0.15);
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
        .header-actions {
            display: flex;
            align-items: center;
            gap: 12px;
        }
        .btn-ghost {
            background: rgba(56, 189, 248, 0.1);
            border: 1px solid rgba(56, 189, 248, 0.3);
            color: var(--accent-cyan);
            padding: 3px 9px;
            border-radius: 6px;
            font-size: 11px;
            cursor: pointer;
            transition: all 0.15s ease;
        }
        .btn-ghost:hover, .btn-ghost.active {
            background: rgba(56, 189, 248, 0.25);
            border-color: var(--accent-cyan);
            color: #ffffff;
        }
        .guide-card {
            background: rgba(15, 23, 42, 0.95);
            border: 1px solid rgba(56, 189, 248, 0.35);
            border-radius: 8px;
            padding: 12px 16px;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
            display: flex;
            flex-direction: column;
            gap: 10px;
            backdrop-filter: blur(12px);
        }
        .guide-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            border-bottom: 1px solid rgba(255, 255, 255, 0.08);
            padding-bottom: 6px;
        }
        .guide-header h4 {
            font-size: 13px;
            color: var(--accent-cyan);
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .close-guide-btn {
            background: none;
            border: none;
            color: var(--text-muted);
            cursor: pointer;
            font-size: 14px;
            padding: 2px 6px;
        }
        .close-guide-btn:hover {
            color: var(--text-main);
        }
        .guide-content {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
            gap: 12px;
        }
        .guide-step {
            background: rgba(30, 41, 59, 0.45);
            border: 1px solid rgba(255, 255, 255, 0.05);
            border-radius: 6px;
            padding: 10px;
            font-size: 12px;
            line-height: 1.5;
        }
        .guide-step strong {
            color: var(--text-main);
            display: block;
            margin-bottom: 4px;
        }
        .guide-step p {
            color: var(--text-muted);
            margin-bottom: 6px;
        }
        .guide-step code {
            background: rgba(56, 189, 248, 0.15);
            color: var(--accent-cyan);
            padding: 1px 4px;
            border-radius: 3px;
            font-family: monospace;
            font-size: 11px;
        }
        .guide-step ul {
            margin-left: 16px;
            color: var(--text-muted);
        }
        .guide-step li {
            margin-bottom: 3px;
        }
        .toolbar-divider {
            width: 1px;
            height: 20px;
            background: rgba(255, 255, 255, 0.15);
            margin: 0 4px;
        }
        .template-dropdown-wrapper {
            position: relative;
            display: inline-block;
        }
        .template-menu {
            position: absolute;
            top: 100%;
            left: 0;
            margin-top: 6px;
            background: #0f172a;
            border: 1px solid rgba(56, 189, 248, 0.35);
            border-radius: 8px;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.7);
            width: 290px;
            z-index: 100;
            overflow: hidden;
            backdrop-filter: blur(12px);
        }
        .template-menu-header {
            padding: 8px 12px;
            font-size: 11px;
            font-weight: 600;
            color: var(--text-muted);
            text-transform: uppercase;
            background: rgba(255, 255, 255, 0.03);
            border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        }
        .template-item {
            padding: 8px 12px;
            font-size: 12px;
            color: var(--text-main);
            cursor: pointer;
            transition: all 0.15s ease;
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .template-item:hover {
            background: rgba(56, 189, 248, 0.15);
            color: var(--accent-cyan);
        }
        .canvas-empty-overlay {
            position: absolute;
            inset: 0;
            background: rgba(8, 12, 20, 0.94);
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            padding: 24px;
            text-align: center;
            backdrop-filter: blur(4px);
        }
        .empty-card {
            max-width: 520px;
            background: rgba(18, 24, 38, 0.9);
            border: 1px solid rgba(56, 189, 248, 0.25);
            border-radius: 10px;
            padding: 20px 24px;
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 10px;
            box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
        }
        .empty-icon {
            font-size: 34px;
        }
        .empty-title {
            font-size: 15px;
            font-weight: 700;
            color: var(--accent-cyan);
        }
        .empty-desc {
            font-size: 12px;
            color: var(--text-main);
            line-height: 1.5;
        }
        .empty-desc strong {
            color: #f59e0b;
            font-family: monospace;
        }
        .empty-hint {
            font-size: 11px;
            color: var(--text-muted);
            line-height: 1.5;
            background: rgba(0, 0, 0, 0.35);
            padding: 8px 12px;
            border-radius: 6px;
            border-left: 3px solid var(--accent-blue);
            text-align: left;
            width: 100%;
        }
        .empty-hint code {
            color: var(--accent-cyan);
            font-family: monospace;
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="header-brand">
            <span style="font-size: 16px;">🎨</span>
            <span>DapCode Live Canvas &amp; Playground</span>
        </div>
        <div class="header-sub">
            <span class="header-badge" id="active-filename" title="Berkas yang sedang aktif di editor">tanpa_nama.dap</span>
            <button id="btn-toggle-guide" class="btn-ghost" title="Tampilkan/sembunyikan panduan kanvas">📖 Cara Pakai</button>
            <div class="status-container" id="status-container">
                <span class="dot" id="status-dot"></span>
                <span id="status-text">Siap</span>
            </div>
        </div>
    </div>

    <!-- Panel Panduan Cepat (Collapsible) -->
    <div class="guide-card" id="guide-card" style="display: none;">
        <div class="guide-header">
            <h4>📖 Panduan: Menggunakan Kanvas 2D untuk Berkas DapCode Anda</h4>
            <button id="btn-close-guide" class="close-guide-btn">✕</button>
        </div>
        <div class="guide-content">
            <div class="guide-step">
                <strong>1. Dinamis untuk Berkas Aktif:</strong>
                <p>Playground ini secara otomatis merender berkas <code>.dap</code> yang sedang Anda buka di editor VS Code. Ubah kode di editor dan kanvas akan langsung terbarui secara instan.</p>
            </div>
            <div class="guide-step">
                <strong>2. Cara Menggambar Grafis 2D:</strong>
                <p>Tulis fungsi <code>render()</code>, <code>dap_gambar()</code>, atau <code>utama()</code> di berkas Anda, lalu panggil fungsi grafis bawaan:</p>
                <ul>
                    <li><code>kanvas_bersihkan(r, g, b, a)</code> — Warna latar</li>
                    <li><code>kanvas_lingkaran(x, y, r)</code> &amp; <code>kanvas_warna_isi(r, g, b, a)</code></li>
                    <li><code>kanvas_persegi(x, y, w, h)</code> &amp; <code>kanvas_bingkai_persegi(...)</code></li>
                    <li><code>kanvas_garis(x1, y1, x2, y2)</code> &amp; <code>kanvas_teks(teks, x, y, sz)</code></li>
                    <li><code>kanvas_mouse_x()</code>, <code>kanvas_mouse_y()</code>, <code>kanvas_mouse_klik()</code></li>
                </ul>
            </div>
            <div class="guide-step">
                <strong>3. Logika &amp; Konsol Teks:</strong>
                <p>Panggilan <code>cetak(...)</code> otomatis dicetak pada panel <em>Konsol Cetak</em> di bawah tanpa harus memanggil fungsi kanvas.</p>
            </div>
        </div>
    </div>

    <div class="toolbar">
        <button class="primary" id="btn-compile" title="Kompilasi ulang berkas aktif sekarang">⚡ Kompilasi</button>
        <button class="active" id="btn-autocompile" title="Kompilasi otomatis setiap kali mengetik atau menyimpan">🔄 Auto: ON</button>
        <button id="btn-rerun" title="Jalankan ulang logika render">▶ Jalankan Ulang</button>
        <button id="btn-loop" title="Aktifkan loop animasi 60 FPS terus-menerus">🔁 Loop: OFF</button>
        <button id="btn-clear-canvas" title="Bersihkan tampilan kanvas">🧹 Bersihkan</button>
        
        <div class="toolbar-divider"></div>

        <div style="display: flex; align-items: center; gap: 5px;">
            <label for="frame-select" style="font-size: 11px; color: var(--accent-cyan); font-weight: 600;">📐 Frame:</label>
            <select id="frame-select" title="Pilih ukuran frame kanvas">
                <option value="autofit">Auto-Fit (Responsif Kontainer)</option>
                <option value="desktop">Desktop (800 × 480)</option>
                <option value="desktop_hd">Desktop HD (960 × 540)</option>
                <option value="mobile_p" selected>Mobile Portrait (360 × 640)</option>
                <option value="mobile_l">Mobile Lanskap (640 × 360)</option>
                <option value="tablet">Tablet (768 × 576)</option>
                <option value="square">Persegi (500 × 500)</option>
            </select>
            <button id="btn-rotate-frame" class="btn-ghost" title="Putar orientasi layar (Portrait ⇄ Lanskap)">🔄 Putar</button>
            <button id="btn-toggle-bezel" class="btn-ghost" title="Nyalakan/matikan bezel bingkai perangkat">📱 Bezel: ON</button>
        </div>

        <div class="toolbar-divider"></div>

        <div class="template-dropdown-wrapper">
            <button id="btn-template-menu" class="btn-ghost" title="Buka contoh template kanvas di tab dokumen baru">💡 Contoh Template ▾</button>
            <div class="template-menu" id="template-menu" style="display: none;">
                <div class="template-menu-header">Buka Contoh di Dokumen Baru:</div>
                <div class="template-item" data-preset="tata_surya">🌌 1. Tata Surya Orbit 2D</div>
                <div class="template-item" data-preset="spiral_archimedes">🌀 2. Pola Spiral Archimedes</div>
                <div class="template-item" data-preset="partikel_pantul">⚡ 3. Partikel Pantul Fisika</div>
                <div class="template-item" data-preset="diagram_batang">📊 4. Diagram Batang Statistik</div>
                <div class="template-item" data-preset="game_pesawat">🎮 5. Game Pesawat Antariksa 2D</div>
            </div>
        </div>

        <button id="btn-mobile-export" style="background: linear-gradient(135deg, #0284c7, #2563eb); color: white;" title="Bangun berkas DapCode aktif ini ke proyek Mobile Native (Android &amp; iOS)">📱 Bangun Mobile Native</button>
    </div>

    <div class="error-card" id="error-card" style="display: none;"></div>

    <div class="canvas-viewport" id="canvas-viewport">
        <div class="device-frame-mockup with-bezel" id="device-frame-mockup">
            <div class="device-notch"></div>
            <canvas id="dap-canvas" width="360" height="640"></canvas>
            <div class="hud-overlay">
                <div class="hud-tag" id="frame-dim-hud">360 × 640</div>
                <div class="hud-tag" id="coords-hud">X: 0, Y: 0</div>
                <div class="hud-tag" id="fps-hud">60 FPS</div>
            </div>
            <div class="canvas-empty-overlay" id="empty-canvas-overlay" style="display: none;">
                <div class="empty-card">
                    <div class="empty-icon" id="empty-icon">🎨</div>
                    <div class="empty-title" id="empty-title">Kanvas Siap Digunakan</div>
                    <div class="empty-desc" id="empty-desc">Berkas <strong id="empty-filename">berkas.dap</strong> berhasil dikompilasi, tetapi belum ada instruksi kanvas yang dipanggil.</div>
                    <div class="empty-hint" id="empty-hint">
                        Tambahkan fungsi <code>render()</code> atau panggil fungsi kanvas (seperti <code>kanvas_lingkaran</code>, <code>kanvas_persegi</code>, <code>kanvas_teks</code>) di berkas Anda untuk melihat visual di sini.<br>
                        Jika kode Anda berisi <code>cetak(...)</code>, output teks dapat dilihat pada <strong>Konsol Cetak</strong> di bawah.
                    </div>
                </div>
            </div>
        </div>
    </div>

    <div class="metrics-bar">
        <div class="metric-item">⏱️ Kompilasi: <span id="metric-compile">-</span></div>
        <div class="metric-item">⚡ Render: <span id="metric-render">-</span></div>
        <div class="metric-item">📦 WASM: <span id="metric-size">-</span></div>
        <div class="metric-item">🎯 Instruksi: <span id="metric-cmds">0</span></div>
    </div>

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
        const statusContainer = document.getElementById("status-container");

        function setStatus(type, text) {
            if (statusDot) {
                statusDot.className = "dot" + (type ? " " + type : "");
            }
            if (statusContainer) {
                statusContainer.className = "status-container" + (type ? " " + type : "");
            }
            if (statusText) {
                statusText.textContent = text;
            }
        }
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
        const btnLoop = document.getElementById("btn-loop");
        const btnAutoCompile = document.getElementById("btn-autocompile");
        const presetSelect = document.getElementById("preset-select");
        const btnApplyPreset = document.getElementById("btn-apply-preset");
        const btnExternal = document.getElementById("btn-external");
        const btnClearConsole = document.getElementById("btn-clear-console");

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
}\`,
            game_pesawat: \`// 🎮 Simulator Pesawat Antariksa 2D
fungsi gambar_bintang(w: f64, h: f64) -> Unit {
    kanvas_bersihkan(0.02, 0.03, 0.07, 1.0)
    kanvas_warna_isi(0.8, 0.9, 1.0, 0.5)
    kanvas_titik(80.0, 60.0, 2.0)
    kanvas_titik(220.0, 110.0, 2.5)
    kanvas_titik(450.0, 80.0, 1.5)
    kanvas_titik(670.0, 140.0, 2.0)
    kanvas_titik(140.0, 320.0, 2.0)
    kanvas_titik(380.0, 420.0, 3.0)
    kanvas_titik(590.0, 310.0, 1.5)
    kanvas_titik(720.0, 410.0, 2.5)
}
fungsi render_pesawat(px: f64, py: f64, klik: u64) -> Unit {
    kanvas_warna_isi(1.0, 0.45, 0.1, 0.9)
    kanvas_lingkaran(px - 22.0, py, 7.0)
    kanvas_warna_isi(1.0, 0.85, 0.2, 0.95)
    kanvas_lingkaran(px - 17.0, py, 4.0)
    kanvas_warna_isi(0.18, 0.45, 0.85, 1.0)
    kanvas_garis(px + 28.0, py, px - 18.0, py - 18.0)
    kanvas_garis(px - 18.0, py - 18.0, px - 10.0, py)
    kanvas_garis(px - 10.0, py, px - 18.0, py + 18.0)
    kanvas_garis(px - 18.0, py + 18.0, px + 28.0, py)
    kanvas_warna_isi(0.25, 0.65, 1.0, 0.9)
    kanvas_persegi(px - 10.0, py - 6.0, 22.0, 12.0)
    kanvas_warna_isi(0.4, 0.95, 1.0, 0.95)
    kanvas_lingkaran(px + 6.0, py, 5.0)
    jika klik == 1 {
        kanvas_warna_garis(0.2, 1.0, 0.5, 0.95)
        kanvas_tebal_garis(3.0)
        kanvas_garis(px + 20.0, py - 12.0, 800.0, py - 12.0)
        kanvas_garis(px + 20.0, py + 12.0, 800.0, py + 12.0)
        kanvas_warna_isi(1.0, 1.0, 1.0, 0.9)
        kanvas_lingkaran(px + 22.0, py - 12.0, 4.0)
        kanvas_lingkaran(px + 22.0, py + 12.0, 4.0)
    }
}
fungsi render_game() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()
    gambar_bintang(w, h)
    render_pesawat(mx, my, klik)
    kanvas_warna_garis(0.2, 0.8, 1.0, 0.5)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_lingkaran(mx, my, 35.0)
    kanvas_garis(mx - 45.0, my, mx + 45.0, my)
    kanvas_garis(mx, my - 45.0, mx, my + 45.0)
    kanvas_warna_isi(0.06, 0.1, 0.18, 0.85)
    kanvas_persegi(15.0, 15.0, 300.0, 75.0)
    kanvas_warna_garis(0.3, 0.7, 1.0, 0.6)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_persegi(15.0, 15.0, 300.0, 75.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("🎮 SIMULATOR PESAWAT ANTARIKSA", 26.0, 38.0, 13.0)
    kanvas_warna_isi(0.4, 0.9, 1.0, 0.9)
    kanvas_teks("Kontrol: Gerakkan Mouse & Klik Tembak", 26.0, 58.0, 11.0)
    jika klik == 1 {
        kanvas_warna_isi(0.2, 1.0, 0.5, 1.0)
        kanvas_teks("STATUS: ⚡ LASER DIAKTIFKAN!", 26.0, 76.0, 11.0)
    } selain_itu {
        kanvas_warna_isi(0.8, 0.8, 0.8, 0.7)
        kanvas_teks("STATUS: Menjelajah Orbit Antariksa", 26.0, 76.0, 11.0)
    }
}
fungsi utama() -> u64 {
    render_game()
    kembali 1
}\`
        };

        let currentWasmInstance = null;
        let autoCompile = true;
        let commandCount = 0;
        let lastFrameTime = performance.now();
        let frameCount = 0;
        let fpsTimer = performance.now();
        let canvasMouseX = 400.0;
        let canvasMouseY = 240.0;
        let canvasMouseDown = false;
        const canvasKeysPressed = new Set();
        let isLooping = false;
        let loopReqId = null;
        let currentDeltaTime = 0.0166;
        let lastLoopTime = performance.now();

        // Penyimpanan KV Status Kanvas & Scratch String Memory
        const canvasKvStore = new Map();
        let wasmScratchOffset = 48000;

        function decodeWasmStr(mem, ptr, len) {
            if (!mem || !mem.buffer) return "";
            const u8 = new Uint8Array(mem.buffer);
            if (len !== undefined && len > 0) {
                return new TextDecoder().decode(u8.subarray(ptr, ptr + len));
            }
            let end = ptr;
            while (end < u8.length && u8[end] !== 0) {
                end++;
            }
            return new TextDecoder().decode(u8.subarray(ptr, end));
        }

        function writeWasmStr(mem, str) {
            if (!mem || !mem.buffer) return 0;
            const encoded = new TextEncoder().encode(str);
            const u8 = new Uint8Array(mem.buffer);
            const targetPtr = wasmScratchOffset;
            for (let i = 0; i < encoded.length; i++) {
                u8[targetPtr + i] = encoded[i];
            }
            u8[targetPtr + encoded.length] = 0;
            wasmScratchOffset += encoded.length + 1;
            if (wasmScratchOffset > 64000) wasmScratchOffset = 48000;
            return targetPtr;
        }

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
                    const str = decodeWasmStr(mem, ptr, len);
                    ctx.font = \`\${Math.round(u)}px sans-serif\`;
                    ctx.fillText(str, x, y);
                },
                // Ukuran kanvas dinamis sesuai frame preset yang dipilih
                kanvas_lebar: () => (canvas ? canvas.width : 360.0),
                kanvas_tinggi: () => (canvas ? canvas.height : 640.0),
                kanvas_mouse_x: () => canvasMouseX,
                kanvas_mouse_y: () => canvasMouseY,
                kanvas_mouse_klik: () => (canvasMouseDown ? 1n : 0n),
                kanvas_tombol_ditekan: (kode) => (canvasKeysPressed.has(Number(kode)) ? 1n : 0n),
                kanvas_delta_waktu: () => currentDeltaTime,
                waktu_sekarang_ms: () => BigInt(Date.now()),

                // Penyimpanan State Persisten Kanvas / UI
                kanvas_simpan_teks: (kPtr, vPtr) => {
                    const mem = getMemory();
                    const k = decodeWasmStr(mem, kPtr);
                    const v = decodeWasmStr(mem, vPtr);
                    canvasKvStore.set(k, v);
                },
                kanvas_ambil_teks: (kPtr) => {
                    const mem = getMemory();
                    const k = decodeWasmStr(mem, kPtr);
                    const val = canvasKvStore.get(k) || "";
                    return writeWasmStr(mem, val);
                },
                kanvas_simpan_u64: (kPtr, val) => {
                    const mem = getMemory();
                    const k = decodeWasmStr(mem, kPtr);
                    canvasKvStore.set(k, BigInt(val));
                },
                kanvas_ambil_u64: (kPtr) => {
                    const mem = getMemory();
                    const k = decodeWasmStr(mem, kPtr);
                    const val = canvasKvStore.get(k);
                    return val !== undefined ? BigInt(val) : 0n;
                },
                kanvas_simpan_angka: (kPtr, val) => {
                    const mem = getMemory();
                    const k = decodeWasmStr(mem, kPtr);
                    canvasKvStore.set(k, Number(val));
                },
                kanvas_ambil_angka: (kPtr) => {
                    const mem = getMemory();
                    const k = decodeWasmStr(mem, kPtr);
                    const val = canvasKvStore.get(k);
                    return val !== undefined ? Number(val) : 0.0;
                },
                ke_desimal: (sPtr) => {
                    const mem = getMemory();
                    const s = decodeWasmStr(mem, sPtr);
                    const parsed = parseFloat(s);
                    return isNaN(parsed) ? 0.0 : parsed;
                },
                format_desimal: (val) => {
                    const mem = getMemory();
                    const str = (Math.round(val * 10000) / 10000).toString();
                    return writeWasmStr(mem, str);
                },
                panjang_teks: (sPtr) => {
                    const mem = getMemory();
                    const s = decodeWasmStr(mem, sPtr);
                    return BigInt(s.length);
                },
                teks_gabung: (s1Ptr, s2Ptr) => {
                    const mem = getMemory();
                    const s1 = decodeWasmStr(mem, s1Ptr);
                    const s2 = decodeWasmStr(mem, s2Ptr);
                    return writeWasmStr(mem, s1 + s2);
                },
                teks_format_angka: (val) => {
                    const mem = getMemory();
                    const str = (Math.round(val * 10000) / 10000).toString();
                    return writeWasmStr(mem, str);
                },
                teks_format_u64: (val) => {
                    const mem = getMemory();
                    return writeWasmStr(mem, val.toString());
                },

                cetak_u64: (n) => appendLog(n.toString()),
                cetak_f64: (f) => appendLog(f.toString()),
                cetak_teks: (ptr, len) => {
                    const mem = getMemory();
                    const str = decodeWasmStr(mem, ptr, len);
                    appendLog(str);
                },

                // Fungsi Matematika Bawaan WebAssembly
                sin: Math.sin,
                cos: Math.cos,
                tan: Math.tan,
                asin: Math.asin,
                acos: Math.acos,
                atan: Math.atan,
                atan2: Math.atan2,
                akar_kubik: Math.cbrt,
                akar_kuadrat: Math.sqrt,
                pangkat: Math.pow,
                log: Math.log,
                log10: Math.log10,
                log2: Math.log2,
                exp: Math.exp,
                tanda: Math.sign,
                lantai: Math.floor,
                bulat: Math.round,
                langit_langit: Math.ceil,
                min: Math.min,
                max: Math.max,
                nilai_mutlak: Math.abs,

                // Web Audio Synthesizer Bawaan
                audio_mainkan_nada: (freq, dur, vol) => {
                    try {
                        const ctx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                        if (ctx.state === 'suspended') ctx.resume();
                        const osc = ctx.createOscillator();
                        const gain = ctx.createGain();
                        osc.type = 'sine';
                        osc.frequency.setValueAtTime(freq, ctx.currentTime);
                        const now = ctx.currentTime;
                        gain.gain.setValueAtTime(0.0001, now);
                        gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                        osc.connect(gain);
                        gain.connect(ctx.destination);
                        osc.start(now);
                        osc.stop(now + dur + 0.05);
                    } catch (_) {}
                },
                audio_mainkan_gelombang: (tipe, freq, dur, vol) => {
                    try {
                        const ctx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                        if (ctx.state === 'suspended') ctx.resume();
                        const types = ['sine', 'square', 'sawtooth', 'triangle'];
                        const osc = ctx.createOscillator();
                        const gain = ctx.createGain();
                        osc.type = types[Number(tipe) % 4] || 'sine';
                        osc.frequency.setValueAtTime(freq, ctx.currentTime);
                        const now = ctx.currentTime;
                        gain.gain.setValueAtTime(0.0001, now);
                        gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                        osc.connect(gain);
                        gain.connect(ctx.destination);
                        osc.start(now);
                        osc.stop(now + dur + 0.05);
                    } catch (_) {}
                },
                audio_efek_suara: (tipe) => {
                    try {
                        const ctx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                        if (ctx.state === 'suspended') ctx.resume();
                        const osc = ctx.createOscillator();
                        const gain = ctx.createGain();
                        const now = ctx.currentTime;
                        if (Number(tipe) === 0) {
                            osc.frequency.setValueAtTime(800, now);
                            gain.gain.setValueAtTime(0.3, now);
                            gain.gain.exponentialRampToValueAtTime(0.001, now + 0.05);
                            osc.connect(gain); gain.connect(ctx.destination);
                            osc.start(now); osc.stop(now + 0.05);
                        } else {
                            osc.frequency.setValueAtTime(250, now);
                            osc.frequency.exponentialRampToValueAtTime(600, now + 0.15);
                            gain.gain.setValueAtTime(0.4, now);
                            gain.gain.exponentialRampToValueAtTime(0.001, now + 0.15);
                            osc.connect(gain); gain.connect(ctx.destination);
                            osc.start(now); osc.stop(now + 0.15);
                        }
                    } catch (_) {}
                },
                audio_set_volume: () => {},
                audio_hentikan_semua: () => {
                    if (window.__dapAudioCtx) window.__dapAudioCtx.suspend();
                },
                audio_frekuensi_nada: (ptr, len) => {
                    const mem = getMemory();
                    const str = mem ? new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len)) : "";
                    const notes = {
                        "C4": 261.63, "D4": 293.66, "E4": 329.63, "F4": 349.23,
                        "G4": 392.00, "A4": 440.00, "B4": 493.88, "C5": 523.25
                    };
                    return notes[str] || 440.0;
                }
            };
        }

        const emptyCanvasOverlay = document.getElementById("empty-canvas-overlay");
        const emptyFilename = document.getElementById("empty-filename");
        const emptyTitle = document.getElementById("empty-title");
        const emptyDesc = document.getElementById("empty-desc");
        const btnToggleGuide = document.getElementById("btn-toggle-guide");
        const guideCard = document.getElementById("guide-card");
        const btnCloseGuide = document.getElementById("btn-close-guide");
        const btnClearCanvas = document.getElementById("btn-clear-canvas");
        const btnTemplateMenu = document.getElementById("btn-template-menu");
        const templateMenu = document.getElementById("template-menu");
        const frameSelect = document.getElementById("frame-select");
        const btnRotateFrame = document.getElementById("btn-rotate-frame");
        const btnToggleBezel = document.getElementById("btn-toggle-bezel");
        const deviceMockup = document.getElementById("device-frame-mockup");
        const frameDimHud = document.getElementById("frame-dim-hud");
        const canvasViewport = document.getElementById("canvas-viewport");

        let currentFrameMode = "mobile_p";
        let isBezelActive = true;
        const FRAME_PRESETS = {
            desktop: { w: 800, h: 480, bezel: false },
            desktop_hd: { w: 960, h: 540, bezel: false },
            mobile_p: { w: 360, h: 640, bezel: true },
            mobile_l: { w: 640, h: 360, bezel: true },
            tablet: { w: 768, h: 576, bezel: true },
            square: { w: 500, h: 500, bezel: false },
            autofit: { autofit: true, bezel: false }
        };

        function applyFrameDimensions(w, h, updateHud = true) {
            canvas.width = Math.max(100, Math.round(w));
            canvas.height = Math.max(100, Math.round(h));
            if (deviceMockup) {
                deviceMockup.style.aspectRatio = \`\${canvas.width} / \${canvas.height}\`;
            }
            if (updateHud && frameDimHud) {
                frameDimHud.textContent = \`\${canvas.width} × \${canvas.height}\`;
            }
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
        }

        function setFramePreset(key) {
            currentFrameMode = key;
            const preset = FRAME_PRESETS[key];
            if (!preset) return;

            if (preset.autofit) {
                if (deviceMockup) deviceMockup.classList.remove("with-bezel");
                if (btnToggleBezel) btnToggleBezel.style.display = "none";
                if (btnRotateFrame) btnRotateFrame.style.display = "none";
                updateAutoFitDimensions();
            } else {
                if (btnToggleBezel) btnToggleBezel.style.display = "inline-block";
                if (btnRotateFrame) btnRotateFrame.style.display = "inline-block";
                isBezelActive = preset.bezel;
                if (deviceMockup) deviceMockup.classList.toggle("with-bezel", isBezelActive);
                if (btnToggleBezel) btnToggleBezel.textContent = isBezelActive ? "📱 Bezel: ON" : "📱 Bezel: OFF";
                applyFrameDimensions(preset.w, preset.h);
            }
        }

        function updateAutoFitDimensions() {
            if (currentFrameMode !== "autofit" || !canvasViewport) return;
            const vpRect = canvasViewport.getBoundingClientRect();
            const w = Math.max(200, Math.floor(vpRect.width - 24));
            const h = Math.max(200, Math.floor(vpRect.height - 24));
            applyFrameDimensions(w, h);
        }

        const resizeObserver = new ResizeObserver(() => {
            if (currentFrameMode === "autofit") {
                updateAutoFitDimensions();
            }
        });
        if (canvasViewport) {
            resizeObserver.observe(canvasViewport);
        }

        if (frameSelect) {
            frameSelect.addEventListener("change", (e) => {
                setFramePreset(e.target.value);
            });
        }

        if (btnRotateFrame) {
            btnRotateFrame.addEventListener("click", () => {
                applyFrameDimensions(canvas.height, canvas.width);
            });
        }

        if (btnToggleBezel) {
            btnToggleBezel.addEventListener("click", () => {
                isBezelActive = !isBezelActive;
                if (deviceMockup) deviceMockup.classList.toggle("with-bezel", isBezelActive);
                btnToggleBezel.textContent = isBezelActive ? "📱 Bezel: ON" : "📱 Bezel: OFF";
            });
        }

        function executeWasmEntry(instance) {
            if (!instance || !instance.exports) return;
            const exportedKeys = Object.keys(instance.exports).filter(k => k !== 'memory');
            let target = null;
            const candidates = [
                instance.exports.render,
                instance.exports.dap_gambar,
                instance.exports.gambar,
                instance.exports.render_game,
                instance.exports.utama,
                instance.exports.gambar_tata_surya,
                instance.exports.perbarui_layar,
                instance.exports.main
            ];
            for (const cand of candidates) {
                if (cand && typeof cand === 'function' && cand.length === 0) {
                    target = cand;
                    break;
                }
            }
            if (!target) {
                for (const key of exportedKeys) {
                    const fn = instance.exports[key];
                    if (typeof fn === 'function' && fn.length === 0) {
                        target = fn;
                        break;
                    }
                }
            }

            if (target) {
                target();
                if (emptyCanvasOverlay) {
                    emptyCanvasOverlay.style.display = commandCount > 0 ? "none" : "flex";
                }
            } else {
                ctx.fillStyle = "#090d16";
                ctx.fillRect(0, 0, canvas.width, canvas.height);
                setStatus("library", "Pustaka Siap");
                if (emptyCanvasOverlay) {
                    emptyCanvasOverlay.style.display = "flex";
                    if (emptyTitle) emptyTitle.textContent = "📦 Modul Logika / Pustaka";
                    if (emptyDesc) emptyDesc.innerHTML = "Berkas <strong>" + (activeFilename ? activeFilename.textContent : "ini") + "</strong> berisi modul fungsi pustaka tanpa siklus render kanvas langsung.<br>Buka berkas <code>src/utama.dap</code> di editor samping untuk menjalankan antarmuka visual kalkulator.";
                }
            }
        }

        function rerunCurrentFrame() {
            if (!currentWasmInstance) return;
            commandCount = 0;
            const tStart = performance.now();
            executeWasmEntry(currentWasmInstance);
            metricRender.textContent = \`\${(performance.now() - tStart).toFixed(2)} ms\`;
            metricCmds.textContent = commandCount.toString();
        }

        function loopFrame() {
            if (!isLooping) return;
            const now = performance.now();
            currentDeltaTime = Math.min((now - lastLoopTime) / 1000.0, 0.1);
            if (currentDeltaTime <= 0) currentDeltaTime = 0.0166;
            lastLoopTime = now;

            rerunCurrentFrame();
            loopReqId = requestAnimationFrame(loopFrame);
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

                executeWasmEntry(instance);

                const tDuration = (performance.now() - tStart).toFixed(2);
                metricRender.textContent = \`\${tDuration} ms\`;
                metricCmds.textContent = commandCount.toString();
                if (statusText && statusText.textContent !== "Pustaka Siap") {
                    setStatus("success", "Berhasil");
                }
                errorCard.style.display = "none";

                if (isLooping && !loopReqId) {
                    lastLoopTime = performance.now();
                    loopReqId = requestAnimationFrame(loopFrame);
                }
            } catch (err) {
                setStatus("error", "Galat Runtime");
                errorCard.textContent = "⚠️ Galat Runtime WASM:\n" + err.message;
                errorCard.style.display = "block";
                if (emptyCanvasOverlay) emptyCanvasOverlay.style.display = "none";
            }
        }

        let currentActiveFilePath = "";

        // Listener pesan dari Extension host
        window.addEventListener("message", (event) => {
            const msg = event.data;
            if (!msg) return;

            if (msg.filepath) {
                currentActiveFilePath = msg.filepath;
            }
            if (msg.filename) {
                activeFilename.textContent = msg.filename;
                if (emptyFilename) emptyFilename.textContent = msg.filename;
            }

            switch (msg.command) {
                case "compiling":
                    setStatus("compiling", "Mengompilasi...");
                    break;
                case "noActiveDoc":
                    setStatus("", "Menunggu Berkas");
                    activeFilename.textContent = "Pilih Berkas .dap";
                    if (emptyCanvasOverlay) emptyCanvasOverlay.style.display = "flex";
                    if (emptyFilename) emptyFilename.textContent = "-";
                    break;
                case "loadWasm": {
                    const binaryStr = atob(msg.wasmBase64);
                    const bytes = new Uint8Array(binaryStr.length);
                    for (let i = 0; i < binaryStr.length; i++) {
                        bytes[i] = binaryStr.charCodeAt(i);
                    }
                    metricSize.textContent = (msg.size / 1024).toFixed(2) + " KB";
                    metricCompile.textContent = (msg.compileTimeMs || 0) + " ms";
                    errorCard.style.display = "none";
                    runWasmBytes(bytes);
                    break;
                }
                case "compileError":
                    setStatus("error", "Galat Kompilasi");
                    errorCard.textContent = "⚠️ Galat Kompilasi:\n" + msg.error;
                    errorCard.style.display = "block";
                    metricCompile.textContent = (msg.compileTimeMs || 0) + " ms";
                    if (emptyCanvasOverlay) emptyCanvasOverlay.style.display = "none";
                    break;
            }
        });

        // Mouse tracking & tombol HUD
        canvas.addEventListener("mousemove", (e) => {
            const rect = canvas.getBoundingClientRect();
            const scaleX = canvas.width / rect.width;
            const scaleY = canvas.height / rect.height;
            canvasMouseX = Math.max(0, Math.min(canvas.width, (e.clientX - rect.left) * scaleX));
            canvasMouseY = Math.max(0, Math.min(canvas.height, (e.clientY - rect.top) * scaleY));
            coordsHud.textContent = \`X: \${Math.round(canvasMouseX)}, Y: \${Math.round(canvasMouseY)}\`;
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
        });

        canvas.addEventListener("mousedown", (e) => {
            canvasMouseDown = true;
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
        });

        canvas.addEventListener("mouseup", (e) => {
            canvasMouseDown = false;
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
        });

        // Touch event support (layar sentuh / stylus / mobile preview)
        function handleTouch(e, isDown) {
            const touch = e.touches[0] || (e.changedTouches && e.changedTouches[0]);
            if (touch) {
                const rect = canvas.getBoundingClientRect();
                const scaleX = canvas.width / rect.width;
                const scaleY = canvas.height / rect.height;
                canvasMouseX = Math.max(0, Math.min(canvas.width, (touch.clientX - rect.left) * scaleX));
                canvasMouseY = Math.max(0, Math.min(canvas.height, (touch.clientY - rect.top) * scaleY));
                coordsHud.textContent = \`X: \${Math.round(canvasMouseX)}, Y: \${Math.round(canvasMouseY)}\`;
            }
            if (isDown !== undefined) {
                canvasMouseDown = isDown;
            }
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
            e.preventDefault();
        }
        canvas.addEventListener("touchstart", (e) => handleTouch(e, true), { passive: false });
        canvas.addEventListener("touchmove", (e) => handleTouch(e), { passive: false });
        canvas.addEventListener("touchend", (e) => handleTouch(e, false), { passive: false });
        canvas.addEventListener("touchcancel", (e) => handleTouch(e, false), { passive: false });

        window.addEventListener("keydown", (e) => {
            canvasKeysPressed.add(e.keyCode || e.which);
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
        });

        window.addEventListener("keyup", (e) => {
            canvasKeysPressed.delete(e.keyCode || e.which);
            if (currentWasmInstance) {
                rerunCurrentFrame();
            }
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
                rerunCurrentFrame();
            } else {
                vscode.postMessage({ command: "requestCompile" });
            }
        });

        btnLoop.addEventListener("click", () => {
            isLooping = !isLooping;
            if (isLooping) {
                btnLoop.classList.add("active");
                btnLoop.textContent = "⏸ Loop: ON";
                lastLoopTime = performance.now();
                currentDeltaTime = 0.0166;
                loopReqId = requestAnimationFrame(loopFrame);
            } else {
                btnLoop.classList.remove("active");
                btnLoop.textContent = "🔁 Loop: OFF";
                if (loopReqId) {
                    cancelAnimationFrame(loopReqId);
                    loopReqId = null;
                }
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

        if (btnToggleGuide && guideCard) {
            btnToggleGuide.addEventListener("click", () => {
                const isHidden = guideCard.style.display === "none";
                guideCard.style.display = isHidden ? "block" : "none";
                btnToggleGuide.classList.toggle("active", isHidden);
            });
            if (btnCloseGuide) {
                btnCloseGuide.addEventListener("click", () => {
                    guideCard.style.display = "none";
                    btnToggleGuide.classList.remove("active");
                });
            }
        }

        if (btnClearCanvas) {
            btnClearCanvas.addEventListener("click", () => {
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                appendLog("[Kanvas dibersihkan]");
            });
        }

        if (btnTemplateMenu && templateMenu) {
            btnTemplateMenu.addEventListener("click", (e) => {
                e.stopPropagation();
                templateMenu.style.display = templateMenu.style.display === "none" ? "block" : "none";
            });
            document.addEventListener("click", (e) => {
                if (!templateMenu.contains(e.target) && e.target !== btnTemplateMenu) {
                    templateMenu.style.display = "none";
                }
            });
            document.querySelectorAll(".template-item").forEach((item) => {
                item.addEventListener("click", (e) => {
                    e.stopPropagation();
                    const key = item.getAttribute("data-preset");
                    const code = PRESETS[key];
                    const title = item.textContent.trim();
                    templateMenu.style.display = "none";
                    vscode.postMessage({
                        command: "openPresetAsNew",
                        presetKey: key,
                        title: title,
                        code: code,
                    });
                });
            });
        }

        const btnMobileExport = document.getElementById("btn-mobile-export");
        if (btnMobileExport) {
            btnMobileExport.addEventListener("click", () => {
                vscode.postMessage({ command: "buildMobile", filePath: currentActiveFilePath });
            });
        }

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
