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
