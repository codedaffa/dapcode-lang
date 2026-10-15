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
fn test_static_file_primitives() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_static_prim_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let html_file = temp_dir.join("index.html");
    std::fs::write(&html_file, "<h1>Halo DapCode</h1>").unwrap();

    let css_file = temp_dir.join("style.css");
    std::fs::write(&css_file, "body { margin: 0; }").unwrap();

    let dir_str = temp_dir.to_str().unwrap().replace("\\", "/");
    let html_str = html_file.to_str().unwrap().replace("\\", "/");

    let code = format!(r#"
fungsi utama() -> Teks {{
    // 1. respons_berkas langsung
    resp_html := respons_berkas("{html_str}") ? tangkap err {{
        kembali "gagal_baca_html: " + err
    }}
    pastikan_sama(resp_html.status, 200)
    pastikan_sama(resp_html.tipe_konten, "text/html; charset=utf-8")
    pastikan_sama(resp_html.konten, "<h1>Halo DapCode</h1>")

    // 2. respons_berkas untuk berkas yang tidak ada
    gagal_resp := respons_berkas("{dir_str}/hilang.txt") ? tangkap err {{
        kirim_respons_http(404, "text/plain", "berkas_hilang")
    }}
    pastikan_sama(gagal_resp.status, 404)

    // 3. sajikan_berkas_statis ke index.html saat root "/"
    resp_root := sajikan_berkas_statis("{dir_str}", "/") ? tangkap err {{
        kembali "gagal_root: " + err
    }}
    pastikan_sama(resp_root.status, 200)
    pastikan_sama(resp_root.tipe_konten, "text/html; charset=utf-8")
    pastikan_sama(resp_root.konten, "<h1>Halo DapCode</h1>")

    // 4. sajikan_berkas_statis untuk berkas css spesifik
    resp_css := sajikan_berkas_statis("{dir_str}", "/style.css") ? tangkap err {{
        kembali "gagal_css: " + err
    }}
    pastikan_sama(resp_css.status, 200)
    pastikan_sama(resp_css.tipe_konten, "text/css; charset=utf-8")
    pastikan(resp_css.konten != "")

    // 5. proteksi directory traversal
    resp_traversal := sajikan_berkas_statis("{dir_str}", "/../rahasia.txt") ? tangkap err {{
        kirim_respons_http(403, "text/plain", "akses_ilegal")
    }}
    pastikan_sama(resp_traversal.status, 403)
    pastikan_sama(resp_traversal.konten, "akses_ilegal")

    kembali "sukses_statis"
}}
"#);

    let res = run_dapcode(&code).expect("Evaluasi primitif statis harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses_statis"),
        other => panic!("Diharapkan string sukses_statis, didapat {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_live_http_server_static_file_serving() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_static_srv_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let html_file = temp_dir.join("index.html");
    std::fs::write(&html_file, "<!DOCTYPE html><html><body>DapCode App</body></html>").unwrap();

    let js_file = temp_dir.join("app.js");
    std::fs::write(&js_file, "console.log('DapCode running');").unwrap();

    // Mock binary file (e.g. 8-byte PNG header: 137, 80, 78, 71, 13, 10, 26, 10)
    let png_file = temp_dir.join("icon.png");
    std::fs::write(&png_file, &[137u8, 80, 78, 71, 13, 10, 26, 10, 0, 1, 2, 3]).unwrap();

    let dir_str = temp_dir.to_str().unwrap().replace("\\", "/");
    let port = get_free_port();
    let json_resp = r#"\{\"status\":\"online\"\}"#;

    let server_code = format!(r#"
fungsi tangani_web(req: PermintaanHttp) -> ResponsHttp {{
    // 1. Rute API dinamis
    jika req.jalur == "/api/status" {{
        kembali respons_json("{json_resp}")
    }}

    // 2. Sajikan berkas statis dari folder
    resp := req.sajikan_statis("{dir_str}") ? tangkap err {{
        kirim_respons_http(404, "text/plain", "Tidak Ditemukan")
    }}
    kembali resp
}}

fungsi utama() -> u64 {{
    hasil := buat_server_http({port}, "tangani_web", 4) ? tangkap err {{
        kembali 0
    }}
    kembali hasil
}}
"#, json_resp = json_resp, dir_str = dir_str, port = port);

    let handle = thread::spawn(move || {
        run_dapcode(&server_code)
    });

    thread::sleep(Duration::from_millis(300));

    // Request 1: Static root index.html
    let mut stream1 = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream1.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut buf1 = Vec::new();
    stream1.read_to_end(&mut buf1).unwrap();
    let resp1 = String::from_utf8_lossy(&buf1);
    assert!(resp1.contains("HTTP/1.1 200 OK"));
    assert!(resp1.contains("Content-Type: text/html; charset=utf-8"));
    assert!(resp1.contains("<!DOCTYPE html><html><body>DapCode App</body></html>"));

    // Request 2: Static JS asset
    let mut stream2 = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream2.write_all(b"GET /app.js HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut buf2 = Vec::new();
    stream2.read_to_end(&mut buf2).unwrap();
    let resp2 = String::from_utf8_lossy(&buf2);
    assert!(resp2.contains("HTTP/1.1 200 OK"));
    assert!(resp2.contains("Content-Type: application/javascript; charset=utf-8"));
    assert!(resp2.contains("console.log('DapCode running');"));

    // Request 3: Static binary PNG asset (verify raw binary integrity)
    let mut stream3 = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream3.write_all(b"GET /icon.png HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut buf3 = Vec::new();
    stream3.read_to_end(&mut buf3).unwrap();
    let header_end = buf3.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let headers_part = String::from_utf8_lossy(&buf3[..header_end]);
    let body_bytes = &buf3[header_end + 4..];
    assert!(headers_part.contains("HTTP/1.1 200 OK"));
    assert!(headers_part.contains("Content-Type: image/png"));
    assert_eq!(body_bytes, &[137u8, 80, 78, 71, 13, 10, 26, 10, 0, 1, 2, 3]);

    // Request 4: Dynamic API route
    let mut stream4 = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream4.write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut buf4 = Vec::new();
    stream4.read_to_end(&mut buf4).unwrap();
    let resp4 = String::from_utf8_lossy(&buf4);
    assert!(resp4.contains("HTTP/1.1 200 OK"));
    assert!(resp4.contains("Content-Type: application/json"));
    assert!(resp4.contains("{\"status\":\"online\"}"));

    let server_res = handle.join().unwrap().expect("Server harus berjalan lancar");
    match server_res {
        Value::Int(n) => assert_eq!(n, 4),
        other => panic!("Diharapkan integer 4, didapat {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_native_c_build_with_static_file_primitives() {
    let out_dir = std::env::temp_dir().join(format!("dapcode_c_static_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&out_dir).unwrap();

    let pub_dir = out_dir.join("public");
    std::fs::create_dir_all(&pub_dir).unwrap();
    std::fs::write(pub_dir.join("index.html"), "<h1>Native C DapCode Static</h1>").unwrap();
    std::fs::write(pub_dir.join("test.json"), "{\"pesan\":\"halo_c\"}").unwrap();

    let pub_str = pub_dir.to_str().unwrap().replace("\\", "/");

    let src_path = out_dir.join("main.dap");
    let bin_path = if cfg!(windows) {
        out_dir.join("main.exe")
    } else {
        out_dir.join("main")
    };

    let src = format!(r#"
fungsi utama() {{
    resp1 := sajikan_berkas_statis("{pub_str}", "/") ? tangkap err {{
        cetak("GAGAL_INDEX")
        kembali
    }}
    cetak("STATUS1:{{resp1.status}}")
    cetak("MIME1:{{resp1.tipe_konten}}")

    resp2 := sajikan_berkas_statis("{pub_str}", "/test.json") ? tangkap err {{
        cetak("GAGAL_JSON")
        kembali
    }}
    cetak("STATUS2:{{resp2.status}}")
    cetak("MIME2:{{resp2.tipe_konten}}")

    // Test directory traversal rejection
    resp3 := sajikan_berkas_statis("{pub_str}", "/../forbidden.txt") ? tangkap err {{
        cetak("TRAVERSAL_DITOLAK")
        kirim_respons_http(403, "text/plain", "ditolak")
    }}
    cetak("STATUS3:{{resp3.status}}")
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
        .output()
        .expect("Failed to run gcc");

    if !gcc_status.status.success() {
        panic!("GCC error: {}", String::from_utf8_lossy(&gcc_status.stderr));
    }

    let run_output = std::process::Command::new(&bin_path)
        .output()
        .expect("Failed to execute native C binary");
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("STATUS1:200"));
    assert!(stdout.contains("MIME1:text/html; charset=utf-8"));
    assert!(stdout.contains("STATUS2:200"));
    assert!(stdout.contains("MIME2:application/json"));
    assert!(stdout.contains("TRAVERSAL_DITOLAK"));
    assert!(stdout.contains("STATUS3:403"));

    let _ = std::fs::remove_dir_all(&out_dir);
}
