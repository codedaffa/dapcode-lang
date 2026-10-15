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
    // Brief sleep to ensure OS releases socket binding
    thread::sleep(Duration::from_millis(50));
    port
}

#[test]
fn test_http_response_primitives() {
    let code = r#"
fungsi utama() -> Teks {
    r1 := respons_json("\{\"status\":\"ok\"\}")
    r2 := respons_teks("Halo DapCode!")
    r3 := kirim_respons_http(201, "application/xml", "<data/>")

    pastikan_sama(r1.status, 200)
    pastikan_sama(r1.tipe_konten, "application/json")
    pastikan_sama(r1.konten, "\{\"status\":\"ok\"\}")

    pastikan_sama(r2.status, 200)
    pastikan_sama(r2.tipe_konten, "text/plain; charset=utf-8")
    pastikan_sama(r2.konten, "Halo DapCode!")

    pastikan_sama(r3.status, 201)
    pastikan_sama(r3.tipe_konten, "application/xml")
    pastikan_sama(r3.konten, "<data/>")

    kembali "sukses"
}
"#;

    let res = run_dapcode(code).expect("Evaluasi primitif respons HTTP harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses"),
        other => panic!("Diharapkan string sukses, didapat {:?}", other),
    }
}

#[test]
fn test_http_server_struct_handler_routes() {
    let port = get_free_port();
    let json_resp = r#"\{\"status\":\"aktif\",\"versi\":\"1.0\"\}"#;

    let server_code = format!(
        r#"
fungsi api_handler(req: PermintaanHttp) -> ResponsHttp {{
    jika req.jalur == "/api/status" {{
        kembali respons_json("{json_resp}")
    }}
    jika req.jalur == "/api/halo" {{
        kembali respons_teks("Halo dari DapCode HTTP Server!")
    }}
    kembali kirim_respons_http(404, "text/plain", "Tidak Ditemukan")
}}

fungsi utama() -> u64 {{
    hasil := buat_server_http({port}, "api_handler", 2) ? tangkap err {{
        cetak("Error server: {port}")
        kembali 0
    }}
    kembali hasil
}}
"#,
        json_resp = json_resp,
        port = port
    );

    let server_thread = thread::spawn(move || {
        run_dapcode(&server_code).expect("Server DapCode harus selesai dengan sukses")
    });

    // Give server thread a moment to bind
    thread::sleep(Duration::from_millis(150));

    // Request 1: GET /api/status
    let mut client1 = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi ke port server harus berhasil");
    client1
        .write_all(b"GET /api/status HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut resp1 = String::new();
    client1.read_to_string(&mut resp1).unwrap();
    assert!(resp1.starts_with("HTTP/1.1 200 OK"));
    assert!(resp1.contains("Content-Type: application/json"));
    assert!(resp1.contains("{\"status\":\"aktif\",\"versi\":\"1.0\"}"));

    // Request 2: GET /api/halo
    let mut client2 = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi ke port server harus berhasil");
    client2
        .write_all(b"GET /api/halo HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut resp2 = String::new();
    client2.read_to_string(&mut resp2).unwrap();
    assert!(resp2.starts_with("HTTP/1.1 200 OK"));
    assert!(resp2.contains("Content-Type: text/plain; charset=utf-8"));
    assert!(resp2.contains("Halo dari DapCode HTTP Server!"));

    let result = server_thread.join().unwrap();
    match result {
        Value::Int(count) => assert_eq!(count, 2, "Server harus melayani tepat 2 permintaan"),
        other => panic!("Diharapkan Value::Int(2), didapat {:?}", other),
    }
}

#[test]
fn test_http_server_multi_arg_string_handler() {
    let port = get_free_port();

    let server_code = format!(
        r#"
fungsi string_handler(metode: Teks, jalur: Teks, isi: Teks) -> Teks {{
    jika metode == "POST" && jalur == "/echo" {{
        kembali "echo: " + isi
    }}
    kembali "rute string biasa"
}}

fungsi utama() -> u64 {{
    hasil := buat_server_http({}, "string_handler", 1) ? tangkap err {{
        kembali 0
    }}
    kembali hasil
}}
"#,
        port
    );

    let server_thread = thread::spawn(move || {
        run_dapcode(&server_code).expect("Server DapCode harus selesai dengan sukses")
    });

    thread::sleep(Duration::from_millis(150));

    let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi ke port server harus berhasil");
    let payload = "{\"nama\":\"DapCode\"}";
    let req = format!(
        "POST /echo HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        payload.len(),
        payload
    );
    client.write_all(req.as_bytes()).unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).unwrap();
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("echo: {\"nama\":\"DapCode\"}"));

    let result = server_thread.join().unwrap();
    match result {
        Value::Int(count) => assert_eq!(count, 1),
        other => panic!("Diharapkan Value::Int(1), didapat {:?}", other),
    }
}

#[test]
fn test_native_c_build_with_http_primitives() {
    let out_dir = std::path::Path::new("target/uji_c_http");
    let _ = std::fs::create_dir_all(out_dir);
    let src_path = out_dir.join("main.dap");
    let bin_path = out_dir.join("app_http.exe");

    let src = r#"
fungsi utama() {
    r1 := respons_json("\{\"status\":\"ok\"\}")
    r2 := respons_teks("halo dari c11")
    r3 := kirim_respons_http(201, "application/xml", "<ok/>")
    cetak("R1_STATUS:{r1.status}")
    cetak("R1_TIPE:{r1.tipe_konten}")
    cetak("R2_STATUS:{r2.status}")
    cetak("R2_KONTEN:{r2.konten}")
    cetak("R3_STATUS:{r3.status}")
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
    let c_code = codegen.generate_c(&prog);

    let c_file = out_dir.join("main.c");
    std::fs::write(&c_file, &c_code).unwrap();

    let gcc_status = std::process::Command::new("gcc")
        .args(&[
            "-std=c11",
            c_file.to_str().unwrap(),
            "-o",
            bin_path.to_str().unwrap(),
            "-lm",
        ])
        .status();

    if let Ok(st) = gcc_status {
        if st.success() {
            let run_output = std::process::Command::new(&bin_path)
                .output()
                .expect("Failed to execute native C binary");
            let stdout = String::from_utf8_lossy(&run_output.stdout);
            assert!(stdout.contains("R1_STATUS:200"));
            assert!(stdout.contains("R1_TIPE:application/json"));
            assert!(stdout.contains("R2_STATUS:200"));
            assert!(stdout.contains("R2_KONTEN:halo dari c11"));
            assert!(stdout.contains("R3_STATUS:201"));
        }
    }
}

