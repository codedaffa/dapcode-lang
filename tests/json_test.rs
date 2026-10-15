use dapcodec::eval::Evaluator;
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;

fn run_dapcode(src: &str) -> Vec<String> {
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("test.dap", src.to_string());

    let mut lexer = Lexer::new(src, file_id);
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
    if let Err(diag) = checker.check_program(&program) {
        panic!("Type checking failed:\n{}", diag.render(&sm));
    }

    let _ownership = OwnershipAnalyzer::new().analyze_program(&program);

    let mut evaluator = Evaluator::new();
    if let Err(diag) = evaluator.run_program(&program) {
        panic!("Evaluation failed:\n{}", diag.render(&sm));
    }

    evaluator.get_output()
}

#[test]
fn test_json_serialization_and_struct() {
    let src = r#"
struktur Pengguna {
    id: u64
    nama: Teks
    skor: f64
    aktif: Bool
}

fungsi utama() {
    u := Pengguna(id: 101, nama: "Budi Santoso", skor: 98.5, aktif: benar)
    raw_json := ke_json(u)
    cetak(raw_json)

    list := Daftar(10, 20, 30)
    list_json := ke_json(list)
    cetak(list_json)
}
"#;

    let output = run_dapcode(src);
    assert_eq!(output.len(), 2);
    assert!(output[0].contains("\"id\":101"));
    assert!(output[0].contains("\"nama\":\"Budi Santoso\""));
    assert!(output[0].contains("\"skor\":98.5"));
    assert!(output[0].contains("\"aktif\":true"));
    assert_eq!(output[1], "[10,20,30]");
}

#[test]
fn test_json_parsing_and_key_extraction() {
    let src = r#"
fungsi utama() {
    json_data := "\{\"produk\": \"Laptop Gaming\", \"harga\": 18500000.0, \"stok\": 15, \"tersedia\": true\}"

    // 1. Ekstraksi string
    nama := ambil_kunci_json(json_data, "produk") ? tangkap err {
        cetak("Error produk: {err}")
        kembali
    }
    cetak("Produk: {nama}")

    // 2. Ekstraksi angka
    harga := ambil_angka_json(json_data, "harga") ? tangkap err {
        cetak("Error harga: {err}")
        kembali
    }
    cetak("Harga: {harga}")

    // 3. Ekstraksi boolean
    ada := ambil_bool_json(json_data, "tersedia") ? tangkap err {
        cetak("Error stok: {err}")
        kembali
    }
    cetak("Tersedia: {ada}")

    // 4. Uji keberadaan kunci
    punya_stok := ada_kunci_json(json_data, "stok")
    cetak("Ada stok: {punya_stok}")

    punya_diskon := ada_kunci_json(json_data, "diskon")
    cetak("Ada diskon: {punya_diskon}")

    // 5. Validasi JSON
    apakah_valid := valid_json(json_data)
    cetak("Valid JSON: {apakah_valid}")

    rusak := "[bukan json"
    apakah_rusak := valid_json(rusak)
    cetak("Rusak valid: {apakah_rusak}")
}
"#;

    let output = run_dapcode(src);
    assert_eq!(
        output,
        vec![
            "Produk: Laptop Gaming",
            "Harga: 18500000.0",
            "Tersedia: benar",
            "Ada stok: benar",
            "Ada diskon: salah",
            "Valid JSON: benar",
            "Rusak valid: salah",
        ]
    );
}

#[test]
fn test_json_error_handling_on_missing_key() {
    let src = r#"
fungsi utama() {
    json_str := "\{\"nama\": \"Kopi Susu\"\}"

    // Mencari kunci yang tidak ada, error harus tertangkap
    hasil := ambil_kunci_json(json_str, "harga") ? tangkap err {
        cetak("Sukses menangkap error: {err}")
        kembali
    }
    cetak("Tidak boleh sampai sini: {hasil}")
}
"#;

    let output = run_dapcode(src);
    assert_eq!(output.len(), 1);
    assert!(output[0].contains("Sukses menangkap error: Kunci 'harga' tidak ditemukan"));
}
