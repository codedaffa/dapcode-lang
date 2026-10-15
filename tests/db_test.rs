use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
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
fn test_basis_data_crud_and_methods() {
    let db_path = "target/uji_basis_data_method.db";
    let _ = fs::remove_file(db_path);

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    db := buka_basis_data("{db_path}") ? tangkap err {{
        kembali "Gagal buka db: " + err
    }}

    // 1. Simpan
    db.simpan("kunci_1", "\{{\"nama\":\"Laptop Gaming\"\}}") ? tangkap err {{
        kembali "Gagal simpan 1"
    }}
    db.simpan("kunci_2", "nilai_dua") ? tangkap err {{
        kembali "Gagal simpan 2"
    }}

    // 2. Ada & Ambil
    ada1 := db.ada("kunci_1")
    pastikan(ada1)

    val1 := db.ambil("kunci_1") ? tangkap err {{
        kembali "Gagal ambil 1"
    }}
    pastikan_sama(val1, "\{{\"nama\":\"Laptop Gaming\"\}}")

    val2 := db.ambil("kunci_2") ? tangkap err {{
        kembali "Gagal ambil 2"
    }}
    pastikan_sama(val2, "nilai_dua")

    // 3. Hapus
    db.hapus("kunci_2") ? tangkap err {{
        kembali "Gagal hapus"
    }}
    pastikan_tidak_sama(db.ada("kunci_2"), benar)

    // 4. Padatkan
    db.padatkan() ? tangkap err {{
        kembali "Gagal padatkan"
    }}

    kembali "sukses"
}}
"#,
        db_path = db_path
    );

    let res = run_dapcode(&code).expect("Evaluasi BasisData method harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses"),
        other => panic!("Diharapkan string sukses, didapat {:?}", other),
    }

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_standalone_db_functions() {
    let db_path = "target/uji_db_fungsi.db";
    let _ = fs::remove_file(db_path);

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    // 1. Simpan
    db_simpan("{db_path}", "pengguna:101", "Budi") ? tangkap err {{
        kembali "Gagal simpan"
    }}

    // 2. Ada
    pastikan(db_ada("{db_path}", "pengguna:101"))

    // 3. Ambil
    u := db_ambil("{db_path}", "pengguna:101") ? tangkap err {{
        kembali "Gagal ambil"
    }}
    pastikan_sama(u, "Budi")

    // 4. Hapus
    db_hapus("{db_path}", "pengguna:101") ? tangkap err {{
        kembali "Gagal hapus"
    }}
    pastikan_tidak_sama(db_ada("{db_path}", "pengguna:101"), benar)

    kembali "sukses"
}}
"#,
        db_path = db_path
    );

    let res = run_dapcode(&code).expect("Evaluasi fungsi basis data harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses"),
        other => panic!("Diharapkan string sukses, didapat {:?}", other),
    }

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_db_and_http_server_rest_api_integration() {
    let port = get_free_port();
    let db_path = "target/uji_server_db.db";
    let _ = fs::remove_file(db_path);

    // Initial seed
    let _ = dapcodec::db::db_simpan(db_path, "101", "{\"id\":101,\"nama\":\"Kopi Robusta\"}");

    let server_code = format!(
        r#"
fungsi tangani_rest(req: PermintaanHttp) -> ResponsHttp {{
    db := buka_basis_data("{db_path}") ? tangkap err {{
        kembali kirim_respons_http(500, "text/plain", "Database error")
    }}

    jika req.jalur == "/api/item/101" {{
        data := db.ambil("101") ? tangkap err {{
            kembali kirim_respons_http(404, "text/plain", "Item tidak ditemukan")
        }}
        kembali respons_json(data)
    }}

    kembali kirim_respons_http(404, "text/plain", "Not Found")
}}

fungsi utama() -> u64 {{
    hasil := buat_server_http({port}, "tangani_rest", 1) ? tangkap err {{
        kembali 0
    }}
    kembali hasil
}}
"#,
        db_path = db_path,
        port = port
    );

    let server_thread = thread::spawn(move || {
        run_dapcode(&server_code).expect("Server DapCode harus selesai dengan sukses")
    });

    thread::sleep(Duration::from_millis(150));

    let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Koneksi ke port server harus berhasil");
    client
        .write_all(b"GET /api/item/101 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();

    let mut resp = String::new();
    client.read_to_string(&mut resp).unwrap();
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("Content-Type: application/json"));
    assert!(resp.contains("Kopi Robusta"));

    let result = server_thread.join().unwrap();
    match result {
        Value::Int(count) => assert_eq!(count, 1),
        other => panic!("Diharapkan Value::Int(1), didapat {:?}", other),
    }

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_native_c_build_with_db() {
    let out_dir = Path::new("target/uji_c_db");
    let _ = fs::create_dir_all(out_dir);
    let src_path = out_dir.join("main.dap");
    let bin_path = out_dir.join("app_db.exe");
    let db_path = "target/uji_c_db/native.db";
    let _ = fs::remove_file(db_path);

    let src = format!(
        r#"
fungsi utama() {{
    db_simpan("{db_path}", "kunci_c", "halo dari c11 db")
    ada := db_ada("{db_path}", "kunci_c")
    val := db_ambil("{db_path}", "kunci_c") ? tangkap err {{
        "error"
    }}
    cetak("ADA:{{ada}}")
    cetak("VAL:{{val}}")
}}
"#,
        db_path = db_path
    );
    fs::write(&src_path, &src).unwrap();

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
    fs::write(&c_file, &c_code).unwrap();

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
            assert!(stdout.contains("ADA:benar"));
            assert!(stdout.contains("VAL:halo dari c11 db"));
        }
    }

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_db_transactions_commit_and_rollback() {
    let db_path = "target/uji_db_transaksi.db";
    let _ = fs::remove_file(db_path);

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    db := buka_basis_data("{db_path}") ? tangkap err {{
        kembali "Gagal buka: " + err
    }}

    db.simpan("tetap", "nilai_tetap") ? tangkap err {{
        kembali "Gagal simpan awal"
    }}

    // 1. Uji Rollback / Batal
    db.mulai_transaksi() ? tangkap err {{
        kembali "Gagal mulai tx"
    }}
    pastikan_sama(db.sedang_transaksi(), benar)

    db.simpan("sementara", "akan_batal") ? tangkap err {{
        kembali "Gagal simpan tx"
    }}
    pastikan_sama(db.ada("sementara"), benar)

    db.batal() ? tangkap err {{
        kembali "Gagal batal"
    }}
    pastikan_sama(db.sedang_transaksi(), salah)
    pastikan_tidak_sama(db.ada("sementara"), benar)

    // 2. Uji Commit / Komit
    db.mulai_transaksi() ? tangkap err {{
        kembali "Gagal mulai tx 2"
    }}
    db.simpan("item:1", "Satu") ? tangkap err {{ kembali "err" }}
    db.simpan("item:2", "Dua") ? tangkap err {{ kembali "err" }}
    db.hapus("tetap") ? tangkap err {{ kembali "err" }}

    db.komit() ? tangkap err {{
        kembali "Gagal komit"
    }}

    pastikan_sama(db.sedang_transaksi(), salah)
    pastikan_tidak_sama(db.ada("tetap"), benar)
    pastikan_sama(db.ada("item:1"), benar)
    pastikan_sama(db.ada("item:2"), benar)

    kembali "sukses_transaksi"
}}
"#,
        db_path = db_path
    );

    let res = run_dapcode(&code).expect("Evaluasi transaksi harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses_transaksi"),
        other => panic!("Diharapkan 'sukses_transaksi', didapat {:?}", other),
    }

    // Pastikan persistensi komit setelah membuka ulang berkas
    let check_code = format!(
        r#"
fungsi utama() -> Teks {{
    db := buka_basis_data("{db_path}") ? tangkap err {{
        kembali "Gagal buka ulang"
    }}
    pastikan_sama(db.ada("item:1"), benar)
    pastikan_sama(db.ada("item:2"), benar)
    pastikan_tidak_sama(db.ada("tetap"), benar)
    kembali "sukses_durabilitas"
}}
"#,
        db_path = db_path
    );
    let check_res = run_dapcode(&check_code).expect("Evaluasi durabilitas harus berhasil");
    match check_res {
        Value::String(s) => assert_eq!(s, "sukses_durabilitas"),
        other => panic!("Diharapkan 'sukses_durabilitas', didapat {:?}", other),
    }

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_db_prefix_queries_and_json_methods() {
    let db_path = "target/uji_db_prefix_json.db";
    let _ = fs::remove_file(db_path);

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    db := buka_basis_data("{db_path}") ? tangkap err {{
        kembali "Gagal buka: " + err
    }}

    // 1. Simpan beberapa data dengan awalan
    db.simpan("produk:kopi", "\{{\"nama\":\"Kopi Robusta\"\}}") ? tangkap err {{ kembali "err" }}
    db.simpan("produk:teh", "\{{\"nama\":\"Teh Melati\"\}}") ? tangkap err {{ kembali "err" }}
    db.simpan("toko:alamat", "Jakarta") ? tangkap err {{ kembali "err" }}

    // 2. Prefix Query
    kunci_produk := db.ambil_awalan("produk:") ? tangkap err {{
        kembali "Gagal ambil awalan"
    }}
    pastikan_sama(kunci_produk.panjang(), 2)

    // 3. Ekspor JSON Awalan
    ekspor := db.ekspor_json_awalan("produk:") ? tangkap err {{
        kembali "Gagal ekspor json"
    }}
    pastikan(panjang_teks(ekspor) > 10)

    // 4. Impor JSON
    json_baru := "\{{\"impor:1\":\"Satu\",\"impor:2\":\"Dua\"\}}"
    jml := db.impor_json(json_baru) ? tangkap err {{
        kembali "Gagal impor json"
    }}
    pastikan_sama(jml, 2)
    pastikan_sama(db.ada("impor:1"), benar)
    pastikan_sama(db.ada("impor:2"), benar)

    val_imp1 := db.ambil("impor:1") ? tangkap err {{ kembali "err" }}
    pastikan_sama(val_imp1, "Satu")

    kembali "sukses_prefix_json"
}}
"#,
        db_path = db_path
    );

    let res = run_dapcode(&code).expect("Evaluasi prefix dan JSON harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "sukses_prefix_json"),
        other => panic!("Diharapkan 'sukses_prefix_json', didapat {:?}", other),
    }

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_native_c_build_with_db_advanced() {
    let out_dir = Path::new("target/uji_c_db_adv");
    let _ = fs::create_dir_all(out_dir);
    let src_path = out_dir.join("main.dap");
    let bin_path = out_dir.join("app_db_adv.exe");
    let db_path = "target/uji_c_db_adv/native_adv.db";
    let _ = fs::remove_file(db_path);

    let src = format!(
        r#"
fungsi utama() {{
    db := buka_basis_data("{db_path}") ? tangkap err {{ kembali }}
    db.simpan("item:1", "Nilai 1") ? tangkap err {{ kembali }}
    db.simpan("item:2", "Nilai 2") ? tangkap err {{ kembali }}

    awalan := db.ambil_awalan("item:") ? tangkap err {{ Daftar() }}
    cetak("JUMLAH_AWALAN:{{awalan.panjang()}}")

    json_str := "\{{\"k1\":\"v1\",\"k2\":\"v2\"\}}"
    diimpor := db.impor_json(json_str) ? tangkap err {{ 0 }}
    cetak("DIIMPOR:{{diimpor}}")

    v1 := db.ambil("k1") ? tangkap err {{ "err" }}
    cetak("K1:{{v1}}")
}}
"#,
        db_path = db_path
    );
    fs::write(&src_path, &src).unwrap();

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
    fs::write(&c_file, &c_code).unwrap();

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
            assert!(stdout.contains("JUMLAH_AWALAN:2"));
            assert!(stdout.contains("DIIMPOR:2"));
            assert!(stdout.contains("K1:v1"));
        }
    }

    let _ = fs::remove_file(db_path);
}
