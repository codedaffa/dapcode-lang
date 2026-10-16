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
    let (file_id, _) = sm.add_file("test_time.dap", src.to_string());

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
fn test_evaluator_time_operations() {
    let src = r#"
fungsi utama() {
    // 1. Waktu saat ini
    detik_kini := waktu_sekarang_detik()
    jika detik_kini > 1000000000 {
        cetak("DETIK_OK")
    }

    ms_kini := waktu_sekarang_ms()
    jika ms_kini > 1000000000000 {
        cetak("MS_OK")
    }

    // 2. Tahun kabisat
    jika waktu_adalah_kabisat(2024) && !waktu_adalah_kabisat(2025) && !waktu_adalah_kabisat(2026) {
        cetak("KABISAT_OK")
    }

    // 3. Waktu komponen UTC
    w := waktu_komponen(0)
    cetak("TGL: {w.tahun}-{w.bulan}-{w.hari}")
    cetak("JAM: {w.jam}:{w.menit}:{w.detik}")

    // 4. Metode pada Waktu
    ep := w.ke_epoch()
    cetak("EPOCH: {ep}")

    iso := w.ke_iso()
    cetak("ISO: {iso}")

    hari := w.hari_dalam_minggu()
    cetak("HARI: {hari}")

    kustom := w.format("%A, %d %B %Y")
    cetak("FORMAT: {kustom}")

    // 5. Waktu sekarang lokal
    sekarang := waktu_sekarang()
    jika sekarang.tahun >= 2026 {
        cetak("TAHUN_SEKARANG_OK")
    }
}
"#;

    let output = run_dapcode(src);
    assert!(output.contains(&"DETIK_OK".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"MS_OK".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"KABISAT_OK".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"TGL: 1970-1-1".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"JAM: 0:0:0".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"EPOCH: 0".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"ISO: 1970-01-01T00:00:00Z".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"HARI: Kamis".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"FORMAT: Kamis, 01 Januari 1970".to_string()), "Output: {:?}", output);
    assert!(output.contains(&"TAHUN_SEKARANG_OK".to_string()), "Output: {:?}", output);
}

#[test]
fn test_native_time_compilation() {
    let src = r#"
fungsi utama() {
    // Uji fungsi waktu & metode pada kompilasi C native
    jika waktu_adalah_kabisat(2024) && !waktu_adalah_kabisat(2023) {
        cetak("NATIVE_KABISAT_OK")
    }

    w := waktu_komponen(0)
    cetak("NATIVE_TGL: {w.tahun}-{w.bulan}-{w.hari}")

    ep := w.ke_epoch()
    cetak("NATIVE_EPOCH: {ep}")

    iso := w.ke_iso()
    cetak("NATIVE_ISO: {iso}")

    hari := w.hari_dalam_minggu()
    cetak("NATIVE_HARI: {hari}")

    fmt := w.format("%d/%m/%Y")
    cetak("NATIVE_FORMAT: {fmt}")

    kini := waktu_sekarang()
    jika kini.tahun >= 2026 {
        cetak("NATIVE_SEKARANG_OK")
    }
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test_native_time.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().unwrap();

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&program).unwrap();

    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);

    let target_dir = std::env::temp_dir();
    let out_exe = target_dir.join("dap_test_native_time.exe");

    codegen.compile_to_native(&program, &out_exe).unwrap();
    assert!(out_exe.exists(), "Target executable must exist");

    let run_res = Command::new(&out_exe).output().expect("Failed to run native time test");
    let stdout = String::from_utf8_lossy(&run_res.stdout);

    assert!(stdout.contains("NATIVE_KABISAT_OK"), "stdout: {}", stdout);
    assert!(stdout.contains("NATIVE_TGL: 1970-1-1"), "stdout: {}", stdout);
    assert!(stdout.contains("NATIVE_EPOCH: 0"), "stdout: {}", stdout);
    assert!(stdout.contains("NATIVE_ISO: 1970-01-01T00:00:00Z"), "stdout: {}", stdout);
    assert!(stdout.contains("NATIVE_HARI: Kamis"), "stdout: {}", stdout);
    assert!(stdout.contains("NATIVE_FORMAT: 01/01/1970"), "stdout: {}", stdout);
    assert!(stdout.contains("NATIVE_SEKARANG_OK"), "stdout: {}", stdout);

    let _ = std::fs::remove_file(&out_exe);
    let _ = std::fs::remove_file(out_exe.with_extension("c"));
}
