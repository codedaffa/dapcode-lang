use dapcodec::codegen::CodeGenerator;
use dapcodec::eval::Evaluator;
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use std::process::Command;

fn run_dapcode(src: &str) -> Vec<String> {
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test_regex.dap", src.to_string());

    let mut lexer = Lexer::new(src, file_id);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(diag) => panic!("Lexing failed:\n{}", diag.render(&sm)),
    };

    let mut parser = Parser::new(tokens, file_id);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(diag) => panic!("Parsing failed:\n{}", diag.render(&sm)),
    };

    let mut checker = TypeChecker::new(file_id);
    if let Err(diag) = checker.check_program(&program) {
        panic!("Type checking failed:\n{}", diag.render(&sm));
    }

    let _ownership = OwnershipAnalyzer::new().analyze_program(&program);

    let mut evaluator = Evaluator::new();
    if let Err(diag) = evaluator.run_program(&program) {
        panic!("Evaluation failed:\n{}", diag.render(&sm));
    }

    evaluator.get_output()
}

#[test]
fn test_evaluator_regex_operations() {
    let src = r#"
fungsi utama() {
    // 1. Uji cocok_regex
    teks_pesan := "Pesanan #12345 berhasil diproses"
    ada_angka := cocok_regex("\\d+", teks_pesan)
    cetak("Ada angka: {ada_angka}")

    cocok_awal := cocok_regex("^Pesanan", teks_pesan)
    cetak("Cocok awal: {cocok_awal}")

    cocok_salah := cocok_regex("^#12345", teks_pesan)
    cetak("Cocok salah: {cocok_salah}")

    // 2. Uji cari_regex
    daftar_harga := "Kopi: 15000, Roti: 8500, Susu: 12000"
    angka_list := cari_regex("\\d+", daftar_harga)
    cetak("Jumlah angka: {angka_list.panjang()}")
    cetak("Harga pertama: {angka_list.ambil(0)}")
    cetak("Harga kedua: {angka_list.ambil(1)}")

    // 3. Uji ganti_regex
    rahasia := "Kontak rahasia: 08123456789 dan 08987654321"
    disensor := ganti_regex("\\d+", rahasia, "[SENSOR]")
    cetak("Disensor: {disensor}")

    // 4. Uji ekstrak_regex
    email_text := "Hubungi tim di halo@dapcode.com sekarang!"
    email := ekstrak_regex("\\w+@\\w+\\.com", email_text) ? tangkap err {
        cetak("Error ekstrak: {err}")
        kembali
    }
    cetak("Email: {email}")

    // 5. Uji alternasi
    hewan := "Saya punya kucing"
    ada_peliharaan := cocok_regex("kucing|anjing", hewan)
    cetak("Peliharaan: {ada_peliharaan}")
}
"#;

    let output = run_dapcode(src);
    assert_eq!(output[0], "Ada angka: benar");
    assert_eq!(output[1], "Cocok awal: benar");
    assert_eq!(output[2], "Cocok salah: salah");
    assert_eq!(output[3], "Jumlah angka: 3");
    assert_eq!(output[4], "Harga pertama: 15000");
    assert_eq!(output[5], "Harga kedua: 8500");
    assert_eq!(output[6], "Disensor: Kontak rahasia: [SENSOR] dan [SENSOR]");
    assert_eq!(output[7], "Email: halo@dapcode.com");
    assert_eq!(output[8], "Peliharaan: benar");
}

#[test]
fn test_native_c_build_with_regex() {
    let src = r#"
fungsi utama() {
    teks := "ID Pengguna: 4589"
    jika cocok_regex("\\d+", teks) {
        cetak("KECACAHAN_OK")
    }

    ganti := ganti_regex("\\d+", teks, "9999")
    cetak("GANTI: {ganti}")

    ekstrak := ekstrak_regex("\\d+", teks) ? tangkap err {
        cetak("ERROR")
        kembali
    }
    cetak("EKSTRAK: {ekstrak}")
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test_native_regex.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().unwrap();

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&program).unwrap();

    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);

    let target_dir = std::env::temp_dir();
    let out_exe = target_dir.join("dap_test_native_regex.exe");

    codegen.compile_to_native(&program, &out_exe).unwrap();
    assert!(out_exe.exists(), "Target executable must exist");

    let run_res = Command::new(&out_exe).output().expect("Failed to run native regex test");
    let stdout = String::from_utf8_lossy(&run_res.stdout);

    assert!(stdout.contains("KECACAHAN_OK"));
    assert!(stdout.contains("GANTI: ID Pengguna: 9999"));
    assert!(stdout.contains("EKSTRAK: 4589"));

    let _ = std::fs::remove_file(&out_exe);
    let _ = std::fs::remove_file(out_exe.with_extension("c"));
}
