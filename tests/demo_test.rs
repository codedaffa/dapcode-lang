use dapcodec::comptime::ComptimeEvaluator;
use dapcodec::eval::Evaluator;
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;

#[test]
fn test_demo_dap_full_pipeline() {
    let demo_src = include_str!("../examples/demo.dap");
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("examples/demo.dap", demo_src.to_string());

    // 1. Lexer
    let mut lexer = Lexer::new(demo_src, file_id);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(diag) => panic!("Lexing failed:\n{}", diag.render(&sm)),
    };
    assert!(!tokens.is_empty());

    // 2. Parser
    let mut parser = Parser::new(tokens, file_id);
    let mut program = match parser.parse_program() {
        Ok(p) => p,
        Err(diag) => panic!("Parsing failed:\n{}", diag.render(&sm)),
    };
    assert_eq!(program.decls.len(), 5);

    // 3. Comptime Pass
    let mut comptime = ComptimeEvaluator::new();
    comptime.fold_program(&mut program).expect("Comptime folding failed");

    // 4. Type Checker
    let mut checker = TypeChecker::new(file_id);
    match checker.check_program(&program) {
        Ok(_) => {}
        Err(diag) => panic!("Type checking failed:\n{}", diag.render(&sm)),
    }

    // 5. Automatic Ownership & Perceus Analysis
    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let utama_analysis = ownership.functions.get("utama").expect("utama analysis missing");
    assert!(utama_analysis.var_usages.contains_key("item"));
    assert!(utama_analysis.var_usages.contains_key("tugas"));

    // 6. Direct Execution
    let mut evaluator = Evaluator::new();
    evaluator.run_program(&program).expect("Evaluation failed");

    // Verify printed output
    let out = evaluator.get_output();
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0],
        "Produk: Laptop Pro | Harga Diskon: Rp13500000.0"
    );
}
