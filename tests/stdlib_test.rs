use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;

fn run_dap_code(src: &str) -> (Value, Vec<String>) {
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("stdlib_test.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&program).expect("Type checking failed");

    let mut eval = Evaluator::new();
    let result = eval.run_program(&program).expect("Execution failed");
    let output = eval.get_output();
    (result, output)
}

fn run_dap_code_with_args(src: &str, args: Vec<String>) -> (Value, Vec<String>) {
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("stdlib_test.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&program).expect("Type checking failed");

    let mut eval = Evaluator::with_cli_args(args);
    let result = eval.run_program(&program).expect("Execution failed");
    let output = eval.get_output();
    (result, output)
}

#[test]
fn test_stdlib_daftar_collection() {
    let src = r#"
fungsi utama() -> u64 {
    koleksi := Daftar(10, 20, 30)
    koleksi.tambah(40)
    
    p := koleksi.panjang() // harus 4
    
    // Ambil elemen valid
    item0 := koleksi.ambil(0) ? tangkap err {
        kembali 0
    }
    
    // Ambil elemen di luar batas (harus ditangkap safely)
    item_out := koleksi.ambil(100) ? tangkap err {
        999
    }
    
    p + item0 + item_out // 4 + 10 + 999 = 1013
}
"#;

    let (res, _) = run_dap_code(src);
    match res {
        Value::Int(n) => assert_eq!(n, 1013),
        other => panic!("Expected Value::Int(1013), got {:?}", other),
    }
}

#[test]
fn test_stdlib_strings_and_math() {
    let src = r#"
fungsi utama() -> f64 {
    teks := "dapcode"
    len := panjang_teks(teks) // 7
    besar := ke_huruf_besar(teks) // DAPCODE
    kecil := ke_huruf_kecil(besar) // dapcode
    
    cetak(besar)
    cetak(kecil)
    
    akar := akar_kuadrat(16.0) // 4.0
    p := pangkat(2.0, 3.0)     // 8.0
    mutlak := nilai_mutlak(-10.5) // 10.5
    
    waktu := waktu_sekarang_ms()
    jika waktu > 0 {
        akar + p + mutlak // 4.0 + 8.0 + 10.5 = 22.5
    } selain_itu {
        0.0
    }
}
"#;

    let (res, output) = run_dap_code(src);
    assert_eq!(output, vec!["DAPCODE".to_string(), "dapcode".to_string()]);
    match res {
        Value::Float(f) => assert!((f - 22.5).abs() < 1e-6),
        other => panic!("Expected Value::Float(22.5), got {:?}", other),
    }
}

#[test]
fn test_stdlib_file_io() {
    let src = r#"
fungsi utama() -> Teks {
    jalur := "temp_dap_test.txt"
    
    // Tulis berkas
    sukses := tulis_berkas(jalur, "Halo DapCode!") ? tangkap err {
        kembali "Gagal menulis"
    }
    
    // Cek ada berkas
    ada := ada_berkas(jalur)
    jika !ada {
        kembali "Berkas tidak ditemukan"
    }
    
    // Baca berkas
    isi := baca_berkas(jalur) ? tangkap err {
        kembali "Gagal membaca"
    }
    
    // Coba baca berkas yang tidak ada (harus tertangkap)
    baca_berkas("berkas_tidak_ada_xyz.txt") ? tangkap err {
        cetak("Tertangkap error berkas")
    }
    
    isi
}
"#;

    let (res, output) = run_dap_code(src);
    assert_eq!(output, vec!["Tertangkap error berkas".to_string()]);
    match res {
        Value::String(s) => assert_eq!(s, "Halo DapCode!"),
        other => panic!("Expected Value::String('Halo DapCode!'), got {:?}", other),
    }

    // Cleanup
    let _ = std::fs::remove_file("temp_dap_test.txt");
}

#[test]
fn test_for_in_loop() {
    let src = r#"
fungsi utama() -> u64 {
    angka_list := Daftar(10, 20, 30, 40)
    total := 0
    
    untuk n di angka_list {
        total = total + n
    }
    
    total
}
"#;

    let (res, _) = run_dap_code(src);
    match res {
        Value::Int(n) => assert_eq!(n, 100),
        other => panic!("Expected Value::Int(100), got {:?}", other),
    }
}

#[test]
fn test_stdlib_json_operations() {
    let src = r#"
struktur Barang {
    kode: u64
    nama: Teks
}

fungsi utama() -> Teks {
    b := Barang(kode: 99, nama: "Keyboard RGB")
    j := ke_json(b)
    nama_b := ambil_kunci_json(j, "nama") ? tangkap err {
        kembali "Error"
    }
    nama_b
}
"#;

    let (res, _) = run_dap_code(src);
    match res {
        Value::String(s) => assert_eq!(s, "Keyboard RGB"),
        other => panic!("Expected Value::String('Keyboard RGB'), got {:?}", other),
    }
}

#[test]
fn test_stdlib_filesystem_and_timer() {
    let src = r#"
fungsi utama() -> u64 {
    dir := "temp_test_dir_dap"
    buat_direktori(dir) ? tangkap err {
        kembali 0
    }

    file := gabung_jalur(dir, "catatan.txt")
    tulis_berkas(file, "tes isi") ? tangkap err {
        kembali 0
    }

    daftar := daftar_direktori(dir) ? tangkap err {
        kembali 0
    }

    jum_entri := daftar.panjang()

    // Test timer
    tidur(5)

    // Bersihkan
    hapus_berkas(file) ? tangkap err {
        kembali 0
    }

    hapus_direktori(dir) ? tangkap err {
        kembali 0
    }

    jum_entri
}
"#;

    let (res, _) = run_dap_code(src);
    match res {
        Value::Int(n) => assert_eq!(n, 1),
        other => panic!("Expected Value::Int(1), got {:?}", other),
    }

    // Double check cleanup
    let _ = std::fs::remove_dir_all("temp_test_dir_dap");
}

#[test]
fn test_stdlib_cli_args_and_env() {
    let src = r#"
fungsi utama() -> Teks {
    // 1. Uji Argumen Baris Perintah (CLI)
    args := ambil_argumen_cli()
    p := args.panjang()
    arg0 := args.ambil(0) ? tangkap err {
        "kosong"
    }

    // 2. Uji Variabel Lingkungan (Set & Get)
    atur_lingkungan("DAPCODE_TEST_KEY", "NilaiRahasia999")
    nilai_env := ambil_lingkungan("DAPCODE_TEST_KEY") ? tangkap err {
        kembali "Gagal baca env"
    }

    // 3. Uji Variabel Lingkungan yang tidak ada (Error Union catch)
    tidak_ada := ambil_lingkungan("KEY_YANG_PASTI_TIDAK_ADA_XYZ") ? tangkap err {
        "berhasil_tertangkap"
    }

    jika p > 0 && nilai_env == "NilaiRahasia999" && tidak_ada == "berhasil_tertangkap" {
        arg0
    } selain_itu {
        "gagal"
    }
}
"#;

    let (res, _) = run_dap_code_with_args(src, vec!["program.dap".to_string(), "--mode=cepat".to_string()]);
    match res {
        Value::String(s) => assert_eq!(s, "program.dap"),
        other => panic!("Expected Value::String('program.dap'), got {:?}", other),
    }
}

