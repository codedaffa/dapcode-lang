use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn run_dapcode(src: &str) -> Result<Value, String> {
    let mut lexer = Lexer::new(src, 0);
    let tokens = lexer.tokenize().map_err(|d| d.message)?;
    let mut parser = Parser::new(tokens, 0);
    let program = parser.parse_program().map_err(|d| d.message)?;
    let mut checker = TypeChecker::new(0);
    checker.check_program(&program).map_err(|d| d.message)?;
    let mut eval = Evaluator::new();
    eval.run_program(&program).map_err(|d| d.message)
}

fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    thread::sleep(Duration::from_millis(50));
    port
}

#[test]
fn test_websocket_primitives_and_handshake() {
    let code = r#"
fungsi utama() -> Teks {
    // 1. SHA-1 standar (RFC 3174)
    h := sha1("abc")
    pastikan_sama(h, "a9993e364706816aba3e25717850c26c9cd0d89d")

    // 2. Kunci WebSocket standar RFC 6455 Section 1.3
    // Kunci klien: dGhlIHNhbXBsZSBub25jZQ==
    // Kunci accept yang diharapkan: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
    kunci_klien := "dGhlIHNhbXBsZSBub25jZQ=="
    kunci_acc := kunci_websocket(kunci_klien)
    pastikan_sama(kunci_acc, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")

    // 3. Respons Upgrade WebSocket
    resp := respons_upgrade_websocket(kunci_klien) ? tangkap err {
        kembali "gagal_upgrade: " + err
    }
    pastikan_sama(resp.status, 101)

    kembali "sukses_primitif_ws"
}
"#;

    let res = run_dapcode(code).expect("Evaluasi primitif WebSocket harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses_primitif_ws"),
        other => panic!("Diharapkan string sukses_primitif_ws, didapat {:?}", other),
    }
}

#[test]
fn test_live_websocket_server_echo() {
    let port = get_free_port();

    let server_code = format!(r#"
fungsi balas_pesan(pesan: Teks) -> Teks {{
    kembali "DAPCODE_BALAS: " + pesan
}}

fungsi tangani_web(req: PermintaanHttp) -> ResponsHttp {{
    jika req.adalah_websocket() {{
        kembali req.tingkatkan_websocket("balas_pesan") ? tangkap err {{
            kirim_respons_http(400, "text/plain", "gagal_upgrade")
        }}
    }}
    kembali kirim_respons_http(200, "text/plain", "halaman_biasa")
}}

fungsi utama() -> u64 {{
    hasil := buat_server_http({port}, "tangani_web", 1) ? tangkap err {{
        kembali 0
    }}
    kembali hasil
}}
"#);

    let server_handle = thread::spawn(move || {
        run_dapcode(&server_code).expect("Server DapCode live echo harus berjalan sukses");
    });

    thread::sleep(Duration::from_millis(250));

    // Connect via raw TCP client
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Gagal terhubung ke WebSocket server DapCode");
    stream.set_read_timeout(Some(Duration::from_secs(4))).unwrap();

    // 1. Send WebSocket Upgrade Request
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let upgrade_req = format!(
        "GET /chat HTTP/1.1\r\n\
         Host: 127.0.0.1:{}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        port, client_key
    );
    stream.write_all(upgrade_req.as_bytes()).unwrap();

    // 2. Read HTTP 101 Switching Protocols response
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 101 Switching Protocols"));
    assert!(resp_str.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

    // 3. Send masked RFC 6455 Text Frame: "HaloDapCode"
    let text_payload = b"HaloDapCode";
    let mask = [0x12, 0x34, 0x56, 0x78];
    let mut masked_payload = Vec::new();
    for (i, &b) in text_payload.iter().enumerate() {
        masked_payload.push(b ^ mask[i % 4]);
    }

    let mut frame = Vec::new();
    frame.push(0x81); // FIN (0x80) | Text (0x01)
    frame.push(0x80 | (text_payload.len() as u8)); // Masked (0x80) | Len
    frame.extend_from_slice(&mask);
    frame.extend_from_slice(&masked_payload);

    stream.write_all(&frame).unwrap();

    // 4. Read server's unmasked echo response
    let mut resp_header = [0u8; 2];
    stream.read_exact(&mut resp_header).unwrap();
    assert_eq!(resp_header[0], 0x81, "Harus text frame unmasked dari server");
    let resp_len = (resp_header[1] & 0x7F) as usize;

    let mut resp_body = vec![0u8; resp_len];
    stream.read_exact(&mut resp_body).unwrap();
    let resp_text = String::from_utf8_lossy(&resp_body);
    assert_eq!(resp_text, "DAPCODE_BALAS: HaloDapCode");

    // 5. Send Ping Frame (0x89) with payload "ping"
    let ping_payload = b"ping";
    let mut ping_frame = vec![0x89, 0x80 | (ping_payload.len() as u8)];
    ping_frame.extend_from_slice(&mask);
    for (i, &b) in ping_payload.iter().enumerate() {
        ping_frame.push(b ^ mask[i % 4]);
    }
    stream.write_all(&ping_frame).unwrap();

    // Read Pong Frame (0x8A)
    let mut pong_header = [0u8; 2];
    stream.read_exact(&mut pong_header).unwrap();
    assert_eq!(pong_header[0], 0x8A, "Harus pong frame balasan dari server");
    let pong_len = (pong_header[1] & 0x7F) as usize;
    let mut pong_body = vec![0u8; pong_len];
    stream.read_exact(&mut pong_body).unwrap();
    assert_eq!(&pong_body, ping_payload);

    // 6. Send Close Frame (0x88)
    let close_frame = vec![0x88, 0x80, mask[0], mask[1], mask[2], mask[3]];
    stream.write_all(&close_frame).unwrap();

    let _ = server_handle.join();
}

#[test]
fn test_standalone_websocket_server() {
    let port = get_free_port();

    let server_code = format!(r#"
fungsi tangani_pesan(pesan: Teks) -> Teks {{
    kembali "STANDALONE_ECHO: " + pesan
}}

fungsi utama() -> u64 {{
    hasil := buat_server_websocket({port}, "tangani_pesan", 1) ? tangkap err {{
        kembali 0
    }}
    kembali hasil
}}
"#);

    let server_handle = thread::spawn(move || {
        run_dapcode(&server_code).expect("Standalone server DapCode harus berjalan sukses");
    });

    thread::sleep(Duration::from_millis(250));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Gagal terhubung ke standalone WebSocket server");
    stream.set_read_timeout(Some(Duration::from_secs(4))).unwrap();

    // Handshake
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let upgrade_req = format!(
        "GET / HTTP/1.1\r\n\
         Host: 127.0.0.1:{}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        port, client_key
    );
    stream.write_all(upgrade_req.as_bytes()).unwrap();

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 101 Switching Protocols"));

    // Send masked frame: "TesDapCode"
    let text = b"TesDapCode";
    let mask = [0xAA, 0xBB, 0xCC, 0xDD];
    let mut frame = vec![0x81, 0x80 | (text.len() as u8)];
    frame.extend_from_slice(&mask);
    for (i, &b) in text.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    stream.write_all(&frame).unwrap();

    // Receive reply
    let mut resp_header = [0u8; 2];
    stream.read_exact(&mut resp_header).unwrap();
    assert_eq!(resp_header[0], 0x81);
    let resp_len = (resp_header[1] & 0x7F) as usize;
    let mut resp_body = vec![0u8; resp_len];
    stream.read_exact(&mut resp_body).unwrap();
    assert_eq!(String::from_utf8_lossy(&resp_body), "STANDALONE_ECHO: TesDapCode");

    // Close
    let close_frame = vec![0x88, 0x80, mask[0], mask[1], mask[2], mask[3]];
    stream.write_all(&close_frame).unwrap();

    let _ = server_handle.join();
}

#[test]
fn test_native_c_websocket_primitives() {
    let out_dir = std::env::temp_dir().join(format!("dapcode_ws_c_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&out_dir).unwrap();

    let src_path = out_dir.join("main.dap");
    let bin_path = if cfg!(windows) {
        out_dir.join("main.exe")
    } else {
        out_dir.join("main")
    };

    let src = r#"
fungsi utama() {
    h := sha1("abc")
    cetak("SHA1:{h}")

    kunci := kunci_websocket("dGhlIHNhbXBsZSBub25jZQ==")
    cetak("KUNCI:{kunci}")

    resp := respons_upgrade_websocket("dGhlIHNhbXBsZSBub25jZQ==") ? tangkap err {
        cetak("GAGAL")
        kembali
    }
    cetak("STATUS:{resp.status}")
}
"#;

    std::fs::write(&src_path, src).unwrap();

    let mut sm = dapcodec::span::SourceMap::new();
    let (file_id, _) = sm.add_file(src_path.to_str().unwrap(), src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, file_id);
    let prog = parser.parse_program().unwrap();

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&prog).unwrap();

    let ownership = dapcodec::ownership::OwnershipAnalyzer::new().analyze_program(&prog);
    let mut codegen = dapcodec::codegen::CodeGenerator::new(&ownership);
    codegen.compile_to_native(&prog, &bin_path).expect("Kompilasi native C WebSocket harus berhasil");

    let run_output = std::process::Command::new(&bin_path)
        .output()
        .expect("Gagal menjalankan binary native C");
    let stdout = String::from_utf8_lossy(&run_output.stdout);

    assert!(stdout.contains("SHA1:a9993e364706816aba3e25717850c26c9cd0d89d"));
    assert!(stdout.contains("KUNCI:s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
    assert!(stdout.contains("STATUS:101"));

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_native_c_live_websocket_server() {
    let out_dir = std::env::temp_dir().join(format!("dapcode_ws_c_live_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&out_dir).unwrap();

    let port = get_free_port();
    let src_path = out_dir.join("main.dap");
    let bin_path = if cfg!(windows) {
        out_dir.join("main.exe")
    } else {
        out_dir.join("main")
    };

    let src = format!(r#"
fungsi balas_c(pesan: Teks) -> Teks {{
    kembali "C_ECHO:{{pesan}}"
}}

fungsi tangani_c(req: PermintaanHttp) -> ResponsHttp {{
    jika req.adalah_websocket() {{
        kembali req.tingkatkan_websocket("balas_c") ? tangkap err {{
            kirim_respons_http(400, "text/plain", "gagal")
        }}
    }}
    kembali kirim_respons_http(200, "text/plain", "biasa")
}}

fungsi utama() -> u64 {{
    hasil := buat_server_http({port}, "tangani_c", 1) ? tangkap err {{
        kembali 0
    }}
    kembali hasil
}}
"#);

    std::fs::write(&src_path, &src).unwrap();

    let mut sm = dapcodec::span::SourceMap::new();
    let (file_id, _) = sm.add_file(src_path.to_str().unwrap(), src.clone());
    let mut lexer = Lexer::new(&src, file_id);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, file_id);
    let prog = parser.parse_program().unwrap();

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&prog).unwrap();

    let ownership = dapcodec::ownership::OwnershipAnalyzer::new().analyze_program(&prog);
    let mut codegen = dapcodec::codegen::CodeGenerator::new(&ownership);
    codegen.compile_to_native(&prog, &bin_path).expect("Kompilasi native C WebSocket live server harus berhasil");

    let mut child = std::process::Command::new(&bin_path)
        .spawn()
        .expect("Gagal menjalankan server native C");

    thread::sleep(Duration::from_millis(300));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Gagal terhubung ke native C WebSocket server");
    stream.set_read_timeout(Some(Duration::from_secs(4))).unwrap();

    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let upgrade_req = format!(
        "GET /ws HTTP/1.1\r\n\
         Host: 127.0.0.1:{}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        port, client_key
    );
    stream.write_all(upgrade_req.as_bytes()).unwrap();

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 101 Switching Protocols"));
    assert!(resp_str.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

    let text = b"NativeTest";
    let mask = [0x11, 0x22, 0x33, 0x44];
    let mut frame = vec![0x81, 0x80 | (text.len() as u8)];
    frame.extend_from_slice(&mask);
    for (i, &b) in text.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    stream.write_all(&frame).unwrap();

    let mut resp_header = [0u8; 2];
    stream.read_exact(&mut resp_header).unwrap();
    assert_eq!(resp_header[0], 0x81);
    let resp_len = (resp_header[1] & 0x7F) as usize;
    let mut resp_body = vec![0u8; resp_len];
    stream.read_exact(&mut resp_body).unwrap();
    assert_eq!(String::from_utf8_lossy(&resp_body), "C_ECHO:NativeTest");

    let close_frame = vec![0x88, 0x80, mask[0], mask[1], mask[2], mask[3]];
    stream.write_all(&close_frame).unwrap();

    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&out_dir);
}
