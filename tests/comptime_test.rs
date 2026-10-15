use dapcodec::ast::*;
use dapcodec::comptime::ComptimeEvaluator;
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;

#[test]
fn test_comptime_block_folding() {
    let src = r#"
fungsi uji_comptime() -> u64 {
    x := comptime {
        a := 10
        b := 20
        a * b + 5
    }
    x
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("comptime.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let mut program = parser.parse_program().expect("Parsing failed");

    let mut comptime = ComptimeEvaluator::new();
    comptime.fold_program(&mut program).expect("Comptime folding failed");

    // Inspect the folded AST
    if let Decl::Fn(f) = &program.decls[0] {
        if let Stmt::VarDecl { init, .. } = &f.body.stmts[0] {
            // It should be folded directly into a Literal Int(205)
            match init {
                Expr::Lit(Literal::Int(205), _) => {}
                other => panic!("Expected folded Literal::Int(205), got {:?}", other),
            }
        } else {
            panic!("Expected VarDecl statement");
        }
    }
}
