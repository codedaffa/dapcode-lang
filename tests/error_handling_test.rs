use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;

#[test]
fn test_error_unions_and_catch() {
    let src = r#"
fungsi bagi(a: u64, b: u64) -> u64 ! Teks {
    jika b == 0 {
        gagal "Tidak boleh dibagi nol"
    }
    a / b
}

fungsi utama() -> u64 {
    // Normal case
    v1 := bagi(100, 2) ? tangkap err {
        kembali 0
    }

    // Error case caught and recovered
    v2 := bagi(50, 0) ? tangkap err {
        999
    }

    v1 + v2
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("errors.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&program).expect("Type checking failed");

    let mut eval = Evaluator::new();
    let result = eval.run_program(&program).expect("Execution failed");

    match result {
        Value::Int(n) => assert_eq!(n, 50 + 999),
        other => panic!("Expected Value::Int(1049), got {:?}", other),
    }
}
