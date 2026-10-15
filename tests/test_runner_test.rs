use dapcodec::cli::cmd_test;
use dapcodec::eval::Evaluator;
use dapcodec::fmt::Formatter;
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use std::fs;

#[test]
fn test_parser_and_format_uji_blocks() {
    let src = r#"
uji "uji penjumlahan" {
    a := 10 + 20
    pastikan_sama(a, 30)
}
"#;
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    assert_eq!(program.decls.len(), 1);
    match &program.decls[0] {
        dapcodec::ast::Decl::Test(t) => {
            assert_eq!(t.name, "uji penjumlahan");
            assert_eq!(t.body.stmts.len(), 1);
            assert!(t.body.expr.is_some());
        }
        other => panic!("Expected Decl::Test, got {:?}", other),
    }

    let mut fmt = Formatter::new();
    let formatted = fmt.format_program(&program);
    assert!(formatted.contains("uji \"uji penjumlahan\" {"));
    assert!(formatted.contains("pastikan_sama(a, 30)"));
}

#[test]
fn test_evaluator_run_tests_success() {
    let src = r#"
fungsi kali(x: bulat, y: bulat) -> bulat {
    kembali x * y
}

uji "perkalian sukses" {
    hasil := kali(5, 6)
    pastikan(hasil == 30)
    pastikan_sama(hasil, 30)
    pastikan_tidak_sama(hasil, 100)
}

uji "string dan boolean" {
    pastikan(benar)
    pastikan_sama("halo", "halo")
    pastikan_tidak_sama("a", "b")
}
"#;
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test_eval.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    let mut tc = TypeChecker::new(file_id);
    tc.check_program(&program).expect("Typecheck failed");

    let mut evaluator = Evaluator::new();
    let results = evaluator.run_tests(&program);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "perkalian sukses");
    assert!(results[0].passed);
    assert!(results[0].error.is_none());

    assert_eq!(results[1].name, "string dan boolean");
    assert!(results[1].passed);
    assert!(results[1].error.is_none());
}

#[test]
fn test_evaluator_run_tests_failures() {
    let src = r#"
uji "asersi_false" {
    pastikan(salah)
}

uji "asersi_tidak_sama" {
    pastikan_sama(10, 20)
}

uji "asersi_gagal_eksplisit" {
    gagal "Kondisi tidak terpenuhi"
}
"#;
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test_fail.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    let mut tc = TypeChecker::new(file_id);
    tc.check_program(&program).expect("Typecheck failed");

    let mut evaluator = Evaluator::new();
    let results = evaluator.run_tests(&program);

    assert_eq!(results.len(), 3);

    // Test 1: pastikan(salah)
    assert_eq!(results[0].name, "asersi_false");
    assert!(!results[0].passed);
    assert!(results[0].error.as_ref().unwrap().contains("Asersi gagal"));

    // Test 2: pastikan_sama(10, 20)
    assert_eq!(results[1].name, "asersi_tidak_sama");
    assert!(!results[1].passed);
    assert!(results[1].error.as_ref().unwrap().contains("tidak sama dengan"));

    // Test 3: gagal "Kondisi tidak terpenuhi"
    assert_eq!(results[2].name, "asersi_gagal_eksplisit");
    assert!(!results[2].passed);
    assert!(results[2].error.as_ref().unwrap().contains("Kondisi tidak terpenuhi"));
}

#[test]
fn test_cli_cmd_test_execution() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_test_runner_cli_{}", std::process::id()));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let test_file = temp_dir.join("uji_matematika.dap");
    let test_code = r#"
fungsi tambah(a: bulat, b: bulat) -> bulat {
    kembali a + b
}

uji "penjumlahan 10 + 20" {
    pastikan_sama(tambah(10, 20), 30)
}

uji "penjumlahan 0 + 5" {
    pastikan_sama(tambah(0, 5), 5)
}
"#;
    fs::write(&test_file, test_code).expect("Failed to write test file");

    // Run tests on file
    let res = cmd_test(Some(&test_file), None);
    assert!(res.is_ok(), "cmd_test should succeed on passing test file");

    // Run tests with filter
    let res_filter = cmd_test(Some(&test_file), Some("10 + 20"));
    assert!(res_filter.is_ok(), "cmd_test should succeed with filter");

    let _ = fs::remove_dir_all(&temp_dir);
}
