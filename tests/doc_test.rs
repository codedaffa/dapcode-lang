use dapcodec::cli::cmd_doc;
use dapcodec::doc::{DocExtractor, DocGenerator, DocItemKind, ProjectDoc};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use std::fs;
use std::path::Path;

#[test]
fn test_doc_extractor_comments_and_signatures() {
    let src = r#"
//! Modul Uji Matematika
//! Modul ini menyediakan fungsi kalkulasi dasar.

/// Menghitung faktorial dari bilangan bulat positif.
///
/// Contoh:
///   f := faktorial(5) // 120
fungsi faktorial(n: u64) -> u64 {
    jika n <= 1 {
        kembali 1
    }
    n * faktorial(n - 1)
}

/// Struktur data untuk merepresentasikan titik dalam bidang 2D.
struktur Titik2D {
    x: f64,
    y: f64
}

/// Sifat kontrak untuk objek yang dapat digambar.
sifat BisaGambar {
    fungsi gambar(ini) -> Teks
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("matematika.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    let module_doc = DocExtractor::extract_module(src, Path::new("matematika.dap"), &program, &sm);

    // Verify module-level doc
    assert!(module_doc.doc.is_some());
    assert!(module_doc.doc.unwrap().contains("Modul Uji Matematika"));

    // Verify functions
    let fn_item = module_doc.items.iter().find(|i| i.name == "faktorial").expect("faktorial function not found");
    assert_eq!(fn_item.kind, DocItemKind::Function);
    assert_eq!(fn_item.ret_type.as_deref(), Some("u64"));
    assert_eq!(fn_item.params.len(), 1);
    assert_eq!(fn_item.params[0].name, "n");
    assert_eq!(fn_item.params[0].ty.as_deref(), Some("u64"));
    assert!(fn_item.doc.as_ref().unwrap().contains("Menghitung faktorial"));

    // Verify struct
    let struct_item = module_doc.items.iter().find(|i| i.name == "Titik2D").expect("Titik2D struct not found");
    assert_eq!(struct_item.kind, DocItemKind::Struct);
    assert_eq!(struct_item.fields.len(), 2);
    assert_eq!(struct_item.fields[0].name, "x");
    assert_eq!(struct_item.fields[0].ty, "f64");
    assert!(struct_item.doc.as_ref().unwrap().contains("bidang 2D"));

    // Verify trait
    let trait_item = module_doc.items.iter().find(|i| i.name == "BisaGambar").expect("BisaGambar trait not found");
    assert_eq!(trait_item.kind, DocItemKind::Trait);
    assert_eq!(trait_item.methods.len(), 1);
    assert_eq!(trait_item.methods[0].name, "gambar");
}

#[test]
fn test_doc_generator_markdown_and_html() {
    let src = r#"
/// Fungsi penjumlahan sederhana
fungsi tambah(a: u64, b: u64) -> u64 {
    a + b
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("calc.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    let module_doc = DocExtractor::extract_module(src, Path::new("calc.dap"), &program, &sm);

    let project_doc = ProjectDoc {
        name: "TestCalc".to_string(),
        version: "1.0.0".to_string(),
        description: "Kalkulator uji".to_string(),
        modules: vec![module_doc],
    };

    let generator = DocGenerator::new(&project_doc);

    // 1. Verify Markdown output
    let md = generator.generate_markdown();
    assert!(md.contains("# 📚 Dokumentasi Proyek: TestCalc"));
    assert!(md.contains("fungsi tambah"));
    assert!(md.contains("fungsi tambah(a: u64, b: u64) -> u64"));

    // 2. Verify HTML output
    let html = generator.generate_html();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("TestCalc - Dokumentasi Resmi DapCode"));
    assert!(html.contains("tambah"));
    assert!(html.contains("Fungsi penjumlahan sederhana"));
    assert!(html.contains("Salin"));
    assert!(html.contains("filterDocs()"));
}

#[test]
fn test_cli_cmd_doc_execution() {
    let out_dir = Path::new("target/uji_doc_cli");
    let _ = fs::remove_dir_all(out_dir);

    let res = cmd_doc(Some(Path::new("examples/proyek_multiberkas")), Some(out_dir), "semua", false);
    assert!(res.is_ok(), "cmd_doc execution failed: {:?}", res);

    assert!(out_dir.join("README.md").exists(), "README.md not created");
    assert!(out_dir.join("index.html").exists(), "index.html not created");

    let md_content = fs::read_to_string(out_dir.join("README.md")).unwrap();
    assert!(md_content.contains("kalkulator_geometri"));
    assert!(md_content.contains("Titik"));

    let html_content = fs::read_to_string(out_dir.join("index.html")).unwrap();
    assert!(html_content.contains("kalkulator_geometri"));

    // Cleanup
    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn test_doc_uji_basis_data_escaped_strings_and_interpolation() {
    let out_dir = Path::new("target/uji_doc_basis_data");
    let _ = fs::remove_dir_all(out_dir);

    let res = cmd_doc(Some(Path::new("examples/uji_basis_data.dap")), Some(out_dir), "semua", false);
    assert!(res.is_ok(), "cmd_doc on uji_basis_data.dap failed: {:?}", res);

    assert!(out_dir.join("README.md").exists(), "README.md not created");
    assert!(out_dir.join("index.html").exists(), "index.html not created");

    let md_content = fs::read_to_string(out_dir.join("README.md")).unwrap();
    assert!(md_content.contains("demo_database_dasar"));
    assert!(md_content.contains("demo_transaksi_acid"));

    // Cleanup
    let _ = fs::remove_dir_all(out_dir);
}
