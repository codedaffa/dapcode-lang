use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;

#[test]
fn test_coroutine_concurrency() {
    let src = r#"
fungsi utama() -> u64 {
    t1 := jalan_paralel {
        x := 10 + 20
        x
    }
    t2 := jalan_paralel {
        y := 100 * 2
        y
    }
    hasil1 := tunggu(t1)
    hasil2 := tunggu(t2)
    hasil1 + hasil2
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("concurrency.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let mut eval = Evaluator::new();
    let result = eval.run_program(&program).expect("Execution failed");

    match result {
        Value::Int(n) => assert_eq!(n, 230),
        other => panic!("Expected Value::Int(230), got {:?}", other),
    }
}
