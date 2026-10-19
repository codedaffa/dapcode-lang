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
    assert!(resp_str.contains("\"definitionProvider\":true"));
    assert!(resp_str.contains("\"documentSymbolProvider\":true"));
    assert!(resp_str.contains("\"codeActionProvider\":true"));
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

#[test]
fn test_lsp_go_to_definition_local_vars_and_params() {
    let mut server = LspServer::new();
    let code = "fungsi hitung(angka: u64) -> u64 {\\n    faktor := 2\\n    hasil := angka * faktor\\n    kembali hasil\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///def_test.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    // Jump to 'angka' param on line 2 col 14 ("angka * faktor")
    let def_req = r#"{"jsonrpc":"2.0","id":30,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///def_test.dap"},"position":{"line":2,"character":14}}}"#;
    let resp = server.process_message(def_req).unwrap();
    assert!(resp.contains("\"uri\":\"file:///def_test.dap\""));
    assert!(resp.contains("\"start\":{\"line\":0"));

    // Jump to 'faktor' var on line 2 col 22 ("angka * faktor")
    let def_req2 = r#"{"jsonrpc":"2.0","id":31,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///def_test.dap"},"position":{"line":2,"character":22}}}"#;
    let resp2 = server.process_message(def_req2).unwrap();
    assert!(resp2.contains("\"uri\":\"file:///def_test.dap\""));
    assert!(resp2.contains("\"start\":{\"line\":1"));

    // Jump to 'hasil' on line 3 col 13 ("kembali hasil")
    let def_req3 = r#"{"jsonrpc":"2.0","id":32,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///def_test.dap"},"position":{"line":3,"character":13}}}"#;
    let resp3 = server.process_message(def_req3).unwrap();
    assert!(resp3.contains("\"uri\":\"file:///def_test.dap\""));
    assert!(resp3.contains("\"start\":{\"line\":2"));
}

#[test]
fn test_lsp_go_to_definition_struct_and_fn() {
    let mut server = LspServer::new();
    let code = "struktur Titik {\\n    x: f64\\n    y: f64\\n}\\n\\nfungsi buat_titik(px: f64, py: f64) -> Titik {\\n    kembali Titik(x: px, y: py)\\n}\\n\\nfungsi utama() {\\n    p := buat_titik(1.0, 2.0)\\n    cetak(p.x)\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///struct_test.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    // Jump to 'buat_titik' in 'p := buat_titik(1.0, 2.0)' on line 10 col 12
    let def_req = r#"{"jsonrpc":"2.0","id":40,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///struct_test.dap"},"position":{"line":10,"character":12}}}"#;
    let resp = server.process_message(def_req).unwrap();
    assert!(resp.contains("\"uri\":\"file:///struct_test.dap\""));
    assert!(resp.contains("\"start\":{\"line\":5"));

    // Jump to 'Titik' in return type on line 5 col 42
    let def_req2 = r#"{"jsonrpc":"2.0","id":41,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///struct_test.dap"},"position":{"line":5,"character":42}}}"#;
    let resp2 = server.process_message(def_req2).unwrap();
    assert!(resp2.contains("\"uri\":\"file:///struct_test.dap\""));
    assert!(resp2.contains("\"start\":{\"line\":0"));

    // Jump to struct field 'x' on line 11 col 12 ('cetak(p.x)')
    let def_req3 = r#"{"jsonrpc":"2.0","id":42,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///struct_test.dap"},"position":{"line":11,"character":12}}}"#;
    let resp3 = server.process_message(def_req3).unwrap();
    assert!(resp3.contains("\"uri\":\"file:///struct_test.dap\""));
    assert!(resp3.contains("\"start\":{\"line\":1"));
}

#[test]
fn test_lsp_go_to_definition_cross_file_and_import() {
    let mut server = LspServer::new();
    let geo_code = "fungsi hitung_luas(sisi: f64) -> f64 {\\n    kembali sisi * sisi\\n}";
    let open_geo = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///geometri.dap","text":"{}"}}}}}}"#,
        geo_code
    );
    server.process_message(&open_geo);

    let main_code = "impor \\\"geometri.dap\\\"\\n\\nfungsi jalankan() {\\n    l := hitung_luas(5.0)\\n    cetak(l)\\n}";
    let open_main = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///main.dap","text":"{}"}}}}}}"#,
        main_code
    );
    server.process_message(&open_main);

    // Jump to import statement on line 0 col 6
    let def_imp = r#"{"jsonrpc":"2.0","id":50,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///main.dap"},"position":{"line":0,"character":6}}}"#;
    let resp_imp = server.process_message(def_imp).unwrap();
    assert!(resp_imp.contains("geometri.dap"));

    // Jump to 'hitung_luas' symbol on line 3 col 12 -> points to file:///geometri.dap
    let def_sym = r#"{"jsonrpc":"2.0","id":51,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///main.dap"},"position":{"line":3,"character":12}}}"#;
    let resp_sym = server.process_message(def_sym).unwrap();
    assert!(resp_sym.contains("geometri.dap"));
    assert!(resp_sym.contains("\"start\":{\"line\":0"));
}

#[test]
fn test_lsp_document_symbols_outline_hierarchy() {
    let mut server = LspServer::new();
    let code = "impor \\\"modul.dap\\\"\\n\\nstruktur Mahasiswa {\\n    nama: Teks\\n    nim: u64\\n}\\n\\nsifat Pencerita {\\n    fungsi cerita(ini) -> Teks\\n}\\n\\nterapkan Pencerita untuk Mahasiswa {\\n    fungsi cerita(ini) -> Teks {\\n        kembali ini.nama\\n    }\\n}\\n\\nfungsi salam(m: Mahasiswa) -> Teks {\\n    kembali m.cerita()\\n}\\n\\nuji \\\"uji_mhs\\\" {\\n    pastikan(benar)\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///outline.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    let sym_req = r#"{"jsonrpc":"2.0","id":60,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///outline.dap"}}}"#;
    let resp = server.process_message(sym_req).unwrap();

    // Check imports
    assert!(resp.contains("\"name\":\"impor \\\"modul.dap\\\"\""));
    assert!(resp.contains("\"kind\":3"));

    // Check struct and its fields as children
    assert!(resp.contains("\"name\":\"Mahasiswa\""));
    assert!(resp.contains("\"detail\":\"struktur\""));
    assert!(resp.contains("\"kind\":23"));
    assert!(resp.contains("\"name\":\"nama\""));
    assert!(resp.contains("\"detail\":\"Teks\""));
    assert!(resp.contains("\"name\":\"nim\""));
    assert!(resp.contains("\"detail\":\"u64\""));

    // Check trait
    assert!(resp.contains("\"name\":\"Pencerita\""));
    assert!(resp.contains("\"detail\":\"sifat\""));
    assert!(resp.contains("\"kind\":11"));

    // Check impl block and its method
    assert!(resp.contains("\"name\":\"terapkan Pencerita untuk Mahasiswa\""));
    assert!(resp.contains("\"detail\":\"implementasi\""));
    assert!(resp.contains("\"kind\":5"));
    assert!(resp.contains("\"name\":\"cerita\""));
    assert!(resp.contains("\"kind\":6"));

    // Check function
    assert!(resp.contains("\"name\":\"salam\""));
    assert!(resp.contains("\"kind\":12"));

    // Check unit test
    assert!(resp.contains("\"name\":\"uji \\\"uji_mhs\\\"\""));
    assert!(resp.contains("\"detail\":\"pengujian unit\""));
}

#[test]
fn test_lsp_code_action_typo_math() {
    let mut server = LspServer::new();
    let code = "fungsi utama() {\\n    h := akar_kuadratt(16.0)\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///action_typo.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    let ca_req = r#"{"jsonrpc":"2.0","id":70,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///action_typo.dap"},"range":{"start":{"line":1,"character":9},"end":{"line":1,"character":22}},"context":{"diagnostics":[]}}}"#;
    let resp = server.process_message(ca_req).unwrap();
    assert!(resp.contains("Perbaiki ejaan menjadi 'akar_kuadrat'"));
    assert!(resp.contains("\"kind\":\"quickfix\""));
    assert!(resp.contains("\"newText\":\"akar_kuadrat\""));
    assert!(resp.contains("\"isPreferred\":true"));
}

#[test]
fn test_lsp_code_action_assignment_operator_correction() {
    let mut server = LspServer::new();
    let code = "fungsi utama() {\\n    nilai = 100\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///action_assign.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    let ca_req = r#"{"jsonrpc":"2.0","id":71,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///action_assign.dap"},"range":{"start":{"line":1,"character":4},"end":{"line":1,"character":9}},"context":{"diagnostics":[]}}}"#;
    let resp = server.process_message(ca_req).unwrap();
    assert!(resp.contains("Ganti '=' dengan ':='"));
    assert!(resp.contains("\"newText\":\":=\""));
}

#[test]
fn test_lsp_code_action_auto_stub_function() {
    let mut server = LspServer::new();
    let code = "fungsi utama() {\\n    hitung_total_belanja(10)\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///action_stub.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    let ca_req = r#"{"jsonrpc":"2.0","id":72,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///action_stub.dap"},"range":{"start":{"line":1,"character":4},"end":{"line":1,"character":24}},"context":{"diagnostics":[]}}}"#;
    let resp = server.process_message(ca_req).unwrap();
    assert!(resp.contains("Buat stub fungsi 'hitung_total_belanja()'"));
    assert!(resp.contains("fungsi hitung_total_belanja()"));
}

#[test]
fn test_lsp_code_action_missing_utama() {
    let mut server = LspServer::new();
    let code = "fungsi pembantu() {\\n    cetak(\\\"Halo\\\")\\n}";
    let open_msg = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///action_utama.dap","text":"{}"}}}}}}"#,
        code
    );
    server.process_message(&open_msg);

    // Provide diagnostic explicitly in context
    let ca_req = r#"{"jsonrpc":"2.0","id":73,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///action_utama.dap"},"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":5}},"context":{"diagnostics":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":5}},"message":"Fungsi 'utama()' tidak ditemukan"}]}}}"#;
    let resp = server.process_message(ca_req).unwrap();
    assert!(resp.contains("Tambahkan fungsi 'utama()'"));
    assert!(resp.contains("fungsi utama()"));
}

#[test]
fn test_lsp_diagnostics_with_imported_modules() {
    let mut server = LspServer::new();
    let kalkulator_path = std::path::Path::new("examples/kalkulator_mobile_app/src/utama.dap");
    if kalkulator_path.exists() {
        let abs_path = std::fs::canonicalize(kalkulator_path).unwrap();
        let uri = LspServer::path_to_uri(&abs_path.to_string_lossy());
        let text = std::fs::read_to_string(&abs_path).unwrap();
        let open_msg = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{}","text":"{}"}}}}}}"#,
            uri,
            text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "")
        );
        let notif = server.process_message(&open_msg);
        assert!(notif.is_some());
        let notif_str = notif.unwrap();
        assert!(
            notif_str.contains("\"diagnostics\":[]"),
            "Expected 0 diagnostics for kalkulator_mobile_app/src/utama.dap with imports, got: {}",
            notif_str
        );
    }
}

