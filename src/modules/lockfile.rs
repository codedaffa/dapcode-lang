use crate::crypto;
use crate::diag::{Diagnostic, Result};
use crate::modules::{DependencyLocation, ProjectManifest};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedPackage {
    pub nama: String,
    pub versi: String,
    pub sumber: String, // "lokal", "git", "versi"
    pub target: String,
    pub hash_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    pub versi_format: u32,
    pub paket: BTreeMap<String, LockedPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub nama: String,
    pub cocok: bool,
    pub hash_terkunci: String,
    pub hash_aktual: String,
}

impl Default for Lockfile {
    fn default() -> Self {
        Lockfile {
            versi_format: 1,
            paket: BTreeMap::new(),
        }
    }
}

impl Lockfile {
    pub const LOCKFILE_FILENAME: &'static str = "DapCode.lock";

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            Diagnostic::error(format!(
                "Gagal membaca lockfile '{}': {}",
                path.display(),
                e
            ))
        })?;
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> Result<Self> {
        let mut lockfile = Lockfile::default();
        let mut in_paket = false;
        let mut current_pkg: Option<LockedPackage> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed == "[[paket]]" {
                if let Some(pkg) = current_pkg.take() {
                    lockfile.paket.insert(pkg.nama.clone(), pkg);
                }
                in_paket = true;
                current_pkg = Some(LockedPackage {
                    nama: String::new(),
                    versi: "0.1.0".to_string(),
                    sumber: "lokal".to_string(),
                    target: String::new(),
                    hash_sha256: String::new(),
                });
                continue;
            }

            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = Self::strip_quotes(v.trim());

                if !in_paket {
                    if key == "versi_format" {
                        if let Ok(vf) = val.parse::<u32>() {
                            lockfile.versi_format = vf;
                        }
                    }
                } else if let Some(ref mut pkg) = current_pkg {
                    match key {
                        "nama" => pkg.nama = val,
                        "versi" => pkg.versi = val,
                        "sumber" => pkg.sumber = val,
                        "target" => pkg.target = val,
                        "hash_sha256" => pkg.hash_sha256 = val,
                        _ => {}
                    }
                }
            }
        }

        if let Some(pkg) = current_pkg.take() {
            if !pkg.nama.is_empty() {
                lockfile.paket.insert(pkg.nama.clone(), pkg);
            }
        }

        Ok(lockfile)
    }

    pub fn to_lock_string(&self) -> String {
        let mut out = String::new();
        out.push_str("# BERKAS INI DIHASILKAN SECARA OTOMATIS OLEH DAPCODE / DAPPKG.\n");
        out.push_str("# JANGAN EDIT BERKAS INI SECARA MANUAL.\n\n");
        out.push_str(&format!("versi_format = {}\n\n", self.versi_format));

        for pkg in self.paket.values() {
            out.push_str("[[paket]]\n");
            out.push_str(&format!("nama = \"{}\"\n", pkg.nama));
            out.push_str(&format!("versi = \"{}\"\n", pkg.versi));
            out.push_str(&format!("sumber = \"{}\"\n", pkg.sumber));
            out.push_str(&format!("target = \"{}\"\n", pkg.target.replace('\\', "/")));
            out.push_str(&format!("hash_sha256 = \"{}\"\n\n", pkg.hash_sha256));
        }

        out
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        fs::write(path, self.to_lock_string())
    }

    fn strip_quotes(s: &str) -> String {
        let s = s.trim();
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            if s.len() >= 2 {
                s[1..s.len() - 1].to_string()
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        }
    }

    pub fn compute_package_hash(
        loc: &DependencyLocation,
        manifest_dir: &Path,
    ) -> Result<(String, String)> {
        match loc {
            DependencyLocation::Path(rel_p) => {
                let abs_p = if rel_p.is_absolute() {
                    rel_p.clone()
                } else {
                    manifest_dir.join(rel_p)
                };

                if !abs_p.exists() {
                    return Err(Diagnostic::error(format!(
                        "Jalur dependensi lokal '{}' tidak ditemukan",
                        abs_p.display()
                    )));
                }

                // Ambil versi dari manifest paket jika ada
                let mut pkg_version = "0.1.0".to_string();
                let pkg_manifest = abs_p.join(ProjectManifest::MANIFEST_FILENAME);
                if pkg_manifest.exists() {
                    if let Ok(m) = ProjectManifest::from_file(&pkg_manifest) {
                        pkg_version = m.versi;
                    }
                }

                // Kumpulkan semua berkas .dap dan DapCode.toml untuk dihitung hash agregatnya
                let mut files = Vec::new();
                Self::collect_package_files(&abs_p, &abs_p, &mut files)?;
                files.sort_by(|a, b| a.0.cmp(&b.0));

                let mut hash_payload = String::new();
                for (rel_file, content) in files {
                    hash_payload.push_str(&rel_file);
                    hash_payload.push(':');
                    hash_payload.push_str(&content);
                    hash_payload.push('\n');
                }

                if hash_payload.is_empty() {
                    hash_payload = format!("path:{}", abs_p.display());
                }

                let hash = crypto::sha256(&hash_payload);
                Ok((hash, pkg_version))
            }
            DependencyLocation::Git(url) => {
                let cache_dir = manifest_dir.join(".dapcode").join("dependensi");
                let mut hash_payload = format!("git:{}", url);

                // Jika ada cache lokal dari repo git
                if cache_dir.exists() {
                    let mut files = Vec::new();
                    let _ = Self::collect_package_files(&cache_dir, &cache_dir, &mut files);
                    files.sort_by(|a, b| a.0.cmp(&b.0));
                    for (rel_file, content) in files {
                        hash_payload.push_str(&rel_file);
                        hash_payload.push(':');
                        hash_payload.push_str(&content);
                        hash_payload.push('\n');
                    }
                }

                let hash = crypto::sha256(&hash_payload);
                Ok((hash, "git-latest".to_string()))
            }
            DependencyLocation::Version(ver) => {
                let hash_payload = format!("registry:versi:{}", ver);
                let hash = crypto::sha256(&hash_payload);
                Ok((hash, ver.clone()))
            }
        }
    }

    fn collect_package_files(
        root: &Path,
        current: &Path,
        files: &mut Vec<(String, String)>,
    ) -> Result<()> {
        if current.is_file() {
            if let Ok(content) = fs::read_to_string(current) {
                let rel = current
                    .strip_prefix(root)
                    .unwrap_or(current)
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((rel, content));
            }
            return Ok(());
        }

        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Abaikan direktori tersembunyi atau target build
                if name.starts_with('.') || name == "target" || name == "build" {
                    continue;
                }

                if path.is_dir() {
                    Self::collect_package_files(root, &path, files)?;
                } else if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "dap" || name == ProjectManifest::MANIFEST_FILENAME {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let rel = path
                                .strip_prefix(root)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .replace('\\', "/");
                            files.push((rel, content));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn generate_or_update(
        manifest: &ProjectManifest,
        manifest_dir: &Path,
    ) -> Result<Lockfile> {
        let mut lockfile = Lockfile::default();

        for (name, loc) in &manifest.dependensi {
            let (hash, versi) = Self::compute_package_hash(loc, manifest_dir)?;
            let (sumber, target) = match loc {
                DependencyLocation::Path(p) => (
                    "lokal".to_string(),
                    p.to_string_lossy().replace('\\', "/"),
                ),
                DependencyLocation::Git(url) => ("git".to_string(), url.clone()),
                DependencyLocation::Version(ver) => ("versi".to_string(), ver.clone()),
            };

            lockfile.paket.insert(
                name.clone(),
                LockedPackage {
                    nama: name.clone(),
                    versi,
                    sumber,
                    target,
                    hash_sha256: hash,
                },
            );
        }

        Ok(lockfile)
    }

    pub fn verify_integrity(
        &self,
        manifest: &ProjectManifest,
        manifest_dir: &Path,
    ) -> Result<Vec<VerificationReport>> {
        let mut reports = Vec::new();

        for (name, loc) in &manifest.dependensi {
            let locked = match self.paket.get(name) {
                Some(p) => p,
                None => {
                    return Err(Diagnostic::error(format!(
                        "Dependensi '{}' terdaftar di DapCode.toml tetapi belum terkunci di DapCode.lock. Jalankan 'dappkg install' atau 'dappkg update'.",
                        name
                    )));
                }
            };

            let (actual_hash, _) = Self::compute_package_hash(loc, manifest_dir)?;
            let cocok = locked.hash_sha256 == actual_hash;

            reports.push(VerificationReport {
                nama: name.clone(),
                cocok,
                hash_terkunci: locked.hash_sha256.clone(),
                hash_aktual: actual_hash.clone(),
            });

            if !cocok {
                return Err(Diagnostic::error(format!(
                    "Integritas dependensi '{}' gagal! Hash SHA-256 berkas tidak cocok dengan DapCode.lock.\n  Diharapkan: {}\n  Aktual:     {}\nPetunjuk: Jalankan 'dappkg update' jika berkas memang sengaja dimodifikasi.",
                    name, locked.hash_sha256, actual_hash
                )));
            }
        }

        Ok(reports)
    }
}
