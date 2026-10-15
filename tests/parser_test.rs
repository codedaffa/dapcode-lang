use dapcodec::ast::*;
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;

#[test]
fn test_parse_demo() {
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

    assert_eq!(program.decls.len(), 5);

    // 1. Struct Produk
    match &program.decls[0] {
        Decl::Struct(s) => {
            assert_eq!(s.name, "Produk");
            assert_eq!(s.fields.len(), 3);
            assert_eq!(s.fields[0].name, "id");
            assert_eq!(s.fields[1].name, "nama");
            assert_eq!(s.fields[2].name, "harga");
        }
        other => panic!("Expected struct Produk, got {:?}", other),
    }

    // 2. Trait BisaDidiskon
    match &program.decls[1] {
        Decl::Trait(t) => {
            assert_eq!(t.name, "BisaDidiskon");
            assert_eq!(t.methods.len(), 1);
            assert_eq!(t.methods[0].name, "hitung_diskon");
            assert_eq!(t.methods[0].params.len(), 2);
            assert!(t.methods[0].params[0].is_self);
        }
        other => panic!("Expected trait BisaDidiskon, got {:?}", other),
    }

    // 3. Impl BisaDidiskon for Produk
    match &program.decls[2] {
        Decl::Impl(i) => {
            assert_eq!(i.trait_name.as_deref(), Some("BisaDidiskon"));
            assert_eq!(i.target_struct, "Produk");
            assert_eq!(i.methods.len(), 1);
            assert_eq!(i.methods[0].name, "hitung_diskon");
        }
        other => panic!("Expected impl BisaDidiskon for Produk, got {:?}", other),
    }

    // 4. Function cari_produk
    match &program.decls[3] {
        Decl::Fn(f) => {
            assert_eq!(f.name, "cari_produk");
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].name, "id");
            assert!(matches!(f.ret_ty, Some(TypeExpr::ErrorUnion(_, _, _))));
        }
        other => panic!("Expected fn cari_produk, got {:?}", other),
    }

    // 5. Function utama
    match &program.decls[4] {
        Decl::Fn(f) => {
            assert_eq!(f.name, "utama");
            assert_eq!(f.params.len(), 0);
            assert!(!f.body.stmts.is_empty());
        }
        other => panic!("Expected fn utama, got {:?}", other),
    }
}
