use dapcodec::cli::calculate_delimiter_delta;
use dapcodec::eval::{Environment, Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::{Parser, Precedence};
use dapcodec::span::SourceMap;
use dapcodec::types::{Type, TypeChecker};

#[test]
fn test_repl_delimiter_delta_calculation() {
    // Normal balanced line
    assert_eq!(calculate_delimiter_delta("a := 10 + 20"), 0);

    // Opening block
    assert_eq!(calculate_delimiter_delta("fungsi hitung(x: u64) -> u64 {"), 1);

    // Closing block
    assert_eq!(calculate_delimiter_delta("}"), -1);

    // Multiple nested opening delimiters
    assert_eq!(calculate_delimiter_delta("if (daftar[0] > { 5 }) {"), 1);

    // Delimiters inside string literals should NOT change delta
    assert_eq!(calculate_delimiter_delta("teks := \"{ ini kurung kurawal } (dan kurung)\""), 0);

    // Delimiters after line comment should NOT change delta
    assert_eq!(calculate_delimiter_delta("x := 10 // ini komentar { { { "), 0);
}

#[test]
fn test_repl_top_level_declaration_and_invocation() {
    let mut eval = Evaluator::new();
    let mut env = Environment::new();

    // 1. Register function declaration in REPL
    let fn_src = r#"
fungsi pangkat_tiga(n: u64) -> u64 {
    n * n * n
}
"#;
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("<repl>", fn_src.to_string());
    let mut lexer = Lexer::new(fn_src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let prog = parser.parse_program().expect("Parse program failed");

    for decl in &prog.decls {
        eval.register_decl(decl);
    }

    assert!(eval.get_defined_functions().contains(&"pangkat_tiga".to_string()));

    // 2. Evaluate expression calling the newly registered function
    let call_src = "pangkat_tiga(4)";
    let (call_id, _) = sm.add_file("<repl_call>", call_src.to_string());
    let mut lexer2 = Lexer::new(call_src, call_id);
    let tokens2 = lexer2.tokenize().expect("Tokenize call failed");
    let mut parser2 = Parser::new(tokens2, call_id);
    let expr = parser2.parse_expr(Precedence::Lowest).expect("Parse expr failed");

    let res = eval.eval_expr(&expr, &mut env).expect("Eval expr failed");
    match res {
        Value::Int(n) => assert_eq!(n, 64),
        other => panic!("Expected Value::Int(64), got {:?}", other),
    }
}

#[test]
fn test_repl_meta_type_query() {
    let mut sm = SourceMap::new();
    let expr_src = "Daftar(10, 20, 30)";
    let (file_id, _) = sm.add_file("<expr>", expr_src.to_string());
    let mut lexer = Lexer::new(expr_src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let expr = parser.parse_expr(Precedence::Lowest).expect("Parse expr failed");

    let mut checker = TypeChecker::new(file_id);
    let ty = checker.check_expr(&expr).expect("Type check failed");
    let resolved = checker.resolve_type(&ty);

    match resolved {
        Type::List(inner) => {
            assert_eq!(*inner, Type::U64);
        }
        other => panic!("Expected Type::List(U64), got {:?}", other),
    }
}
