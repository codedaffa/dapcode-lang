use crate::ast::{Decl, Program};
use crate::diag::{Diagnostic, Result};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::span::{SourceMap, Span};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub mod lockfile;
pub use lockfile::{Lockfile, LockedPackage, VerificationReport};

#[derive(Debug, Clone, PartialEq)]
pub enum DependencyLocation {
    Path(PathBuf),
    Git(String),
    Version(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectManifest {
    pub nama: String,
    pub versi: String,
    pub penulis: Vec<String>,
    pub berkas_utama: String,
    pub dependensi: BTreeMap<String, DependencyLocation>,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        ProjectManifest {
            nama: "proyek_dapcode".to_string(),
            versi: "0.1.0".to_string(),
            penulis: Vec::new(),
            berkas_utama: "src/utama.dap".to_string(),
            dependensi: BTreeMap::new(),
        }
    }
}

impl ProjectManifest {
    pub const MANIFEST_FILENAME: &'static str = "DapCode.toml";

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            Diagnostic::error(format!("Gagal membaca manifest '{}': {}", path.display(), e))
        })?;
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> Result<Self> {
        let mut manifest = ProjectManifest::default();
        let mut current_section = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].trim().to_string();
                continue;
            }

            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = v.trim();

                if current_section == "paket" {
                    match key {
                        "nama" => {
                            manifest.nama = Self::strip_quotes(val);
                        }
                        "versi" => {
                            manifest.versi = Self::strip_quotes(val);
                        }
                        "berkas_utama" => {
                            manifest.berkas_utama = Self::strip_quotes(val);
                        }
                        "penulis" => {
                            if val.starts_with('[') && val.ends_with(']') {
                                let inner = &val[1..val.len() - 1];
                                manifest.penulis = inner
                                    .split(',')
                                    .map(|s| Self::strip_quotes(s.trim()))
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                        }
                        _ => {}
                    }
                } else if current_section == "dependensi" {
                    if val.starts_with('{') && val.ends_with('}') {
                        let inner = &val[1..val.len() - 1].trim();
                        for field in inner.split(',') {
                            if let Some((fk, fv)) = field.split_once('=') {
                                let f_key = fk.trim();
                                let f_val = Self::strip_quotes(fv.trim());
                                if f_key == "jalur" || f_key == "path" {
                                    manifest.dependensi.insert(
                                        key.to_string(),
                                        DependencyLocation::Path(PathBuf::from(f_val)),
                                    );
                                } else if f_key == "git" {
                                    manifest.dependensi.insert(
                                        key.to_string(),
                                        DependencyLocation::Git(f_val),
                                    );
                                } else if f_key == "versi" || f_key == "version" {
                                    manifest.dependensi.insert(
                                        key.to_string(),
                                        DependencyLocation::Version(f_val),
                                    );
                                }
                            }
                        }
                    } else {
                        let clean = Self::strip_quotes(val);
                        if clean.contains('/') || clean.contains('\\') {
                            manifest.dependensi.insert(
                                key.to_string(),
                                DependencyLocation::Path(PathBuf::from(clean)),
                            );
                        } else {
                            manifest.dependensi.insert(
                                key.to_string(),
                                DependencyLocation::Version(clean),
                            );
                        }
                    }
                }
            }
        }

        Ok(manifest)
    }

    pub fn to_toml_string(&self) -> String {
        let mut out = String::new();
        out.push_str("[paket]\n");
        out.push_str(&format!("nama = \"{}\"\n", self.nama));
        out.push_str(&format!("versi = \"{}\"\n", self.versi));
        let penulis_str = self
            .penulis
            .iter()
            .map(|p| format!("\"{}\"", p))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("penulis = [{}]\n", penulis_str));
        out.push_str(&format!("berkas_utama = \"{}\"\n\n", self.berkas_utama));

        out.push_str("[dependensi]\n");
        for (name, loc) in &self.dependensi {
            match loc {
                DependencyLocation::Path(p) => {
                    let p_str = p.to_string_lossy().replace('\\', "/");
                    out.push_str(&format!("{} = {{ jalur = \"{}\" }}\n", name, p_str));
                }
                DependencyLocation::Git(url) => {
                    out.push_str(&format!("{} = {{ git = \"{}\" }}\n", name, url));
                }
                DependencyLocation::Version(v) => {
                    out.push_str(&format!("{} = \"{}\"\n", name, v));
                }
            }
        }

        out
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        fs::write(path, self.to_toml_string())
    }

    pub fn add_dependency(&mut self, name: &str, loc: DependencyLocation) {
        self.dependensi.insert(name.to_string(), loc);
    }

    pub fn remove_dependency(&mut self, name: &str) -> bool {
        self.dependensi.remove(name).is_some()
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

    pub fn find_in_dir(dir: &Path) -> Option<PathBuf> {
        let manifest_path = dir.join(Self::MANIFEST_FILENAME);
        if manifest_path.exists() {
            Some(manifest_path)
        } else {
            None
        }
    }

    pub fn find_upwards(start: &Path) -> Option<(PathBuf, ProjectManifest)> {
        let mut curr = if start.is_file() {
            start.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
        } else {
            start.to_path_buf()
        };

        loop {
            let manifest_path = curr.join(Self::MANIFEST_FILENAME);
            if manifest_path.exists() {
                if let Ok(m) = Self::from_file(&manifest_path) {
                    return Some((manifest_path, m));
                }
            }
            if !curr.pop() {
                break;
            }
        }
        None
    }

    pub fn init_project(target_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
        if !target_dir.exists() {
            fs::create_dir_all(target_dir)?;
        }

        let manifest = ProjectManifest {
            nama: name.to_string(),
            versi: "0.1.0".to_string(),
            penulis: vec!["Pengembang DapCode".to_string()],
            berkas_utama: "src/utama.dap".to_string(),
            dependensi: BTreeMap::new(),
        };

        let manifest_path = target_dir.join(Self::MANIFEST_FILENAME);
        manifest.save_to_file(&manifest_path)?;

        let src_dir = target_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        let entry_file = src_dir.join("utama.dap");
        if !entry_file.exists() {
            let entry_content = format!(
                r#"// Titik masuk utama untuk proyek {}

fungsi utama() {{
    cetak("Halo dari proyek DapCode: {}!")
}}
"#,
                name, name
            );
            fs::write(&entry_file, entry_content)?;
        }

        let gitignore = target_dir.join(".gitignore");
        if !gitignore.exists() {
            let gitignore_content = r#"# DapCode build output
target/
*.exe
*.c
*.o
.dapcode/
"#;
            fs::write(&gitignore, gitignore_content)?;
        }

        Ok(manifest_path)
    }
}

pub type CustomFileReader = Box<dyn Fn(&Path) -> Option<String> + Send + Sync>;

pub struct ModuleLoader {
    visited: HashSet<PathBuf>,
    pub file_reader: Option<CustomFileReader>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        ModuleLoader {
            visited: HashSet::new(),
            file_reader: None,
        }
    }

    pub fn with_file_reader<F>(mut self, reader: F) -> Self
    where
        F: Fn(&Path) -> Option<String> + Send + Sync + 'static,
    {
        self.file_reader = Some(Box::new(reader));
        self
    }

    /// Recursively loads an entry file and all its imported dependencies into a unified Program AST
    pub fn load_program(
        &mut self,
        entry_path: &Path,
        source_map: &mut SourceMap,
    ) -> Result<(usize, Program)> {
        let canonical_entry = fs::canonicalize(entry_path).map_err(|e| {
            Diagnostic::error(format!(
                "Tidak dapat menemukan berkas '{}': {}",
                entry_path.display(),
                e
            ))
        })?;

        self.visited.insert(canonical_entry.clone());

        let src = if let Some(ref reader) = self.file_reader {
            if let Some(s) = reader(&canonical_entry) {
                s
            } else {
                fs::read_to_string(&canonical_entry).map_err(|e| {
                    Diagnostic::error(format!(
                        "Gagal membaca berkas '{}': {}",
                        canonical_entry.display(),
                        e
                    ))
                })?
            }
        } else {
            fs::read_to_string(&canonical_entry).map_err(|e| {
                Diagnostic::error(format!(
                    "Gagal membaca berkas '{}': {}",
                    canonical_entry.display(),
                    e
                ))
            })?
        };

        let (file_id, _) = source_map.add_file(
            canonical_entry.to_string_lossy().to_string(),
            src.clone(),
        );

        let mut lexer = Lexer::new(&src, file_id);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens, file_id);
        let current_program = parser.parse_program()?;

        let prog = self.load_program_from_ast(&canonical_entry, current_program, file_id, source_map)?;
        Ok((file_id, prog))
    }

    /// Recursively loads dependencies for an already-parsed Program AST into a unified Program AST
    pub fn load_program_from_ast(
        &mut self,
        entry_path: &Path,
        current_program: Program,
        _file_id: usize,
        source_map: &mut SourceMap,
    ) -> Result<Program> {
        let canonical_entry = fs::canonicalize(entry_path).unwrap_or_else(|_| entry_path.to_path_buf());
        self.visited.insert(canonical_entry.clone());

        let parent_dir = canonical_entry.parent().unwrap_or_else(|| Path::new("."));

        let mut all_decls = Vec::new();

        // 1. Resolve and load all imported files / packages first
        for decl in &current_program.decls {
            if let Decl::Import(imp) = decl {
                let canonical_target = self.resolve_import_path(&imp.path, parent_dir, imp.span)?;

                // If not already visited in this session, resolve its dependencies recursively
                if !self.visited.contains(&canonical_target) {
                    let (_, imported_prog) = self.load_program(&canonical_target, source_map)?;
                    for d in imported_prog.decls {
                        if !matches!(d, Decl::Import(_)) {
                            all_decls.push(d);
                        }
                    }
                }
            }
        }

        // 2. Append current file declarations
        for decl in current_program.decls {
            if !matches!(decl, Decl::Import(_)) {
                all_decls.push(decl);
            }
        }

        Ok(Program {
            decls: all_decls,
            span: current_program.span,
        })
    }

    pub fn resolve_import_path(&self, import_str: &str, parent_dir: &Path, span: Span) -> Result<PathBuf> {
        // 1. Cek jalur berkas relatif terlebih dahulu
        let mut rel_path = PathBuf::from(import_str);
        if rel_path.extension().is_none() {
            rel_path.set_extension("dap");
        }
        let target_path = if rel_path.is_absolute() {
            rel_path
        } else {
            parent_dir.join(&rel_path)
        };

        if target_path.exists() {
            return fs::canonicalize(&target_path).map_err(|e| {
                Diagnostic::error(format!("Gagal membuka '{}': {}", target_path.display(), e))
                    .with_span(span)
            });
        }

        // 2. Cek dependensi paket di DapCode.toml
        if let Some((manifest_path, manifest)) = ProjectManifest::find_upwards(parent_dir) {
            let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
            let parts: Vec<&str> = import_str.split(['/', '\\']).collect();
            let dep_name = parts[0];
            let sub_path = if parts.len() > 1 {
                Some(parts[1..].join("/"))
            } else {
                None
            };

            if let Some(loc) = manifest.dependensi.get(dep_name) {
                let vendor_dir = manifest_dir.join("vendor").join(dep_name);
                let pkg_dir = if vendor_dir.exists() && vendor_dir.is_dir() {
                    vendor_dir
                } else {
                    match loc {
                        DependencyLocation::Path(p) => {
                            if p.is_absolute() {
                                p.clone()
                            } else {
                                manifest_dir.join(p)
                            }
                        }
                        DependencyLocation::Git(_) | DependencyLocation::Version(_) => {
                            manifest_dir.join(".dapcode").join("dependensi").join(dep_name)
                        }
                    }
                };

                if !pkg_dir.exists() {
                    return Err(Diagnostic::error(format!(
                        "Direktori paket dependensi '{}' tidak ditemukan di '{}'. Jalankan 'dapcodec install' terlebih dahulu.",
                        dep_name,
                        pkg_dir.display()
                    ))
                    .with_span(span));
                }

                // Tentukan berkas entry paket
                let entry_file = if let Some(sub) = sub_path {
                    let mut sub_file = PathBuf::from(&sub);
                    if sub_file.extension().is_none() {
                        sub_file.set_extension("dap");
                    }
                    let cand1 = pkg_dir.join("src").join(&sub_file);
                    let cand2 = pkg_dir.join(&sub_file);
                    if cand1.exists() {
                        cand1
                    } else {
                        cand2
                    }
                } else {
                    // Cari berkas_utama dari DapCode.toml paket
                    let pkg_manifest_path = pkg_dir.join(ProjectManifest::MANIFEST_FILENAME);
                    if pkg_manifest_path.exists() {
                        if let Ok(pkg_m) = ProjectManifest::from_file(&pkg_manifest_path) {
                            pkg_dir.join(pkg_m.berkas_utama)
                        } else {
                            pkg_dir.join("src/lib.dap")
                        }
                    } else {
                        let cand_lib = pkg_dir.join("src/lib.dap");
                        let cand_utama = pkg_dir.join("src/utama.dap");
                        let cand_named = pkg_dir.join(format!("{}.dap", dep_name));
                        if cand_lib.exists() {
                            cand_lib
                        } else if cand_utama.exists() {
                            cand_utama
                        } else {
                            cand_named
                        }
                    }
                };

                if entry_file.exists() {
                    return fs::canonicalize(&entry_file).map_err(|e| {
                        Diagnostic::error(format!("Gagal membuka '{}': {}", entry_file.display(), e))
                            .with_span(span)
                    });
                } else {
                    return Err(Diagnostic::error(format!(
                        "Tidak dapat menemukan titik masuk untuk dependensi '{}' pada '{}'",
                        dep_name,
                        entry_file.display()
                    ))
                    .with_span(span));
                }
            }
        }

        Err(Diagnostic::error(format!(
            "Gagal mengimpor '{}': berkas tidak ditemukan di jalur relatif '{}' dan tidak terdaftar pada [dependensi] di DapCode.toml",
            import_str,
            target_path.display()
        ))
        .with_span(span))
    }
}

/// Mengunduh repositori Git remote ke direktori cache lokal secara otomatis
pub fn fetch_git_dependency(url: &str, target_dir: &Path) -> std::io::Result<()> {
    if target_dir.exists() && target_dir.is_dir() {
        if target_dir.join(".git").exists() {
            let _ = std::process::Command::new("git")
                .current_dir(target_dir)
                .args(["pull", "--ff-only"])
                .output();
        }
        return Ok(());
    }

    if let Some(parent) = target_dir.parent() {
        let _ = fs::create_dir_all(parent);
    }

    match std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, &target_dir.to_string_lossy()])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let _ = fs::create_dir_all(target_dir);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Git clone gagal untuk '{}': {}", url, stderr.trim()),
                ))
            }
        }
        Err(e) => {
            let _ = fs::create_dir_all(target_dir);
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Gagal mengeksekusi git (apakah git terpasang?): {}", e),
            ))
        }
    }
}

/// Menyalin seluruh berkas direktori secara rekursif (mengabaikan berkas tersembunyi & build target)
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if name_str.starts_with('.') || name_str == "target" || name_str == "build" {
            continue;
        }
        let dest_path = dst.join(&file_name);
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
