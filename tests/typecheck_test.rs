use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;

#[test]
fn test_typecheck_demo() {
    let demo_src = include_str!("../examples/demo.dap");
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("examples/demo.dap", demo_src.to_string());
    let mut lexer = Lexer::new(demo_src, file_id);
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
    match checker.check_program(&program) {
        Ok(_) => {}
        Err(diag) => panic!("Typechecking failed:\n{}", diag.render(&sm)),
    }
}
