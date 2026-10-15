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
    let (fid, _) = sm.add_file("uji_kamus.dap", code.to_string());
    let mut lexer = Lexer::new(code, fid);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, fid);
    let program = parser.parse_program().expect("Parsing gagal");
    let mut tc = TypeChecker::new(fid);
    tc.check_program(&program).expect("Pemeriksaan tipe gagal");
    program
}

#[test]
fn test_kamus_methods_interpreter() {
    let code = r#"
fungsi utama() -> u64 {
    kamus := Kamus()

    // 1. Awalnya kosong
    pastikan(kamus.kosong())
    pastikan_sama(kamus.panjang(), 0)

    // 2. Pasang nilai
    kamus.pasang("bahasa", "DapCode")
    kamus.pasang("pembuat", "Dap")
    kamus.pasang("status", "rilis")

    pastikan(!kamus.kosong())
    pastikan_sama(kamus.panjang(), 3)

    // 3. Periksa keberadaan kunci (.ada)
    pastikan(kamus.ada("bahasa"))
    pastikan(kamus.ada("pembuat"))
    pastikan(kamus.ada("status"))
    pastikan(!kamus.ada("tahun"))

    // 4. Ambil nilai (.ambil & .ambil_atau)
    bhs := kamus.ambil("bahasa") ? tangkap err { "gagal" }
    pastikan_sama(bhs, "DapCode")

    tidak_ada := kamus.ambil("tahun") ? tangkap err { "tidak_ditemukan" }
    pastikan_sama(tidak_ada, "tidak_ditemukan")

    status := kamus.ambil_atau("status", "bawaan")
    pastikan_sama(status, "rilis")

    tahun_default := kamus.ambil_atau("tahun", "2026")
    pastikan_sama(tahun_default, "2026")

    // 5. Timpa nilai yang sudah ada
    kamus.pasang("status", "stabil")
    pastikan_sama(kamus.panjang(), 3)
    status_baru := kamus.ambil_atau("status", "")
    pastikan_sama(status_baru, "stabil")

    // 6. Daftar Kunci & Nilai
    kunci := kamus.daftar_kunci()
    pastikan_sama(kunci.panjang(), 3)
    pastikan_sama(kunci.ambil(0) ? tangkap err { "" }, "bahasa")
    pastikan_sama(kunci.ambil(1) ? tangkap err { "" }, "pembuat")
    pastikan_sama(kunci.ambil(2) ? tangkap err { "" }, "status")

    nilai := kamus.daftar_nilai()
    pastikan_sama(nilai.panjang(), 3)
    pastikan_sama(nilai.ambil(0) ? tangkap err { "" }, "DapCode")
    pastikan_sama(nilai.ambil(1) ? tangkap err { "" }, "Dap")
    pastikan_sama(nilai.ambil(2) ? tangkap err { "" }, "stabil")

    // 7. Serialisasi JSON (.ke_json)
    json_str := kamus.ke_json()
    pastikan(json_str.berisi("\"bahasa\":\"DapCode\""))
    pastikan(json_str.berisi("\"pembuat\":\"Dap\""))
    pastikan(json_str.berisi("\"status\":\"stabil\""))

    // Fungsi global ke_json(kamus) juga bekerja
    json_global := ke_json(kamus)
    pastikan_sama(json_str, json_global)

    // 8. Hapus nilai (.hapus)
    dihapus := kamus.hapus("pembuat") ? tangkap err { "gagal" }
    pastikan_sama(dihapus, "Dap")
    pastikan_sama(kamus.panjang(), 2)
    pastikan(!kamus.ada("pembuat"))

    // 9. Bersihkan seluruh entri (.bersihkan)
    kamus.bersihkan()
    pastikan_sama(kamus.panjang(), 0)
    pastikan(kamus.kosong())

    kembali 1
}
"#;

    let program = parse_and_check(code);
    let mut eval = Evaluator::new();
    let res = eval.run_program(&program).expect("Evaluasi gagal");
    assert!(matches!(res, Value::Int(1)));
}

#[test]
fn test_kamus_for_in_loop() {
    let code = r#"
fungsi utama() -> u64 {
    kamus := Kamus()
    kamus.pasang("a", "apel")
    kamus.pasang("b", "belimbing")
    kamus.pasang("c", "ceri")

    daftar_kunci_hasil := Daftar()
    untuk k di kamus {
        daftar_kunci_hasil.tambah(k)
    }

    pastikan_sama(daftar_kunci_hasil.panjang(), 3)
    pastikan_sama(daftar_kunci_hasil.ambil(0) ? tangkap err { "" }, "a")
    pastikan_sama(daftar_kunci_hasil.ambil(1) ? tangkap err { "" }, "b")
    pastikan_sama(daftar_kunci_hasil.ambil(2) ? tangkap err { "" }, "c")

    kembali 100
}
"#;

    let program = parse_and_check(code);
    let mut eval = Evaluator::new();
    let res = eval.run_program(&program).expect("Evaluasi gagal");
    assert!(matches!(res, Value::Int(100)));
}

#[test]
fn test_kamus_native_c_codegen() {
    let code = r#"
fungsi utama() -> Unit {
    kamus := Kamus()
    
    // Periksa status kosong
    jika kamus.kosong() {
        cetak("KAMUS_KOSONG:OK")
    }
    
    kamus.pasang("id", "user_101")
    kamus.pasang("nama", "Budi Santoso")
    kamus.pasang("role", "admin")
    
    cetak(kamus.panjang())
    
    // Uji ambil_atau
    cetak(kamus.ambil_atau("role", "tamu"))
    cetak(kamus.ambil_atau("kota", "Jakarta"))
    
    // Uji ada
    jika kamus.ada("nama") {
        cetak("ADA_NAMA:OK")
    }
    jika !kamus.ada("kota") {
        cetak("TIDAK_ADA_KOTA:OK")
    }
    
    // Uji daftar_kunci & daftar_nilai
    kunci := kamus.daftar_kunci()
    cetak(kunci.panjang())
    cetak(kunci.ambil(0) ? tangkap err { "" })
    cetak(kunci.ambil(1) ? tangkap err { "" })
    cetak(kunci.ambil(2) ? tangkap err { "" })
    
    // Uji ke_json
    json := kamus.ke_json()
    cetak(json)
    
    // Uji hapus
    hapus_res := kamus.hapus("role") ? tangkap err { "" }
    cetak(hapus_res)
    cetak(kamus.panjang())
    
    // Perulangan untuk di kamus
    untuk k di kamus {
        cetak(k)
    }
    
    // Uji bersihkan
    kamus.bersihkan()
    jika kamus.kosong() {
        cetak("BERSIH:OK")
    }
}
"#;

    let program = parse_and_check(code);

    let ownership = OwnershipAnalyzer::new();
    let report = ownership.analyze_program(&program);

    let mut codegen = CodeGenerator::new(&report);
    let c_code = codegen.generate_c(&program);

    let temp_dir = std::env::temp_dir();
    let src_file = temp_dir.join("dap_test_native_kamus.c");
    let bin_file = temp_dir.join("dap_test_native_kamus.exe");

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
            assert!(out_str.contains("KAMUS_KOSONG:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("3"), "stdout: {}", out_str);
            assert!(out_str.contains("admin"), "stdout: {}", out_str);
            assert!(out_str.contains("Jakarta"), "stdout: {}", out_str);
            assert!(out_str.contains("ADA_NAMA:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("TIDAK_ADA_KOTA:OK"), "stdout: {}", out_str);
            assert!(out_str.contains("id"), "stdout: {}", out_str);
            assert!(out_str.contains("nama"), "stdout: {}", out_str);
            assert!(out_str.contains("\"id\":\"user_101\""), "stdout: {}", out_str);
            assert!(out_str.contains("\"nama\":\"Budi Santoso\""), "stdout: {}", out_str);
            assert!(out_str.contains("2"), "stdout: {}", out_str);
            assert!(out_str.contains("BERSIH:OK"), "stdout: {}", out_str);
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
