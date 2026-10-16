use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::codegen::CodeGenerator;
use std::process::Command;

#[test]
fn test_basic_closure_invocation_and_capture() {
    let code = r#"
        fungsi utama() -> u64 {
            kali_dua := |x: u64| x * 2
            hasil1 := kali_dua(7)
            pastikan_sama(hasil1, 14)

            tambah := |a: u64, b: u64| a + b
            hasil2 := tambah(20, 22)
            pastikan_sama(hasil2, 42)

            faktor := 10
            skala := |n: u64| n * faktor
            hasil3 := skala(5)
            pastikan_sama(hasil3, 50)

            kembali 1
        }
    "#;

    let mut lexer = Lexer::new(code, 0);
    let tokens = lexer.tokenize().expect("Lexer failed");
    let mut parser = Parser::new(tokens, 0);
    let program = parser.parse_program().expect("Parser failed");

    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("TypeChecker failed");

    let mut eval = Evaluator::new();
    let res = eval.run_program(&program).expect("Evaluator failed");
    match res {
        Value::Int(n) => assert_eq!(n, 1),
        other => panic!("Expected Int(1), got {:?}", other),
    }
}

#[test]
fn test_daftar_petakan_saring_lipat_setiap_interpreter() {
    let code = r#"
        fungsi utama() -> u64 {
            angka := Daftar()
            angka.tambah(1)
            angka.tambah(2)
            angka.tambah(3)
            angka.tambah(4)
            angka.tambah(5)

            // 1. petakan (map)
            kuadrat := angka.petakan(|x| x * x)
            pastikan_sama(kuadrat.panjang(), 5)
            k0 := kuadrat.ambil(0) ? tangkap err { 0 }
            k4 := kuadrat.ambil(4) ? tangkap err { 0 }
            pastikan_sama(k0, 1)
            pastikan_sama(k4, 25)

            // 2. saring (filter)
            genap := angka.saring(|x| x % 2 == 0)
            pastikan_sama(genap.panjang(), 2)
            g0 := genap.ambil(0) ? tangkap err { 0 }
            g1 := genap.ambil(1) ? tangkap err { 0 }
            pastikan_sama(g0, 2)
            pastikan_sama(g1, 4)

            // 3. lipat (fold / reduce)
            total := angka.lipat(0, |acc, x| acc + x)
            pastikan_sama(total, 15)

            // 4. chaining (saring + petakan)
            hasil := angka.saring(|x| x > 2).petakan(|x| x * 10)
            pastikan_sama(hasil.panjang(), 3)
            h0 := hasil.ambil(0) ? tangkap err { 0 }
            h1 := hasil.ambil(1) ? tangkap err { 0 }
            h2 := hasil.ambil(2) ? tangkap err { 0 }
            pastikan_sama(h0, 30)
            pastikan_sama(h1, 40)
            pastikan_sama(h2, 50)

            // 5. setiap (for-each callback)
            angka.setiap(|x| cetak("Elemen: {x}"))

            kembali 1
        }
    "#;

    let mut lexer = Lexer::new(code, 0);
    let tokens = lexer.tokenize().expect("Lexer failed");
    let mut parser = Parser::new(tokens, 0);
    let program = parser.parse_program().expect("Parser failed");

    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("TypeChecker failed");

    let mut eval = Evaluator::new();
    let res = eval.run_program(&program).expect("Evaluator failed");
    match res {
        Value::Int(n) => assert_eq!(n, 1),
        other => panic!("Expected Int(1), got {:?}", other),
    }

    let output = eval.get_output();
    assert!(output.iter().any(|line| line.contains("Elemen: 1")));
    assert!(output.iter().any(|line| line.contains("Elemen: 5")));
}

#[test]
fn test_daftar_petakan_saring_lipat_native_c_codegen() {
    let code = r#"
        fungsi utama() -> u64 {
            angka := Daftar()
            angka.tambah(1)
            angka.tambah(2)
            angka.tambah(3)
            angka.tambah(4)
            angka.tambah(5)

            kuadrat := angka.petakan(|x| x * x)
            genap := angka.saring(|x| x % 2 == 0)
            total := angka.lipat(0, |acc, x| acc + x)

            cetak("Panjang kuadrat: {kuadrat.panjang()}")
            cetak("Panjang genap: {genap.panjang()}")
            cetak("Total lipat: {total}")

            kembali 0
        }
    "#;

    let mut lexer = Lexer::new(code, 0);
    let tokens = lexer.tokenize().expect("Lexer failed");
    let mut parser = Parser::new(tokens, 0);
    let program = parser.parse_program().expect("Parser failed");

    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("TypeChecker failed");

    let ownership = OwnershipAnalyzer::new();
    let report = ownership.analyze_program(&program);

    let mut codegen = CodeGenerator::new(&report);
    let c_code = codegen.generate_c(&program);

    let scratch_dir = std::env::temp_dir().join("dapcode_closure_test");
    let _ = std::fs::create_dir_all(&scratch_dir);
    let c_file = scratch_dir.join("test_closure.c");
    let exe_file = if cfg!(windows) {
        scratch_dir.join("test_closure.exe")
    } else {
        scratch_dir.join("test_closure")
    };

    std::fs::write(&c_file, c_code).expect("Failed to write C file");

    let compile_status = Command::new("gcc")
        .arg(&c_file)
        .arg("-o")
        .arg(&exe_file)
        .status()
        .expect("Failed to execute GCC");

    assert!(compile_status.success(), "GCC compilation failed for closure program");

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute compiled binary");

    assert!(run_output.status.success(), "Binary execution failed");

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("Panjang kuadrat: 5"), "Output should contain 'Panjang kuadrat: 5'");
    assert!(stdout.contains("Panjang genap: 2"), "Output should contain 'Panjang genap: 2'");
    assert!(stdout.contains("Total lipat: 15"), "Output should contain 'Total lipat: 15'");
}
