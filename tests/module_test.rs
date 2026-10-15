use dapcodec::eval::Evaluator;
use dapcodec::modules::{ModuleLoader, ProjectManifest};
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use std::fs;

#[test]
fn test_manifest_parsing() {
    let toml = r#"
[paket]
nama = "aplikasi_keren"
versi = "1.2.3"
penulis = ["Daffa", "Antigravity"]
berkas_utama = "sumber/utama.dap"

[dependensi]
"#;

    let manifest = ProjectManifest::from_str(toml)
        .expect("Manifest parsing failed");

    assert_eq!(manifest.nama, "aplikasi_keren");
    assert_eq!(manifest.versi, "1.2.3");
    assert_eq!(manifest.penulis, vec!["Daffa", "Antigravity"]);
    assert_eq!(manifest.berkas_utama, "sumber/utama.dap");
}

#[test]
fn test_project_init_scaffolding() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_test_init_{}", std::process::id()));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let manifest_path = ProjectManifest::init_project(&temp_dir, "proyek_uji")
        .expect("Project init failed");

    assert!(manifest_path.exists());
    let manifest_content = fs::read_to_string(&manifest_path).expect("Failed to read manifest");
    assert!(manifest_content.contains("nama = \"proyek_uji\""));
    assert!(manifest_content.contains("berkas_utama = \"src/utama.dap\""));

    let entry_file = temp_dir.join("src").join("utama.dap");
    assert!(entry_file.exists());
    let entry_content = fs::read_to_string(&entry_file).expect("Failed to read entry");
    assert!(entry_content.contains("fungsi utama()"));
    assert!(entry_content.contains("Halo dari proyek DapCode: proyek_uji!"));

    let gitignore = temp_dir.join(".gitignore");
    assert!(gitignore.exists());

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_multi_file_module_import_and_execution() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_test_mod_{}", std::process::id()));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let helper_file = temp_dir.join("matematika.dap");
    let helper_src = r#"
struktur Titik {
    x: desimal,
    y: desimal,
}

fungsi buat_titik(x: desimal, y: desimal) -> Titik {
    kembali Titik(x: x, y: y)
}

fungsi tambah(a: bulat, b: bulat) -> bulat {
    kembali a + b
}
"#;
    fs::write(&helper_file, helper_src).expect("Failed to write helper module");

    let main_file = temp_dir.join("utama.dap");
    let main_src = r#"
impor "matematika.dap"

fungsi utama() {
    pt := buat_titik(12.5, 34.5)
    cetak(pt.x)
    cetak(pt.y)
    total := tambah(100, 250)
    cetak(total)
}
"#;
    fs::write(&main_file, main_src).expect("Failed to write main module");

    let mut sm = SourceMap::new();
    let mut loader = ModuleLoader::new();
    let (entry_id, program) = match loader.load_program(&main_file, &mut sm) {
        Ok(res) => res,
        Err(diag) => panic!("Module loading failed:\n{}", diag.render(&sm)),
    };

    // Verify program has combined declarations
    assert!(program.decls.len() >= 4);

    // Type checking
    let mut checker = TypeChecker::new(entry_id);
    match checker.check_program(&program) {
        Ok(_) => {}
        Err(diag) => panic!("Type check failed:\n{}", diag.render(&sm)),
    }

    // Ownership analysis
    let _ownership = OwnershipAnalyzer::new().analyze_program(&program);

    // Execution
    let mut evaluator = Evaluator::new();
    evaluator.run_program(&program).expect("Evaluation failed");

    let out = evaluator.get_output();
    assert_eq!(out, vec!["12.5", "34.5", "350"]);

    // Test native C11/GCC compilation pipeline
    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = dapcodec::codegen::CodeGenerator::new(&ownership);
    let out_exe = temp_dir.join(if cfg!(windows) { "app.exe" } else { "app" });
    let build_res = codegen.compile_to_native(&program, &out_exe);
    if let Ok(()) = build_res {
        assert!(out_exe.exists());
        let run_cmd = std::process::Command::new(&out_exe).output().expect("Failed to execute native binary");
        assert!(run_cmd.status.success());
        let stdout_str = String::from_utf8_lossy(&run_cmd.stdout);
        assert!(stdout_str.contains("12.5"));
        assert!(stdout_str.contains("34.5"));
        assert!(stdout_str.contains("350"));
    }

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_project_dir_and_manifest_resolution() {
    let manifest_path = std::path::Path::new("examples/proyek_multiberkas/DapCode.toml");
    let dir_path = std::path::Path::new("examples/proyek_multiberkas");
    assert!(manifest_path.exists());
    assert!(dir_path.exists());

    let entry_from_dir = dapcodec::cli::resolve_target_file(Some(dir_path))
        .expect("Failed to resolve entry from dir");
    let s_dir = entry_from_dir.to_string_lossy();
    assert!(s_dir.ends_with("src\\utama.dap") || s_dir.ends_with("src/utama.dap"));

    let entry_from_manifest = dapcodec::cli::resolve_target_file(Some(manifest_path))
        .expect("Failed to resolve entry from manifest file");
    let s_manifest = entry_from_manifest.to_string_lossy();
    assert!(s_manifest.ends_with("src\\utama.dap") || s_manifest.ends_with("src/utama.dap"));
}

