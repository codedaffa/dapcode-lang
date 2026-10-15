use dapcodec::lexer::{Lexer, TokenKind};
use dapcodec::span::SourceMap;

#[test]
fn test_lex_demo() {
    let demo_src = include_str!("../examples/demo.dap");
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("examples/demo.dap", demo_src.to_string());
    let mut lexer = Lexer::new(demo_src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed!");
    assert!(!tokens.is_empty());
    
    // Check some known tokens
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Struktur));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Sifat));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Terapkan));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Walrus));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Question));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Tangkap));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::JalanParalel));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Tunggu));
}
