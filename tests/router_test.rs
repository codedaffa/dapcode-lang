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
fn test_router_standalone_primitives() {
    let code = r#"
fungsi utama() -> Teks {
    // 1. Pencocokan Rute Pola
    c1 := cocok_rute("/api/produk/:id", "/api/produk/100")
    c2 := cocok_rute("/api/users/:uid/posts/:pid", "/api/users/andi/posts/77")
    c3 := cocok_rute("/api/produk/:id", "/api/kategori/100")
    pastikan(c1)
    pastikan(c2)
    pastikan(!c3)

    // 2. Ekstraksi Parameter Rute
    p1 := ambil_param_rute("/api/produk/:id", "/api/produk/999", "id") ? tangkap err {
        kembali "gagal_param_1"
    }
    pastikan_sama(p1, "999")

    p2 := ambil_param_rute("/api/users/:uid/orders/:oid", "/api/users/budi/orders/ord-555", ":oid") ? tangkap err {
        kembali "gagal_param_2"
    }
    pastikan_sama(p2, "ord-555")

    // 3. Query String Parsing
    kueri := "kategori=elektronik&halaman=5&urut=termurah"
    k1 := ambil_kueri_param(kueri, "kategori") ? tangkap err {
        kembali "gagal_kueri_1"
    }
    k2 := ambil_kueri_param(kueri, "halaman") ? tangkap err {
        kembali "gagal_kueri_2"
    }
    pastikan_sama(k1, "elektronik")
    pastikan_sama(k2, "5")
    pastikan(ada_kueri_param(kueri, "urut"))
    pastikan(!ada_kueri_param(kueri, "stok"))

    // 4. Header & CORS
    resp := respons_teks("Halo")
    resp_cors := respons_cors(resp)
    pastikan_sama(resp_cors.status, 200)

    kembali "sukses_primitif"
}
"#;

    let res = run_dapcode(code).expect("Evaluasi primitif router harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses_primitif"),
        other => panic!("Diharapkan string sukses_primitif, didapat {:?}", other),
    }
}

#[test]
fn test_router_live_http_server_route_params_and_cors() {
    let port = get_free_port();

    let template = r#"
fungsi api_handler(req: PermintaanHttp) -> ResponsHttp {
    // Rute Dinamis dengan Param: /api/produk/:id
    jika req.cocok("/api/produk/:id") {
        id := req.param("/api/produk/:id", "id") ? tangkap err {
            kembali respons_teks(err)
        }
        resp := respons_json("\{\"id\":\"{id}\"\}")
        kembali resp.cors()
    }

    // Rute dengan Query: /api/cari
    jika req.jalur == "/api/cari" {
        kata := req.kueri_param("kata") ? tangkap err { kembali respons_teks("kata tidak ada") }
        hal := req.kueri_param("hal") ? tangkap err { kembali respons_teks("hal tidak ada") }
        kembali respons_json("\{\"kata\":\"{kata}\",\"hal\":\"{hal}\"\}")
    }

    // Rute OPTIONS Preflight CORS
    jika req.metode == "OPTIONS" {
        kembali respons_teks("OK").cors()
    }

    kembali kirim_respons_http(404, "text/plain", "Rute Tidak Ditemukan")
}

fungsi utama() -> u64 {
    hasil := buat_server_http(PORT, "api_handler", 3) ? tangkap err {
        cetak("Error server: PORT")
        kembali 0
    }
    kembali hasil
}
"#;
    let server_code = template.replace("PORT", &port.to_string());

    let server_thread = thread::spawn(move || {
        run_dapcode(&server_code).expect("Server DapCode harus berjalan dengan sukses")
    });

    thread::sleep(Duration::from_millis(150));

    // Request 1: GET /api/produk/42 (Route Parameter)
    let mut client1 = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi 1 ke server harus berhasil");
    client1
        .write_all(b"GET /api/produk/42 HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut buf1 = Vec::new();
    client1.read_to_end(&mut buf1).unwrap();
    let resp1 = String::from_utf8_lossy(&buf1);
    assert!(resp1.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(resp1.contains("Access-Control-Allow-Origin: *"));
    assert!(resp1.contains("{\"id\":\"42\"}"));

    // Request 2: GET /api/cari?kata=buku&hal=3 (Query String Parameters)
    let mut client2 = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi 2 ke server harus berhasil");
    client2
        .write_all(b"GET /api/cari?kata=buku&hal=3 HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut buf2 = Vec::new();
    client2.read_to_end(&mut buf2).unwrap();
    let resp2 = String::from_utf8_lossy(&buf2);
    assert!(resp2.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(resp2.contains("{\"kata\":\"buku\",\"hal\":\"3\"}"));

    // Request 3: OPTIONS /api/apapun (Preflight CORS)
    let mut client3 = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi 3 ke server harus berhasil");
    client3
        .write_all(b"OPTIONS /api/apapun HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut buf3 = Vec::new();
    client3.read_to_end(&mut buf3).unwrap();
    let resp3 = String::from_utf8_lossy(&buf3);
    assert!(resp3.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(resp3.contains("Access-Control-Allow-Origin: *"));
    assert!(resp3.contains("Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS"));

    let served = server_thread.join().expect("Server thread join harus berhasil");
    match served {
        Value::Int(count) => assert_eq!(count, 3),
        other => panic!("Diharapkan jumlah permintaan 3, didapat {:?}", other),
    }
}

#[test]
fn test_native_c_build_with_router_primitives() {
    let out_dir = std::env::temp_dir().join("dapcode_router_test");
    let _ = std::fs::create_dir_all(&out_dir);
    let src_path = out_dir.join("main.dap");
    let bin_path = if cfg!(windows) {
        out_dir.join("main.exe")
    } else {
        out_dir.join("main")
    };

    let src = r#"
fungsi utama() {
    cocok := cocok_rute("/api/barang/:id", "/api/barang/789")
    jika cocok {
        cetak("COCOK:BENAR")
    }

    id := ambil_param_rute("/api/barang/:id", "/api/barang/789", "id") ? tangkap err {
        cetak("ERROR_PARAM")
        kembali
    }
    cetak("ID:{id}")

    kueri := "tema=gelap&bahasa=id"
    tema := ambil_kueri_param(kueri, "tema") ? tangkap err {
        cetak("ERROR_KUERI")
        kembali
    }
    cetak("TEMA:{tema}")

    resp := respons_json("\{\"status\":\"ok\"\}")
    resp_cors := respons_cors(resp)
    cetak("STATUS:{resp_cors.status}")
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
        .output()
        .expect("Failed to run gcc");

    if !gcc_status.status.success() {
        panic!("GCC error: {}", String::from_utf8_lossy(&gcc_status.stderr));
    }

    let run_output = std::process::Command::new(&bin_path)
        .output()
        .expect("Failed to execute native C binary");
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("COCOK:BENAR"));
    assert!(stdout.contains("ID:789"));
    assert!(stdout.contains("TEMA:gelap"));
    assert!(stdout.contains("STATUS:200"));
}
