use dapcodec::lsp::LspServer;

#[test]
fn test_lsp_initialize_and_capabilities() {
    let mut server = LspServer::new();
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    let resp = server.process_message(init_req);
    assert!(resp.is_some());
    let resp_str = resp.unwrap();
    assert!(resp_str.contains("\"hoverProvider\":true"));
    assert!(resp_str.contains("\"documentFormattingProvider\":true"));
    assert!(resp_str.contains("\"completionProvider\""));
}

#[test]
fn test_lsp_did_open_and_diagnostics() {
    let mut server = LspServer::new();
    // Valid code
    let valid_code = r#"fungsi utama() {\n    cetak(\"Halo DapCode\")\n}"#;
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///test.dap","text":"{}"}}}}}}"#,
        valid_code
    );
    let notif = server.process_message(&open_msg);
    assert!(notif.is_some());
    let notif_str = notif.unwrap();
    assert!(notif_str.contains("textDocument/publishDiagnostics"));
    assert!(notif_str.contains("\"diagnostics\":[]")); // No errors

    // Invalid code with type error
    let invalid_code = "fungsi uji_tipe() {\\n    x: u64 = \\\"bukan angka\\\"\\n}";
    let change_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didChange","params":{{"textDocument":{{"uri":"file:///test.dap"}},"contentChanges":[{{"text":"{}"}}]}}}}"#,
        invalid_code
    );
    let diag_notif = server.process_message(&change_msg);
    assert!(diag_notif.is_some());
    let diag_str = diag_notif.unwrap();
    assert!(diag_str.contains("Ketidakcocokan"));
}

#[test]
fn test_lsp_hover_builtin_and_symbols() {
    let mut server = LspServer::new();
    let code = r#"fungsi utama() {\n    teks := ke_json(123)\n}"#;
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///test.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    // Hover over ke_json (line 1, col 12)
    let hover_req = r#"{"jsonrpc":"2.0","id":10,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///test.dap"},"position":{"line":1,"character":12}}}"#;
    let resp = server.process_message(hover_req);
    assert!(resp.is_some());
    let resp_str = resp.unwrap();
    assert!(resp_str.contains("fungsi ke_json"));
    assert!(resp_str.contains("Serialisasi nilai primitif"));
}

#[test]
fn test_lsp_completions() {
    let mut server = LspServer::new();
    let code = r#"struktur Mahasiswa {\n    nama: Teks,\n}"#;
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///test.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    let comp_req = r#"{"jsonrpc":"2.0","id":20,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.dap"},"position":{"line":0,"character":0}}}"#;
    let resp = server.process_message(comp_req);
    assert!(resp.is_some());
    let resp_str = resp.unwrap();
    assert!(resp_str.contains("\"label\":\"struktur\""));
    assert!(resp_str.contains("\"label\":\"ke_json\""));
    assert!(resp_str.contains("\"label\":\"jalan_paralel\""));
    assert!(resp_str.contains("\"label\":\"Mahasiswa\""));
}
