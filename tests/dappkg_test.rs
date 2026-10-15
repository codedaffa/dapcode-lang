use dapcodec::cli::{
    cmd_add_in, cmd_init_in, cmd_install_in, cmd_pkg_check_in, cmd_remove_in, cmd_tree_in,
    cmd_update_in,
};
use dapcodec::modules::{Lockfile, ProjectManifest};
use std::fs;
use std::path::PathBuf;

#[test]
fn test_lockfile_generation_and_integrity() {
    let temp_root = std::env::temp_dir().join(format!("dapcode_lock_test_{}", std::process::id()));
    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp root");

    // 1. Buat paket dependensi lokal: 'matematika'
    let dep_dir = temp_root.join("matematika");
    fs::create_dir_all(dep_dir.join("src")).expect("Failed to create dep src");
    let dep_manifest = r#"
[paket]
nama = "matematika"
versi = "1.2.0"
berkas_utama = "src/lib.dap"
"#;
    fs::write(dep_dir.join("DapCode.toml"), dep_manifest).expect("Failed to write dep manifest");
    fs::write(
        dep_dir.join("src").join("lib.dap"),
        "fungsi tambah(a: bulat, b: bulat) -> bulat { kembali a + b }",
    )
    .expect("Failed to write dep lib");

    // 2. Buat proyek utama: 'proyek_app'
    let app_dir = temp_root.join("proyek_app");
    fs::create_dir_all(app_dir.join("src")).expect("Failed to create app src");
    cmd_init_in(&app_dir, "proyek_app").expect("Failed to init app");

    // 3. Tambahkan dependensi lokal menggunakan cmd_add_in
    cmd_add_in(
        &app_dir,
        "matematika",
        Some(PathBuf::from("../matematika")),
        None,
        None,
    )
    .expect("Failed to add dependency");

    // 4. Jalankan install -> harus membuat DapCode.lock dengan hash SHA-256
    cmd_install_in(&app_dir).expect("Failed to install dependencies");

    let lockfile_path = app_dir.join(Lockfile::LOCKFILE_FILENAME);
    assert!(lockfile_path.exists(), "DapCode.lock harus dibuat setelah install");

    let lockfile = Lockfile::from_file(&lockfile_path).expect("Failed to read lockfile");
    assert_eq!(lockfile.versi_format, 1);
    assert!(lockfile.paket.contains_key("matematika"));

    let locked_dep = lockfile.paket.get("matematika").unwrap();
    assert_eq!(locked_dep.nama, "matematika");
    assert_eq!(locked_dep.versi, "1.2.0");
    assert_eq!(locked_dep.sumber, "lokal");
    assert_eq!(locked_dep.hash_sha256.len(), 64, "Hash SHA-256 harus 64 karakter heksadesimal");

    // 5. Verifikasi integritas harus berhasil
    cmd_pkg_check_in(&app_dir).expect("Pemeriksaan integritas lockfile harus sukses");

    // 6. Tampilkan tree tidak boleh crash
    cmd_tree_in(&app_dir).expect("cmd_tree_in harus sukses");

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_lockfile_tamper_detection_and_update() {
    let temp_root = std::env::temp_dir().join(format!("dapcode_tamper_test_{}", std::process::id()));
    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp root");

    // 1. Buat dependensi 'utilitas'
    let dep_dir = temp_root.join("utilitas");
    fs::create_dir_all(dep_dir.join("src")).expect("Failed to create dep src");
    let dep_manifest = r#"
[paket]
nama = "utilitas"
versi = "0.3.0"
berkas_utama = "src/lib.dap"
"#;
    fs::write(dep_dir.join("DapCode.toml"), dep_manifest).expect("Failed to write dep manifest");
    let dep_code_path = dep_dir.join("src").join("lib.dap");
    fs::write(&dep_code_path, "fungsi sapa() -> Teks { kembali \"halo\" }")
        .expect("Failed to write dep lib");

    // 2. Buat proyek konsumen
    let app_dir = temp_root.join("konsumen");
    fs::create_dir_all(app_dir.join("src")).expect("Failed to create app src");
    cmd_init_in(&app_dir, "konsumen").expect("Failed to init app");

    cmd_add_in(
        &app_dir,
        "utilitas",
        Some(PathBuf::from("../utilitas")),
        None,
        None,
    )
    .expect("Failed to add dependency");
    cmd_install_in(&app_dir).expect("Failed to install");

    let lockfile_path = app_dir.join(Lockfile::LOCKFILE_FILENAME);
    let original_lock = Lockfile::from_file(&lockfile_path).expect("Failed to read lock");
    let original_hash = original_lock.paket.get("utilitas").unwrap().hash_sha256.clone();

    // 3. Modifikasi kode dependensi (tampering)
    fs::write(
        &dep_code_path,
        "fungsi sapa() -> Teks { kembali \"halo yang telah diubah secara ilegal!\" }",
    )
    .expect("Failed to modify dep code");

    // 4. Pemeriksaan integritas sekarang HARUS GAGAL
    let check_result = cmd_pkg_check_in(&app_dir);
    assert!(
        check_result.is_err(),
        "Pemeriksaan integritas harus mendeteksi perubahan hash yang tidak sah"
    );

    // 5. Jalankan update untuk memperbarui hash sah di DapCode.lock
    cmd_update_in(&app_dir).expect("cmd_update_in harus sukses memperbarui lockfile");

    let updated_lock = Lockfile::from_file(&lockfile_path).expect("Failed to read updated lock");
    let updated_hash = updated_lock.paket.get("utilitas").unwrap().hash_sha256.clone();
    assert_ne!(
        original_hash, updated_hash,
        "Hash baru harus berbeda dari hash sebelum modifikasi"
    );

    // 6. Pemeriksaan integritas sekarang harus lolos kembali
    cmd_pkg_check_in(&app_dir).expect("Pemeriksaan integritas harus sukses setelah update");

    // 7. Uji remove -> dependensi harus terhapus dari DapCode.toml dan DapCode.lock
    cmd_remove_in(&app_dir, "utilitas").expect("Failed to remove dependency");
    let final_manifest = ProjectManifest::from_file(&app_dir.join(ProjectManifest::MANIFEST_FILENAME))
        .expect("Failed to read final manifest");
    assert!(!final_manifest.dependensi.contains_key("utilitas"));

    let final_lock = Lockfile::from_file(&lockfile_path).expect("Failed to read final lock");
    assert!(!final_lock.paket.contains_key("utilitas"));

    let _ = fs::remove_dir_all(&temp_root);
}
