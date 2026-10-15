use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;

#[test]
fn test_ownership_and_perceus_drops() {
    let src = r#"
struktur Titik {
    x: f64
    y: f64
}

fungsi hitung_jarak() -> f64 {
    p1 := Titik(x: 10.0, y: 20.0)
    p2 := Titik(x: 30.0, y: 40.0)
    hasil := p1.x + p2.x
    kembali hasil
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let analyzer = OwnershipAnalyzer::new();
    let report = analyzer.analyze_program(&program);

    let fn_analysis = report.functions.get("hitung_jarak").expect("Function analysis missing");
    
    // Check that p1 and p2 were analyzed
    assert!(fn_analysis.var_usages.contains_key("p1"));
    assert!(fn_analysis.var_usages.contains_key("p2"));

    // Check that drop points are injected
    assert!(!fn_analysis.drop_points.is_empty() || !fn_analysis.exit_drops.is_empty());
}
