use dapcodec::debug::DebugServer;
use std::fs;

#[test]
fn test_dap_initialize_protocol() {
    let mut server = DebugServer::new();
    let init_req = r#"{"seq":1,"type":"request","command":"initialize","arguments":{"clientID":"vscode","adapterID":"dapcode"}}"#;
    let resps = server.handle_message(init_req);

    assert_eq!(resps.len(), 2);
    // Response
    assert!(resps[0].contains("\"command\":\"initialize\""));
    assert!(resps[0].contains("\"success\":true"));
    assert!(resps[0].contains("\"supportsConfigurationDoneRequest\":true"));
    assert!(resps[0].contains("\"supportsEvaluateForHovers\":true"));
    // Initialized event
    assert!(resps[1].contains("\"event\":\"initialized\""));
}

#[test]
fn test_dap_set_breakpoints() {
    let mut server = DebugServer::new();

    let bp_req = r#"{"seq":2,"type":"request","command":"setBreakpoints","arguments":{"source":{"path":"C:/project/test.dap"},"breakpoints":[{"line":5},{"line":12}]}}"#;
    let resps = server.handle_message(bp_req);

    assert_eq!(resps.len(), 1);
    assert!(resps[0].contains("\"command\":\"setBreakpoints\""));
    assert!(resps[0].contains("\"success\":true"));
    assert!(resps[0].contains("{\"verified\":true,\"line\":5}"));
    assert!(resps[0].contains("{\"verified\":true,\"line\":12}"));
}

#[test]
fn test_dap_threads() {
    let mut server = DebugServer::new();
    let thread_req = r#"{"seq":3,"type":"request","command":"threads"}"#;
    let resps = server.handle_message(thread_req);

    assert_eq!(resps.len(), 1);
    assert!(resps[0].contains("\"command\":\"threads\""));
    assert!(resps[0].contains("\"name\":\"Utama\""));
}

#[test]
fn test_dap_full_interactive_breakpoint_and_inspection() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("dap_debug_integration_test.dap");

    let code = "fungsi hitung(a: u64, b: u64) -> u64 {\n    hasil := a * b\n    kembali hasil\n}\n\nfungsi utama() {\n    x := 15\n    y := 3\n    z := hitung(x, y)\n    cetak(z)\n}\n";
    fs::write(&test_file, code).expect("Gagal menulis berkas uji DAP");

    let file_path_str = test_file.to_string_lossy().to_string();

    let mut server = DebugServer::new();

    // 1. Initialize
    let init_req = r#"{"seq":1,"type":"request","command":"initialize","arguments":{"clientID":"vscode","adapterID":"dapcode"}}"#;
    server.handle_message(init_req);

    // 2. Launch
    let launch_req = format!(
        r#"{{"seq":2,"type":"request","command":"launch","arguments":{{"program":"{}","stopOnEntry":false}}}}"#,
        file_path_str.replace('\\', "/")
    );
    let launch_resp = server.handle_message(&launch_req);
    assert!(launch_resp[0].contains("\"success\":true"));

    // 3. Set breakpoint on line 8 (`y := 3`)
    let bp_req = format!(
        r#"{{"seq":3,"type":"request","command":"setBreakpoints","arguments":{{"source":{{"path":"{}"}},"breakpoints":[{{"line":8}}]}}}}"#,
        file_path_str.replace('\\', "/")
    );
    let bp_resp = server.handle_message(&bp_req);
    assert!(bp_resp[0].contains("\"verified\":true,\"line\":8"));

    // 4. Configuration done (starts execution thread)
    let config_req = r#"{"seq":4,"type":"request","command":"configurationDone"}"#;
    server.handle_message(config_req);

    // Give worker thread a moment to reach breakpoint
    std::thread::sleep(std::time::Duration::from_millis(150));

    // 5. Stack trace
    let stack_req = r#"{"seq":5,"type":"request","command":"stackTrace","arguments":{"threadId":1}}"#;
    let stack_resp = server.handle_message(stack_req);
    assert!(!stack_resp.is_empty());
    assert!(stack_resp[0].contains("\"line\":8"));
    assert!(stack_resp[0].contains("\"name\":\"utama\""));

    // 6. Scopes & Variables
    let scopes_req = r#"{"seq":6,"type":"request","command":"scopes","arguments":{"frameId":1}}"#;
    let scopes_resp = server.handle_message(scopes_req);
    assert!(scopes_resp[0].contains("\"name\":\"Lokal\""));

    let vars_req = r#"{"seq":7,"type":"request","command":"variables","arguments":{"variablesReference":1000}}"#;
    let vars_resp = server.handle_message(vars_req);
    assert!(vars_resp[0].contains("\"name\":\"x\""));
    assert!(vars_resp[0].contains("\"value\":\"15\""));
    assert!(vars_resp[0].contains("\"type\":\"u64\""));

    // 7. Evaluate expression (e.g. hover over `x`)
    let eval_req = r#"{"seq":8,"type":"request","command":"evaluate","arguments":{"expression":"x","frameId":1}}"#;
    let eval_resp = server.handle_message(eval_req);
    assert!(eval_resp[0].contains("\"result\":\"15\""));
    assert!(eval_resp[0].contains("\"type\":\"u64\""));

    // 8. Next (step over to line 9)
    let next_req = r#"{"seq":9,"type":"request","command":"next","arguments":{"threadId":1}}"#;
    let next_resp = server.handle_message(next_req);
    assert!(next_resp[0].contains("\"command\":\"next\""));
    assert!(next_resp[0].contains("\"success\":true"));

    std::thread::sleep(std::time::Duration::from_millis(150));

    // Variables after stepping: both x and y should now be visible!
    let vars_resp2 = server.handle_message(vars_req);
    assert!(vars_resp2[0].contains("\"name\":\"x\""));
    assert!(vars_resp2[0].contains("\"name\":\"y\""));
    assert!(vars_resp2[0].contains("\"value\":\"3\""));

    // 9. Continue to end
    let cont_req = r#"{"seq":10,"type":"request","command":"continue","arguments":{"threadId":1}}"#;
    let cont_resp = server.handle_message(cont_req);
    assert!(cont_resp[0].contains("\"success\":true"));

    std::thread::sleep(std::time::Duration::from_millis(150));

    // Clean up temp file
    let _ = fs::remove_file(test_file);
}

#[derive(Clone, Default)]
struct SharedBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[test]
fn test_cli_debugger_stepping_and_breakpoints() {
    use dapcodec::debug::CliDebugger;
    use dapcodec::eval::Evaluator;
    use dapcodec::lexer::Lexer;
    use dapcodec::parser::Parser;
    use dapcodec::span::SourceMap;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    let code = "fungsi kali(a: u64, b: u64) -> u64 {\n    hasil := a * b\n    kembali hasil\n}\n\nfungsi utama() {\n    p := 10\n    q := 20\n    r := kali(p, q)\n    cetak(r)\n}\n";
    let file_path = "D:/dapcode-lang/tests/fixtures/sample_debug.dap";

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file(file_path, code.to_string());

    let mut lexer = Lexer::new(code, file_id);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing gagal");

    // Rangkaian perintah debugger simulasi pengguna:
    // 1. Berhenti di awal eksekusi (stop-on-entry di baris 7)
    // 2. b 9      -> pasang breakpoint pada baris 9 ('r := kali(p, q)')
    // 3. c        -> terus jalan sampai breakpoint baris 9
    // 4. v        -> lihat semua variabel lokal (p=10, q=20)
    // 5. p p + q  -> evaluasi ekspresi dinamis (30)
    // 6. s        -> langkah masuk ke fungsi 'kali'
    // 7. bt       -> periksa jejak panggilan stack
    // 8. out      -> keluar dari fungsi kali
    // 9. c        -> lanjutkan eksekusi hingga akhir program
    let input_commands = "b 9\nc\nv\np p + q\ns\nbt\nout\nc\n";
    let reader = Box::new(Cursor::new(input_commands.as_bytes().to_vec()));

    let output_store = Arc::new(Mutex::new(Vec::new()));
    let writer = Box::new(SharedBuffer(Arc::clone(&output_store)));

    let sm_arc = Arc::new(Mutex::new(sm));
    let dbg = CliDebugger::new(
        sm_arc,
        file_path.to_string(),
        vec![],
        true,  // stop_on_entry true agar berhenti di awal dan menerima perintah
        reader,
        writer,
    );

    let mut evaluator = Evaluator::new();
    evaluator.set_debug_hook(Arc::new(Mutex::new(dbg)));

    let result = evaluator.run_program(&program);
    assert!(result.is_ok(), "Evaluator harus menyelesaikan eksekusi program");

    let output_bytes = output_store.lock().unwrap().clone();
    let output_str = String::from_utf8_lossy(&output_bytes);

    // Verifikasi fitur debugger:
    // 1. Titik henti ditambahkan
    assert!(output_str.contains("Titik henti ditambahkan"), "Harus menampilkan konfirmasi breakpoint");
    // 2. Berhenti pada baris 9
    assert!(output_str.contains("Berhenti karena titik henti pada baris 9"), "Harus berhenti di baris 9");
    // 3. Inspeksi variabel lokal
    assert!(output_str.contains("p (u64) = 10"), "Variabel p harus tercetak");
    assert!(output_str.contains("q (u64) = 20"), "Variabel q harus tercetak");
    // 4. Evaluasi ekspresi
    assert!(output_str.contains("p + q (u64) = 30"), "Ekspresi p + q harus bernilai 30");
    // 5. Langkah masuk & backtrace
    assert!(output_str.contains("Jejak Panggilan (Call Stack):"), "Call stack harus tercetak");
    assert!(output_str.contains("kali"), "Call stack harus memuat fungsi kali");
    assert!(output_str.contains("utama"), "Call stack harus memuat fungsi utama");
}

#[test]
fn test_cli_debugger_stop_on_entry_and_quit() {
    use dapcodec::debug::CliDebugger;
    use dapcodec::eval::Evaluator;
    use dapcodec::lexer::Lexer;
    use dapcodec::parser::Parser;
    use dapcodec::span::SourceMap;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    let code = "fungsi utama() {\n    a := 100\n    cetak(a)\n}\n";
    let file_path = "D:/dapcode-lang/tests/fixtures/entry_quit.dap";

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file(file_path, code.to_string());

    let mut lexer = Lexer::new(code, file_id);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing gagal");

    // Perintah: berhenti di awal, lalu ketik 'q' untuk berhenti
    let input_commands = "q\n";
    let reader = Box::new(Cursor::new(input_commands.as_bytes().to_vec()));

    let output_store = Arc::new(Mutex::new(Vec::new()));
    let writer = Box::new(SharedBuffer(Arc::clone(&output_store)));

    let sm_arc = Arc::new(Mutex::new(sm));
    let dbg = CliDebugger::new(
        sm_arc,
        file_path.to_string(),
        vec![],
        true, // stop_on_entry true
        reader,
        writer,
    );

    let mut evaluator = Evaluator::new();
    evaluator.set_debug_hook(Arc::new(Mutex::new(dbg)));

    let result = evaluator.run_program(&program);
    // Program harus berhenti karena debugger menterminasi eksekusi
    assert!(result.is_err(), "Harus menghasilkan terminasi dari debugger");
    let err = result.err().unwrap();
    assert_eq!(err.message, "Eksekusi dihentikan oleh debugger");

    let output_bytes = output_store.lock().unwrap().clone();
    let output_str = String::from_utf8_lossy(&output_bytes);
    assert!(output_str.contains("henti awal (stop-on-entry)"));
    assert!(output_str.contains("Sesi awakutu dihentikan."));
}

