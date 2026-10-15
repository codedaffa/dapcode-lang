use dapcodec::cli::{cmd_add_in, cmd_install_in, cmd_remove_in};
use dapcodec::eval::Evaluator;
use dapcodec::modules::{DependencyLocation, ModuleLoader, ProjectManifest};
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_manifest_dependency_add_remove_save() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_dep_manifest_{}", std::process::id()));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let manifest_path = temp_dir.join(ProjectManifest::MANIFEST_FILENAME);
    let mut manifest = ProjectManifest::default();
    manifest.nama = "proyek_uji".to_string();

    // Add path dependency
    manifest.add_dependency(
        "kalkulator",
        DependencyLocation::Path(PathBuf::from("../kalkulator")),
    );
    // Add git dependency
    manifest.add_dependency(
        "jaringan",
        DependencyLocation::Git("https://github.com/dapcode/jaringan.git".to_string()),
    );
    // Add version dependency
    manifest.add_dependency(
        "basisdata",
        DependencyLocation::Version("2.1.0".to_string()),
    );

    // Save and read back
    manifest.save_to_file(&manifest_path).expect("Failed to save manifest");
    let loaded = ProjectManifest::from_file(&manifest_path).expect("Failed to load manifest");

    assert_eq!(loaded.nama, "proyek_uji");
    assert_eq!(loaded.dependensi.len(), 3);
    assert_eq!(
        loaded.dependensi.get("kalkulator"),
        Some(&DependencyLocation::Path(PathBuf::from("../kalkulator")))
    );
    assert_eq!(
        loaded.dependensi.get("jaringan"),
        Some(&DependencyLocation::Git(
            "https://github.com/dapcode/jaringan.git".to_string()
        ))
    );
    assert_eq!(
        loaded.dependensi.get("basisdata"),
        Some(&DependencyLocation::Version("2.1.0".to_string()))
    );

    // Remove dependency
    let mut loaded = loaded;
    let removed = loaded.remove_dependency("jaringan");
    assert!(removed);
    loaded.save_to_file(&manifest_path).expect("Failed to save manifest after remove");

    let reloaded = ProjectManifest::from_file(&manifest_path).expect("Failed to reload manifest");
    assert_eq!(reloaded.dependensi.len(), 2);
    assert!(!reloaded.dependensi.contains_key("jaringan"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_multi_package_dependency_resolution_and_eval() {
    let temp_root = std::env::temp_dir().join(format!("dapcode_dep_eval_{}", std::process::id()));
    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp root");

    // 1. Create dependency package: 'kalkulator'
    let pkg_dir = temp_root.join("kalkulator");
    fs::create_dir_all(pkg_dir.join("src")).expect("Failed to create pkg src");
    let pkg_manifest = r#"
[paket]
nama = "kalkulator"
versi = "0.5.0"
berkas_utama = "src/lib.dap"

[dependensi]
"#;
    fs::write(pkg_dir.join("DapCode.toml"), pkg_manifest).expect("Failed to write pkg manifest");

    let pkg_code = r#"
fungsi hitung_faktorial(n: bulat) -> bulat {
    jika n <= 1 {
        kembali 1
    }
    kembali n * hitung_faktorial(n - 1)
}

fungsi kali_dua(n: bulat) -> bulat {
    kembali n * 2
}
"#;
    fs::write(pkg_dir.join("src").join("lib.dap"), pkg_code).expect("Failed to write pkg code");

    // 2. Create main project: 'aplikasi'
    let app_dir = temp_root.join("aplikasi");
    fs::create_dir_all(app_dir.join("src")).expect("Failed to create app src");
    let app_manifest = r#"
[paket]
nama = "aplikasi"
versi = "0.1.0"
berkas_utama = "src/utama.dap"

[dependensi]
kalkulator = { jalur = "../kalkulator" }
"#;
    fs::write(app_dir.join("DapCode.toml"), app_manifest).expect("Failed to write app manifest");

    let app_code = r#"
impor "kalkulator"

fungsi utama() {
    f5 := hitung_faktorial(5)
    cetak(f5)
    k10 := kali_dua(f5)
    cetak(k10)
}
"#;
    let main_file = app_dir.join("src").join("utama.dap");
    fs::write(&main_file, app_code).expect("Failed to write app code");

    // 3. Load program using ModuleLoader
    let mut sm = SourceMap::new();
    let mut loader = ModuleLoader::new();
    let (entry_id, program) = match loader.load_program(&main_file, &mut sm) {
        Ok(res) => res,
        Err(diag) => panic!("Load program failed:\n{}", diag.render(&sm)),
    };

    // 4. Type check
    let mut tc = TypeChecker::new(entry_id);
    tc.check_program(&program).expect("Type checking multi-package program failed");

    // 5. Ownership analysis
    let _ownership = OwnershipAnalyzer::new().analyze_program(&program);

    // 6. Evaluator execution
    let mut eval = Evaluator::new();
    eval.run_program(&program).expect("Evaluation failed");
    let out = eval.get_output();
    assert_eq!(out, vec!["120", "240"]);

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_multi_package_subpath_import() {
    let temp_root = std::env::temp_dir().join(format!("dapcode_dep_sub_{}", std::process::id()));
    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp root");

    // Create dependency package: 'utilitas' with submodules
    let pkg_dir = temp_root.join("utilitas");
    fs::create_dir_all(pkg_dir.join("src")).expect("Failed to create pkg src");
    let pkg_manifest = r#"
[paket]
nama = "utilitas"
versi = "1.0.0"
berkas_utama = "src/lib.dap"

[dependensi]
"#;
    fs::write(pkg_dir.join("DapCode.toml"), pkg_manifest).expect("Failed to write pkg manifest");
    fs::write(
        pkg_dir.join("src").join("lib.dap"),
        "fungsi versi_util() -> teks { kembali \"1.0.0\" }",
    ).expect("Failed to write lib");

    let format_code = r#"
fungsi salam(nama: teks) -> teks {
    kembali "Halo, " + nama + "!"
}
"#;
    fs::write(pkg_dir.join("src").join("format.dap"), format_code).expect("Failed to write format submodule");

    // Main app importing utilitas/format
    let app_dir = temp_root.join("aplikasi_sub");
    fs::create_dir_all(app_dir.join("src")).expect("Failed to create app src");
    let app_manifest = r#"
[paket]
nama = "aplikasi_sub"
versi = "0.1.0"
berkas_utama = "src/utama.dap"

[dependensi]
utilitas = { jalur = "../utilitas" }
"#;
    fs::write(app_dir.join("DapCode.toml"), app_manifest).expect("Failed to write app manifest");

    let app_code = r#"
impor "utilitas/format"

fungsi utama() {
    msg := salam("DapCode")
    cetak(msg)
}
"#;
    let main_file = app_dir.join("src").join("utama.dap");
    fs::write(&main_file, app_code).expect("Failed to write app code");

    let mut sm = SourceMap::new();
    let mut loader = ModuleLoader::new();
    let (entry_id, program) = match loader.load_program(&main_file, &mut sm) {
        Ok(res) => res,
        Err(diag) => panic!("Load program failed:\n{}", diag.render(&sm)),
    };

    let mut tc = TypeChecker::new(entry_id);
    tc.check_program(&program).expect("Typecheck failed");

    let mut eval = Evaluator::new();
    eval.run_program(&program).expect("Eval failed");
    let out = eval.get_output();
    assert_eq!(out, vec!["Halo, DapCode!"]);

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_package_dependency_not_found_diagnostic() {
    let temp_root = std::env::temp_dir().join(format!("dapcode_dep_err_{}", std::process::id()));
    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp root");

    let app_manifest = r#"
[paket]
nama = "aplikasi_hilang"
versi = "0.1.0"
berkas_utama = "src/utama.dap"

[dependensi]
paket_gaib = { jalur = "../paket_yang_tidak_pernah_ada" }
"#;
    fs::write(temp_root.join("DapCode.toml"), app_manifest).expect("Failed to write manifest");

    let app_code = r#"
impor "paket_gaib"

fungsi utama() {
    cetak("tidak akan sampai sini")
}
"#;
    let main_file = temp_root.join("src").join("utama.dap");
    fs::create_dir_all(temp_root.join("src")).expect("Failed to create src");
    fs::write(&main_file, app_code).expect("Failed to write app code");

    let mut sm = SourceMap::new();
    let mut loader = ModuleLoader::new();
    let res = loader.load_program(&main_file, &mut sm);

    assert!(res.is_err());
    let err = res.unwrap_err();
    let msg = err.render(&sm);
    assert!(
        msg.contains("tidak ditemukan") || msg.contains("dapcodec install"),
        "Diagnostic should inform user about missing dependency package, got: {}",
        msg
    );

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_cli_dependency_commands() {
    let temp_dir = std::env::temp_dir().join(format!("dapcode_cli_dep_{}", std::process::id()));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    // Initialize project
    ProjectManifest::init_project(&temp_dir, "proyek_cli").expect("Project init failed");

    // 1. Add dependency via cmd_add_in
    cmd_add_in(
        &temp_dir,
        "kalkulator",
        Some(PathBuf::from("../kalkulator")),
        None,
        None,
    )
    .expect("cmd_add_in failed");

    cmd_add_in(
        &temp_dir,
        "jaringan",
        None,
        Some("https://github.com/dapcode/jaringan.git".to_string()),
        None,
    )
    .expect("cmd_add_in git failed");

    cmd_add_in(
        &temp_dir,
        "basisdata",
        None,
        None,
        Some("1.0.0".to_string()),
    )
    .expect("cmd_add_in version failed");

    // Verify manifest has all 3
    let manifest_path = temp_dir.join(ProjectManifest::MANIFEST_FILENAME);
    let m = ProjectManifest::from_file(&manifest_path).expect("Failed to read manifest");
    assert_eq!(m.dependensi.len(), 3);
    assert!(m.dependensi.contains_key("kalkulator"));
    assert!(m.dependensi.contains_key("jaringan"));
    assert!(m.dependensi.contains_key("basisdata"));

    // 2. Install dependencies via cmd_install_in
    cmd_install_in(&temp_dir).expect("cmd_install_in failed");

    // 3. Remove dependency via cmd_remove_in
    cmd_remove_in(&temp_dir, "jaringan").expect("cmd_remove_in failed");

    let m_after = ProjectManifest::from_file(&manifest_path).expect("Failed to read manifest");
    assert_eq!(m_after.dependensi.len(), 2);
    assert!(!m_after.dependensi.contains_key("jaringan"));
    assert!(m_after.dependensi.contains_key("kalkulator"));
    assert!(m_after.dependensi.contains_key("basisdata"));

    let _ = fs::remove_dir_all(&temp_dir);
}
