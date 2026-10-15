use crate::ast::{Decl, Program};
use crate::diag::{Diagnostic, Result};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::span::SourceMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectManifest {
    pub nama: String,
    pub versi: String,
    pub penulis: Vec<String>,
    pub berkas_utama: String,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        ProjectManifest {
            nama: "proyek_dapcode".to_string(),
            versi: "0.1.0".to_string(),
            penulis: Vec::new(),
            berkas_utama: "src/utama.dap".to_string(),
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
                }
            }
        }

        Ok(manifest)
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

    pub fn init_project(target_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
        if !target_dir.exists() {
            fs::create_dir_all(target_dir)?;
        }

        let manifest_path = target_dir.join(Self::MANIFEST_FILENAME);
        let manifest_content = format!(
            r#"[paket]
nama = "{}"
versi = "0.1.0"
penulis = ["Pengembang DapCode"]
berkas_utama = "src/utama.dap"

[dependensi]
"#,
            name
        );
        fs::write(&manifest_path, manifest_content)?;

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
"#;
            fs::write(&gitignore, gitignore_content)?;
        }

        Ok(manifest_path)
    }
}

pub struct ModuleLoader {
    visited: HashSet<PathBuf>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        ModuleLoader {
            visited: HashSet::new(),
        }
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

        let src = fs::read_to_string(&canonical_entry).map_err(|e| {
            Diagnostic::error(format!(
                "Gagal membaca berkas '{}': {}",
                canonical_entry.display(),
                e
            ))
        })?;

        let (file_id, _) = source_map.add_file(
            canonical_entry.to_string_lossy().to_string(),
            src.clone(),
        );

        let mut lexer = Lexer::new(&src, file_id);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens, file_id);
        let current_program = parser.parse_program()?;

        let parent_dir = canonical_entry.parent().unwrap_or_else(|| Path::new("."));

        let mut all_decls = Vec::new();

        // 1. Resolve and load all imported files first
        for decl in &current_program.decls {
            if let Decl::Import(imp) = decl {
                let mut import_path = PathBuf::from(&imp.path);
                if import_path.extension().is_none() {
                    import_path.set_extension("dap");
                }
                let target_path = if import_path.is_absolute() {
                    import_path
                } else {
                    parent_dir.join(import_path)
                };

                let canonical_target = fs::canonicalize(&target_path).map_err(|e| {
                    Diagnostic::error(format!(
                        "Gagal mengimpor '{}': berkas tidak ditemukan di '{}' ({})",
                        imp.path,
                        target_path.display(),
                        e
                    ))
                    .with_span(imp.span)
                })?;

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

        Ok((
            file_id,
            Program {
                decls: all_decls,
                span: current_program.span,
            },
        ))
    }
}
