use dapcodec::ast::*;
use dapcodec::codegen::CodeGenerator;
use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use std::fs;
use std::process::Command;

fn parse_and_check(code: &str) -> Program {
    let mut sm = SourceMap::new();
    let (fid, _) = sm.add_file("uji_metode.dap", code.to_string());
    let mut lexer = Lexer::new(code, fid);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, fid);
    let program = parser.parse_program().expect("Parsing gagal");
    let mut tc = TypeChecker::new(fid);
    tc.check_program(&program).expect("Pemeriksaan tipe gagal");
    program
}

#[test]
fn test_teks_methods_interpreter() {
    let code = r#"
fungsi utama() -> u64 {
    teks := "  Halo Dunia DapCode  "
    
    // 1. Pangkas & Huruf Besar/Kecil
    pangkas := teks.pangkas()
    besar := pangkas.huruf_besar()
    kecil := pangkas.huruf_kecil()
    
    pastikan_sama(pangkas, "Halo Dunia DapCode")
    pastikan_sama(besar, "HALO DUNIA DAPCODE")
    pastikan_sama(kecil, "halo dunia dapcode")
    pastikan_sama(pangkas.panjang(), 18)
    
    // 2. Berisi, Awalan, Akhiran
    pastikan(pangkas.berisi("Dunia"))
    pastikan(!pangkas.berisi("Rust"))
    pastikan(pangkas.awalan("Halo"))
    pastikan(!pangkas.awalan("Dunia"))
    pastikan(pangkas.akhiran("DapCode"))
    pastikan(!pangkas.akhiran("Halo"))
    
    // 3. Ganti & Potong
    ganti := pangkas.ganti("Dunia", "Nusantara")
    pastikan_sama(ganti, "Halo Nusantara DapCode")
    
    potong := pangkas.potong(5, 5) // ambil 5 karakter mulai offset 5: "Dunia"
    pastikan_sama(potong, "Dunia")
    
    // 4. Ambil karakter di indeks
    char0 := pangkas.ambil(0) ? tangkap err { "" }
    char1 := pangkas.ambil(1) ? tangkap err { "" }
    pastikan_sama(char0, "H")
    pastikan_sama(char1, "a")
    
    // 5. Pisah string ke Daftar teks
    csv := "apel,jeruk,mangga,pisang"
    buah := csv.pisah(",")
    pastikan_sama(buah.panjang(), 4)
    b0 := buah.ambil(0) ? tangkap err { "" }
    b1 := buah.ambil(1) ? tangkap err { "" }
    pastikan_sama(b0, "apel")
    pastikan_sama(b1, "jeruk")
    
    // 6. Method chaining
    gabung_chain := csv.pisah(",").balik().gabung(" - ")
    pastikan_sama(gabung_chain, "pisang - mangga - jeruk - apel")
    
    kembali 100
}
"#;
    let program = parse_and_check(code);
    let mut eval = Evaluator::new();
    let res = eval.run_program(&program).expect("Evaluasi gagal");
    assert!(matches!(res, Value::Int(100)));
}

#[test]
fn test_daftar_methods_interpreter() {
    let code = r#"
fungsi utama() -> u64 {
    daftar := Daftar(10, 20, 30)
    
    // 1. Tambah & Panjang
    daftar.tambah(40)
    pastikan_sama(daftar.panjang(), 4)
    
    // 2. Berisi & Kosong
    pastikan(daftar.berisi(20))
    pastikan(!daftar.berisi(99))
    pastikan(!daftar.kosong())
    
    kosong_list := Daftar()
    pastikan(kosong_list.kosong())
    
    // 3. Sisip
    daftar.sisip(1, 15) ? tangkap err { salah }
    pastikan_sama(daftar.panjang(), 5)
    // Sekarang: [10, 15, 20, 30, 40]
    i0 := daftar.ambil(0) ? tangkap err { 0 }
    i1 := daftar.ambil(1) ? tangkap err { 0 }
    i2 := daftar.ambil(2) ? tangkap err { 0 }
    pastikan_sama(i0, 10)
    pastikan_sama(i1, 15)
    pastikan_sama(i2, 20)
    
    // 4. Hapus
    dihapus := daftar.hapus(1) ? tangkap err { 0 }
    pastikan_sama(dihapus, 15)
    pastikan_sama(daftar.panjang(), 4)
    // Kembali ke: [10, 20, 30, 40]
    
    // 5. Potong
    sub := daftar.potong(1, 3) // indeks 1 sampai < 3 -> [20, 30]
    pastikan_sama(sub.panjang(), 2)
    s0 := sub.ambil(0) ? tangkap err { 0 }
    s1 := sub.ambil(1) ? tangkap err { 0 }
    pastikan_sama(s0, 20)
    pastikan_sama(s1, 30)
    
    // 6. Balik & Gabung
    kata := Daftar("kopi", "gula", "air")
    kebalik := kata.balik()
    k0 := kebalik.ambil(0) ? tangkap err { "" }
    pastikan_sama(k0, "air")
    
    gabungan := kata.gabung(" + ")
    pastikan_sama(gabungan, "kopi + gula + air")
    
    kembali 200
}
"#;
    let program = parse_and_check(code);
    let mut eval = Evaluator::new();
    let res = eval.run_program(&program).expect("Evaluasi gagal");
    assert!(matches!(res, Value::Int(200)));
}

#[test]
fn test_native_c_collection_and_string_methods() {
    let code = r#"
fungsi uji_c_native() -> u64 {
    // 1. Teks methods di C native
    s := "  DapCode Native C11  "
    p := s.pangkas()
    besar := p.huruf_besar()
    kecil := p.huruf_kecil()
    
    cetak("PANJANG:{p.panjang()}")
    cetak("BESAR:{besar}")
    cetak("KECIL:{kecil}")
    jika p.berisi("Native") { cetak("BERISI:OK") }
    jika p.awalan("DapCode") { cetak("AWALAN:OK") }
    jika p.akhiran("C11") { cetak("AKHIRAN:OK") }
    
    ganti := p.ganti("Native", "Super")
    cetak("GANTI:{ganti}")
    
    pot := p.potong(8, 6) // "Native"
    cetak("POTONG:{pot}")
    
    c0 := p.ambil(0) ? tangkap err { "" }
    cetak("CHAR0:{c0}")
    
    // 2. Daftar methods di C native
    d := Daftar("satu", "dua", "tiga")
    d.tambah("empat")
    cetak("D_PANJANG:{d.panjang()}")
    jika !d.kosong() { cetak("D_TIDAK_KOSONG:OK") }
    jika d.berisi("dua") { cetak("D_BERISI:OK") }
    
    d.sisip(1, "sisipan") ? tangkap err { salah }
    cetak("D_SISIP_LEN:{d.panjang()}")
    
    dihapus := d.hapus(1) ? tangkap err { "" }
    cetak("D_DIHAPUS:{dihapus}")
    cetak("D_AFTER_HAPUS_LEN:{d.panjang()}")
    
    gabung := d.balik().gabung(", ")
    cetak("D_GABUNG:{gabung}")
    
    bagian := d.potong(0, 2)
    cetak("D_BAGIAN_LEN:{bagian.panjang()}")
    
    kembali 42
}

fungsi utama() -> u64 {
    kembali uji_c_native()
}
"#;
    let program = parse_and_check(code);
    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);
    let c_code = codegen.generate_c(&program);

    let temp_dir = std::env::temp_dir();
    let src_file = temp_dir.join("dap_test_native_methods.c");
    let bin_file = temp_dir.join("dap_test_native_methods.exe");

    fs::write(&src_file, &c_code).expect("Gagal menulis kode C sementara");

    let gcc_status = Command::new("gcc")
        .args(&[
            "-std=c11",
            src_file.to_str().unwrap(),
            "-o",
            bin_file.to_str().unwrap(),
            "-lm",
            "-lws2_32",
        ])
        .status();

    match gcc_status {
        Ok(status) if status.success() => {
            let run_output = Command::new(&bin_file)
                .output()
                .expect("Gagal menjalankan biner native");
            assert!(
                run_output.status.success(),
                "Eksekusi C binary gagal dengan exit code: {:?}, stdout: {}, stderr: {}",
                run_output.status.code(),
                String::from_utf8_lossy(&run_output.stdout),
                String::from_utf8_lossy(&run_output.stderr)
            );
            let out_str = String::from_utf8_lossy(&run_output.stdout);
            assert!(out_str.contains("PANJANG:18"), "stdout: {}", out_str);
            assert!(out_str.contains("BESAR:DAPCODE NATIVE C11"), "stdout: {}", out_str);
            assert!(out_str.contains("KECIL:dapcode native c11"), "stdout: {}", out_str);
            assert!(out_str.contains("BERISI:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("AWALAN:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("AKHIRAN:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("GANTI:DapCode Super C11"), "stdout: {}", out_str);
            assert!(out_str.contains("POTONG:Native"), "stdout: {}", out_str);
            assert!(out_str.contains("CHAR0:D"), "stdout: {}", out_str);
            assert!(out_str.contains("D_PANJANG:4"), "stdout: {}", out_str);
            assert!(out_str.contains("D_TIDAK_KOSONG:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("D_BERISI:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("D_SISIP_LEN:5"), "stdout: {}", out_str);
            assert!(out_str.contains("D_DIHAPUS:sisipan"), "stdout: {}", out_str);
            assert!(out_str.contains("D_AFTER_HAPUS_LEN:4"), "stdout: {}", out_str);
            assert!(out_str.contains("D_GABUNG:empat, tiga, dua, satu"), "stdout: {}", out_str);
            assert!(out_str.contains("D_BAGIAN_LEN:2"), "stdout: {}", out_str);
        }
        Ok(status) => {
            panic!("Kompilasi GCC gagal dengan status {:?}", status);
        }
        Err(e) => {
            eprintln!("GCC tidak tersedia di lingkungan pengujian: {:?}", e);
        }
    }

    let _ = fs::remove_file(src_file);
    let _ = fs::remove_file(bin_file);
}
