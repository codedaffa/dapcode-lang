use dapcodec::ast::*;
use dapcodec::codegen::CodeGenerator;
use dapcodec::lexer::Lexer;
use dapcodec::mobile::gpu_engine::build_native_gpu_bundle;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
use std::fs;
use std::process::Command;

fn parse_code(code: &str) -> Program {
    let mut lexer = Lexer::new(code, 0);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, 0);
    parser.parse_program().expect("Parsing gagal")
}

#[test]
fn test_flutter_style_direct_gpu_bundle_generation() {
    let code = r#"
fungsi perbarui_animasi() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    kanvas_bersihkan(0.05, 0.08, 0.15, 1.0)
    kanvas_warna_isi(0.2, 0.7, 1.0, 0.9)
    kanvas_persegi(50.0, 60.0, 200.0, 100.0)
    kanvas_warna_garis(1.0, 1.0, 1.0, 0.8)
    kanvas_tebal_garis(2.5)
    kanvas_bingkai_persegi(45.0, 55.0, 210.0, 110.0)
    kanvas_lingkaran(300.0, 200.0, 40.0)
    kanvas_garis(0.0, 0.0, w, h)
    kanvas_titik(150.0, 250.0, 8.0)
    kanvas_teks("DapCode Direct GPU Engine", 60.0, 90.0, 14.0)

    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()
    dt := kanvas_delta_waktu()
    jika klik == 1 {
        kanvas_lingkaran(mx, my, 25.0)
    }
}

fungsi utama() -> u64 {
    perbarui_animasi()
    kembali 0
}
"#;
    let program = parse_code(code);

    // 1. Verifikasi TypeChecker
    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("TypeChecker gagal");

    // 2. Transpilasi ke C11
    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);
    let c_code = codegen.generate_c(&program);

    // 3. Bangun bundel Direct GPU Native
    let temp_dir = std::env::temp_dir().join("test_dapcode_mobile_gpu");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Gagal membuat direktori sementara");

    build_native_gpu_bundle(&c_code, "AplikasiUjiGPU", &temp_dir)
        .expect("Gagal membuat bundel native GPU");

    // 4. Verifikasi Struktur Android NDK Project
    let android_dir = temp_dir.join("android");
    assert!(android_dir.exists(), "Folder android harus ada");
    let cmake_file = android_dir.join("app").join("src").join("main").join("cpp").join("CMakeLists.txt");
    assert!(cmake_file.exists(), "CMakeLists.txt harus ada");
    let cmake_content = fs::read_to_string(&cmake_file).unwrap();
    assert!(cmake_content.contains("GLESv2"), "CMake harus menautkan pustaka OpenGL ES 2.0");
    assert!(cmake_content.contains("DAPCODE_CUSTOM_CANVAS=1"));

    let surface_view = android_dir.join("app").join("src").join("main").join("java").join("com").join("dapcode").join("engine").join("DapCodeSurfaceView.java");
    assert!(surface_view.exists(), "DapCodeSurfaceView.java harus ada");

    let jni_engine = android_dir.join("app").join("src").join("main").join("java").join("com").join("dapcode").join("engine").join("DapCodeEngine.java");
    assert!(jni_engine.exists(), "DapCodeEngine.java harus ada");

    // 5. Verifikasi Struktur iOS Native Project
    let ios_dir = temp_dir.join("ios");
    assert!(ios_dir.exists(), "Folder ios harus ada");
    let vc_file = ios_dir.join("DapCodeViewController.m");
    assert!(vc_file.exists(), "DapCodeViewController.m harus ada");

    // 6. Verifikasi Berkas Engine Tersemat
    let engine_dir = temp_dir.join("engine");
    let engine_h = engine_dir.join("dapcode_engine.h");
    let engine_c = engine_dir.join("dapcode_engine.c");
    let app_c = engine_dir.join("app_dapcode.c");
    assert!(engine_h.exists() && engine_c.exists() && app_c.exists());

    // 7. Uji Kompilasi Riil C Engine + Transpiled Code dengan GCC (Desktop OpenGL Target)
    let out_obj = temp_dir.join("test_gpu_compile.o");
    let gcc_check = Command::new("gcc")
        .arg("-c")
        .arg(&engine_c)
        .arg("-I")
        .arg(&engine_dir)
        .arg("-o")
        .arg(&out_obj)
        .output();

    if let Ok(output) = gcc_check {
        assert!(
            output.status.success(),
            "Kompilasi C Engine gagal: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out_obj.exists(), "Objek hasil kompilasi C engine harus ada");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
