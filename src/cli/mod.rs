use clap::{Parser as ClapParser, Subcommand};
use colored::*;
use crate::ast::Decl;
use crate::codegen::CodeGenerator;
use crate::comptime::ComptimeEvaluator;
use crate::eval::{ControlFlow, Environment, Evaluator, Value};
use crate::fmt::Formatter;
use crate::ir::IRBuilder;
use crate::lexer::Lexer;
use crate::lsp::LspServer;
use crate::modules::{DependencyLocation, ModuleLoader, ProjectManifest};
use crate::ownership::OwnershipAnalyzer;
use crate::parser::{Parser, Precedence};
use crate::span::SourceMap;
use crate::types::TypeChecker;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(ClapParser)]
#[command(
    name = "dapcodec",
    author = "DapCode Core Team",
    version = "0.1.0",
    about = "Kompilator dan Toolchain Resmi untuk Bahasa Pemrograman DapCode (.dap)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Menginisialisasi proyek DapCode baru dengan manifes DapCode.toml
    Init {
        /// Nama proyek atau direktori target (opsional)
        name: Option<String>,
    },
    /// Menjalankan program DapCode (.dap) secara langsung
    Run {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
    },
    /// Memeriksa tipe (type check) dan analisis kepemilikan (ownership) tanpa eksekusi
    Check {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
    },
    /// Mengompilasi program DapCode ke berkas binary mandiri (native executable)
    Build {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
        /// Berkas keluaran binary
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Menampilkan pohon sintaks abstrak (Abstract Syntax Tree / AST)
    DumpAst {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
    },
    /// Menampilkan Representasi Perantara (IR) dengan anotasi drop deterministik Perceus
    DumpIr {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
    },
    /// Menampilkan laporan rinci analisis auto-ownership dan Perceus
    DumpOwnership {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
    },
    /// Memformat kode sumber DapCode (.dap)
    Fmt {
        /// Berkas atau direktori sumber .dap
        path: Option<PathBuf>,
        /// Periksa format tanpa menulis perubahan
        #[arg(short, long)]
        check: bool,
    },
    /// Menjalankan Language Server Protocol (LSP) untuk integrasi editor/IDE
    Lsp,
    /// Memulai konsol interaktif DapCode (REPL)
    Repl,
    /// Menambahkan dependensi paket baru ke DapCode.toml
    Add {
        /// Nama paket dependensi
        name: String,
        /// Jalur direktori lokal ke paket (opsional)
        #[arg(long, alias = "jalur")]
        path: Option<PathBuf>,
        /// URL Git repositori paket (opsional)
        #[arg(long)]
        git: Option<String>,
        /// Versi paket semver (opsional, default: "0.1.0")
        #[arg(short, long, alias = "versi")]
        version: Option<String>,
    },
    /// Memverifikasi dan memasang semua dependensi yang tertera di DapCode.toml
    Install,
    /// Menghapus dependensi dari DapCode.toml
    Remove {
        /// Nama paket dependensi yang akan dihapus
        name: String,
    },
    /// Menjalankan pengujian terintegrasi (blok 'uji "..."')
    Test {
        /// Berkas sumber .dap atau direktori pengujian (opsional)
        file: Option<PathBuf>,
        /// Filter nama pengujian tertentu
        #[arg(short, long)]
        filter: Option<String>,
    },
}

pub fn run_cli() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => cmd_init(name.as_deref()),
        Commands::Run { file } => cmd_run(file.as_deref()),
        Commands::Check { file } => cmd_check(file.as_deref()),
        Commands::Build { file, output } => cmd_build(file.as_deref(), output),
        Commands::DumpAst { file } => cmd_dump_ast(file.as_deref()),
        Commands::DumpIr { file } => cmd_dump_ir(file.as_deref()),
        Commands::DumpOwnership { file } => cmd_dump_ownership(file.as_deref()),
        Commands::Fmt { path, check } => cmd_fmt(path.as_deref(), check),
        Commands::Lsp => cmd_lsp(),
        Commands::Repl => cmd_repl(),
        Commands::Add {
            name,
            path,
            git,
            version,
        } => cmd_add(&name, path, git, version),
        Commands::Install => cmd_install(),
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Test { file, filter } => cmd_test(file.as_deref(), filter.as_deref()),
    }
}

fn resolve_target_file(file: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(f) = file {
        return Ok(f.to_path_buf());
    }

    if let Some(manifest_path) = ProjectManifest::find_in_dir(Path::new(".")) {
        if let Ok(manifest) = ProjectManifest::from_file(&manifest_path) {
            let entry = PathBuf::from(&manifest.berkas_utama);
            if entry.exists() {
                return Ok(entry);
            } else {
                eprintln!(
                    "{}: Berkas utama '{}' yang ditentukan di {} tidak ditemukan.",
                    "kesalahan".red().bold(),
                    manifest.berkas_utama,
                    ProjectManifest::MANIFEST_FILENAME
                );
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Berkas utama manifest tidak ditemukan",
                ));
            }
        }
    }

    eprintln!(
        "{}: Tidak ada berkas yang ditentukan dan '{}' tidak ditemukan di direktori saat ini.\nPetunjuk: Tentukan berkas (.dap) atau jalankan '{}' untuk membuat proyek baru.",
        "kesalahan".red().bold(),
        ProjectManifest::MANIFEST_FILENAME,
        "dapcodec init".cyan()
    );
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Berkas tidak ditentukan",
    ))
}

fn load_program_and_modules(file: &Path, sm: &mut SourceMap) -> Option<(usize, crate::ast::Program)> {
    let mut loader = ModuleLoader::new();
    match loader.load_program(file, sm) {
        Ok(res) => Some(res),
        Err(diag) => {
            eprint!("{}", diag.render(sm));
            None
        }
    }
}

fn cmd_init(name: Option<&str>) -> io::Result<()> {
    let (target_dir, project_name) = match name {
        Some(n) if n != "." => {
            let p = Path::new(n);
            let dir_name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| n.to_string());
            (p, dir_name)
        }
        _ => {
            let default_name = std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "proyek_dapcode".to_string());
            (Path::new("."), default_name)
        }
    };

    match ProjectManifest::init_project(target_dir, &project_name) {
        Ok(_manifest_path) => {
            println!(
                "{} Proyek DapCode '{}' berhasil diinisialisasi!",
                "sukses:".green().bold(),
                project_name.cyan()
            );
            println!("  Berkas dibuat di '{}':", target_dir.display());
            println!("    - {}", ProjectManifest::MANIFEST_FILENAME.bold());
            println!("    - {}", "src/utama.dap".bold());
            println!("    - {}", ".gitignore".bold());
            println!("\n  Jalankan proyek dengan:");
            if target_dir != Path::new(".") {
                println!("    cd {}", target_dir.display());
            }
            println!("    {}", "dapcodec run".cyan());
            Ok(())
        }
        Err(e) => {
            eprintln!("{} Gagal menginisialisasi proyek: {}", "kesalahan:".red().bold(), e);
            Err(e)
        }
    }
}

fn cmd_run(file: Option<&Path>) -> io::Result<()> {
    let target = resolve_target_file(file)?;
    let mut sm = SourceMap::new();
    let (file_id, mut program) = match load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    // 1. Comptime Metaprogramming folding
    let mut comptime = ComptimeEvaluator::new();
    if let Err(diag) = comptime.fold_program(&mut program) {
        eprint!("{}", diag.render(&sm));
        return Ok(());
    }

    // 2. Hindley-Milner Type Checking
    let mut checker = TypeChecker::new(file_id);
    if let Err(diag) = checker.check_program(&program) {
        eprint!("{}", diag.render(&sm));
        return Ok(());
    }

    // 3. Automatic Ownership & Perceus Analysis
    let _ownership = OwnershipAnalyzer::new().analyze_program(&program);

    // 4. Direct Execution (Evaluator & Concurrency Runtime)
    let mut evaluator = Evaluator::new();
    if let Err(diag) = evaluator.run_program(&program) {
        eprint!("{}", diag.render(&sm));
    }

    Ok(())
}

fn cmd_check(file: Option<&Path>) -> io::Result<()> {
    let target = resolve_target_file(file)?;
    let mut sm = SourceMap::new();
    let (file_id, mut program) = match load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    let mut comptime = ComptimeEvaluator::new();
    if let Err(diag) = comptime.fold_program(&mut program) {
        eprint!("{}", diag.render(&sm));
        return Ok(());
    }

    let mut checker = TypeChecker::new(file_id);
    if let Err(diag) = checker.check_program(&program) {
        eprint!("{}", diag.render(&sm));
        return Ok(());
    }

    let _ownership = OwnershipAnalyzer::new().analyze_program(&program);

    println!(
        "{} Berkas '{}' (beserta modul impor) lolos pemeriksaan tipe dan validasi auto-ownership!",
        "sukses:".green().bold(),
        target.display()
    );
    Ok(())
}

fn cmd_dump_ast(file: Option<&Path>) -> io::Result<()> {
    let target = resolve_target_file(file)?;
    let mut sm = SourceMap::new();
    let (_, program) = match load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    println!("{:#?}", program);
    Ok(())
}

fn cmd_dump_ir(file: Option<&Path>) -> io::Result<()> {
    let target = resolve_target_file(file)?;
    let mut sm = SourceMap::new();
    let (_, mut program) = match load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    let mut comptime = ComptimeEvaluator::new();
    let _ = comptime.fold_program(&mut program);

    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut ir_builder = IRBuilder::new(&ownership);
    let ir_program = ir_builder.build_program(&program);

    println!("{}", ir_program);
    Ok(())
}

fn cmd_dump_ownership(file: Option<&Path>) -> io::Result<()> {
    let target = resolve_target_file(file)?;
    let mut sm = SourceMap::new();
    let (_, program) = match load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    println!("{}", ownership);
    Ok(())
}

fn cmd_build(file: Option<&Path>, output: Option<PathBuf>) -> io::Result<()> {
    let target = resolve_target_file(file)?;
    let mut sm = SourceMap::new();
    let (file_id, mut program) = match load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    let mut comptime = ComptimeEvaluator::new();
    let _ = comptime.fold_program(&mut program);

    let mut checker = TypeChecker::new(file_id);
    if let Err(diag) = checker.check_program(&program) {
        eprint!("{}", diag.render(&sm));
        return Ok(());
    }

    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);

    let target_out = output.unwrap_or_else(|| {
        if cfg!(windows) {
            target.with_extension("exe")
        } else {
            target.with_extension("")
        }
    });

    println!(
        "{} Mengompilasi '{}' (beserta modul impor) ke biner mandiri '{}'...",
        "info:".blue().bold(),
        target.display(),
        target_out.display()
    );

    match codegen.compile_to_native(&program, &target_out) {
        Ok(_) => {
            println!(
                "{} Berhasil mengompilasi '{}'!",
                "sukses:".green().bold(),
                target_out.display()
            );
        }
        Err(diag) => {
            eprint!("{}", diag.render(&sm));
        }
    }

    Ok(())
}

fn cmd_repl() -> io::Result<()> {
    println!(
        "{}",
        "=== DapCode Interactive REPL v0.1.0 (Ketik 'keluar' untuk mengakhiri) ==="
            .cyan()
            .bold()
    );

    let mut env = Environment::new();
    let mut evaluator = Evaluator::new();
    let mut line_buf = String::new();

    loop {
        print!("{} ", "dap>".yellow().bold());
        io::stdout().flush()?;

        line_buf.clear();
        if io::stdin().read_line(&mut line_buf)? == 0 {
            break;
        }

        let trimmed = line_buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "keluar" || trimmed == "exit" {
            break;
        }

        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file("<repl>", trimmed.to_string());
        let mut lexer = Lexer::new(trimmed, file_id);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(diag) => {
                eprint!("{}", diag.render(&sm));
                continue;
            }
        };

        let mut parser = Parser::new(tokens, file_id);
        // Try parsing as expression first
        match parser.parse_expr(Precedence::Lowest) {
            Ok(expr) => match evaluator.eval_expr(&expr, &mut env) {
                Ok(v) => println!("{} {}", "=>".green().bold(), v),
                Err(diag) => eprintln!("{}", diag.render(&sm)),
            },
            Err(_) => {
                // Try parsing as block of statements
                let wrapped = format!("fungsi __repl_tmp() {{\n{}\n}}", trimmed);
                let (w_id, _) = sm.add_file("<repl_wrap>", wrapped.clone());
                let mut w_lex = Lexer::new(&wrapped, w_id);
                if let Ok(w_tokens) = w_lex.tokenize() {
                    let mut w_parser = Parser::new(w_tokens, w_id);
                    if let Ok(prog) = w_parser.parse_program() {
                        if let Some(Decl::Fn(f)) = prog.decls.into_iter().next() {
                            match evaluator.eval_block(&f.body, &mut env) {
                                Ok(ControlFlow::Next(v)) => {
                                    if !matches!(v, Value::Unit) {
                                        println!("{} {}", "=>".green().bold(), v);
                                    }
                                }
                                Ok(ControlFlow::Return(v)) => {
                                    println!("{} {}", "=>".green().bold(), v);
                                }
                                Ok(ControlFlow::Fail(e)) => {
                                    eprintln!("{}: {}", "Gagal".red().bold(), e);
                                }
                                Err(diag) => eprintln!("{}", diag.render(&sm)),
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn cmd_fmt(path_opt: Option<&Path>, check: bool) -> io::Result<()> {
    let target = path_opt.unwrap_or_else(|| Path::new("."));
    let mut files_to_format = Vec::new();

    if target.is_file() {
        files_to_format.push(target.to_path_buf());
    } else if target.is_dir() {
        collect_dap_files(target, &mut files_to_format)?;
    } else {
        eprintln!("{}: Path '{}' tidak ditemukan", "kesalahan".red().bold(), target.display());
        return Ok(());
    }

    if files_to_format.is_empty() {
        println!("Tidak ada berkas .dap yang ditemukan.");
        return Ok(());
    }

    let mut unformatted_count = 0;

    for file in &files_to_format {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: Gagal membaca '{}': {}", "kesalahan".red().bold(), file.display(), e);
                continue;
            }
        };

        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file(file, content.clone());
        let mut lexer = Lexer::new(&content, file_id);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(diag) => {
                eprintln!("{}: {}", file.display(), diag.render(&sm));
                continue;
            }
        };

        let mut parser = Parser::new(tokens, file_id);
        let program = match parser.parse_program() {
            Ok(p) => p,
            Err(diag) => {
                eprintln!("{}: {}", file.display(), diag.render(&sm));
                continue;
            }
        };

        let mut formatter = Formatter::new();
        let formatted = formatter.format_program(&program);

        if content != formatted {
            unformatted_count += 1;
            if check {
                println!("{}: Memerlukan pemformatan: {}", "tidak rapi".yellow().bold(), file.display());
            } else {
                if let Err(e) = fs::write(file, &formatted) {
                    eprintln!("{}: Gagal menulis ke '{}': {}", "kesalahan".red().bold(), file.display(), e);
                } else {
                    println!("{}: Berhasil diformat: {}", "rapi".green().bold(), file.display());
                }
            }
        } else if !check {
            println!("{}: Sudah rapi: {}", "ok".cyan().bold(), file.display());
        }
    }

    if check {
        if unformatted_count > 0 {
            println!("{}: {} berkas memerlukan pemformatan.", "peringatan".yellow().bold(), unformatted_count);
        } else {
            println!("{}: Semua berkas .dap sudah terformat dengan rapi!", "sukses".green().bold());
        }
    }

    Ok(())
}

fn collect_dap_files(dir: &Path, list: &mut Vec<PathBuf>) -> io::Result<()> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip target and hidden directories
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "target" {
                    collect_dap_files(&path, list)?;
                }
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("dap") {
                list.push(path);
            }
        }
    }
    Ok(())
}

fn cmd_lsp() -> io::Result<()> {
    let mut server = LspServer::new();
    server.start()
}

pub fn cmd_add(
    name: &str,
    path: Option<PathBuf>,
    git: Option<String>,
    version: Option<String>,
) -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    cmd_add_in(&current_dir, name, path, git, version)
}

pub fn cmd_add_in(
    dir: &Path,
    name: &str,
    path: Option<PathBuf>,
    git: Option<String>,
    version: Option<String>,
) -> io::Result<()> {
    let (manifest_path, mut manifest) = match ProjectManifest::find_upwards(dir) {
        Some(m) => m,
        None => {
            eprintln!(
                "{}: Tidak dapat menemukan '{}' di direktori saat ini atau direktori induk.\nPetunjuk: Jalankan '{}' untuk membuat proyek baru.",
                "kesalahan".red().bold(),
                ProjectManifest::MANIFEST_FILENAME,
                "dapcodec init".cyan()
            );
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "DapCode.toml tidak ditemukan",
            ));
        }
    };

    let loc = if let Some(p) = path {
        DependencyLocation::Path(p)
    } else if let Some(g) = git {
        DependencyLocation::Git(g)
    } else if let Some(v) = version {
        DependencyLocation::Version(v)
    } else {
        let p = PathBuf::from(name);
        if p.exists() && p.is_dir() {
            let pkg_name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(name)
                .to_string();
            manifest.add_dependency(&pkg_name, DependencyLocation::Path(p));
            manifest.save_to_file(&manifest_path)?;
            println!(
                "{} Berhasil menambahkan dependensi lokal '{}' ke {}",
                "sukses:".green().bold(),
                pkg_name.cyan(),
                ProjectManifest::MANIFEST_FILENAME.bold()
            );
            return Ok(());
        }
        DependencyLocation::Version("0.1.0".to_string())
    };

    let loc_desc = match &loc {
        DependencyLocation::Path(p) => format!("jalur = \"{}\"", p.display()),
        DependencyLocation::Git(g) => format!("git = \"{}\"", g),
        DependencyLocation::Version(v) => format!("versi = \"{}\"", v),
    };

    manifest.add_dependency(name, loc);
    manifest.save_to_file(&manifest_path)?;

    println!(
        "{} Berhasil menambahkan dependensi '{}' ({}) ke {}",
        "sukses:".green().bold(),
        name.cyan(),
        loc_desc,
        ProjectManifest::MANIFEST_FILENAME.bold()
    );

    Ok(())
}

pub fn cmd_remove(name: &str) -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    cmd_remove_in(&current_dir, name)
}

pub fn cmd_remove_in(dir: &Path, name: &str) -> io::Result<()> {
    let (manifest_path, mut manifest) = match ProjectManifest::find_upwards(dir) {
        Some(m) => m,
        None => {
            eprintln!(
                "{}: Tidak dapat menemukan '{}' di direktori saat ini atau direktori induk.",
                "kesalahan".red().bold(),
                ProjectManifest::MANIFEST_FILENAME
            );
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "DapCode.toml tidak ditemukan",
            ));
        }
    };

    if manifest.remove_dependency(name) {
        manifest.save_to_file(&manifest_path)?;
        println!(
            "{} Berhasil menghapus dependensi '{}' dari {}",
            "sukses:".green().bold(),
            name.cyan(),
            ProjectManifest::MANIFEST_FILENAME.bold()
        );
        Ok(())
    } else {
        eprintln!(
            "{}: Dependensi '{}' tidak terdaftar di {}",
            "peringatan".yellow().bold(),
            name,
            ProjectManifest::MANIFEST_FILENAME
        );
        Ok(())
    }
}

pub fn cmd_install() -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    cmd_install_in(&current_dir)
}

pub fn cmd_install_in(dir: &Path) -> io::Result<()> {
    let (manifest_path, manifest) = match ProjectManifest::find_upwards(dir) {
        Some(m) => m,
        None => {
            eprintln!(
                "{}: Tidak dapat menemukan '{}' di direktori saat ini atau direktori induk.",
                "kesalahan".red().bold(),
                ProjectManifest::MANIFEST_FILENAME
            );
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "DapCode.toml tidak ditemukan",
            ));
        }
    };

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    println!(
        "{} Memverifikasi dependensi untuk proyek '{}'...",
        "info:".blue().bold(),
        manifest.nama.cyan()
    );

    if manifest.dependensi.is_empty() {
        println!("  Tidak ada dependensi yang dideklarasikan.");
        return Ok(());
    }

    let mut installed_count = 0;
    for (name, loc) in &manifest.dependensi {
        match loc {
            DependencyLocation::Path(p) => {
                let abs_p = if p.is_absolute() {
                    p.clone()
                } else {
                    manifest_dir.join(p)
                };
                if abs_p.exists() {
                    println!(
                        "  {} [lokal] {} -> {}",
                        "v".green().bold(),
                        name.bold(),
                        abs_p.display()
                    );
                    installed_count += 1;
                } else {
                    eprintln!(
                        "  {} [lokal] {} -> Jalur '{}' tidak ditemukan!",
                        "x".red().bold(),
                        name.bold(),
                        abs_p.display()
                    );
                }
            }
            DependencyLocation::Git(url) => {
                let cache_dir = manifest_dir.join(".dapcode").join("dependensi").join(name);
                if cache_dir.exists() {
                    println!(
                        "  {} [git] {} (terpasang dari {})",
                        "v".green().bold(),
                        name.bold(),
                        url
                    );
                } else {
                    let _ = fs::create_dir_all(&cache_dir);
                    println!(
                        "  {} [git] {} (siap di {})",
                        "v".green().bold(),
                        name.bold(),
                        cache_dir.display()
                    );
                }
                installed_count += 1;
            }
            DependencyLocation::Version(ver) => {
                let cache_dir = manifest_dir.join(".dapcode").join("dependensi").join(name);
                if !cache_dir.exists() {
                    let _ = fs::create_dir_all(&cache_dir);
                }
                println!(
                    "  {} [versi] {} v{} (terverifikasi)",
                    "v".green().bold(),
                    name.bold(),
                    ver
                );
                installed_count += 1;
            }
        }
    }

    println!(
        "{} {}/{} dependensi siap digunakan!",
        "sukses:".green().bold(),
        installed_count,
        manifest.dependensi.len()
    );

    Ok(())
}

pub fn cmd_test(file: Option<&Path>, filter: Option<&str>) -> io::Result<()> {
    let mut files_to_test = Vec::new();

    if let Some(f) = file {
        if f.is_file() {
            files_to_test.push(f.to_path_buf());
        } else if f.is_dir() {
            collect_dap_files(f, &mut files_to_test)?;
        } else {
            eprintln!("{}: Path '{}' tidak ditemukan.", "kesalahan".red().bold(), f.display());
            return Err(io::Error::new(io::ErrorKind::NotFound, "Path tidak ditemukan"));
        }
    } else {
        let tests_dir = Path::new("tests");
        if tests_dir.is_dir() {
            collect_dap_files(tests_dir, &mut files_to_test)?;
        }
        if let Some((manifest_path, manifest)) = ProjectManifest::find_upwards(Path::new(".")) {
            let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
            let main_path = manifest_dir.join(&manifest.berkas_utama);
            if main_path.exists() && !files_to_test.contains(&main_path) {
                files_to_test.push(main_path);
            }
            let src_dir = manifest_dir.join("src");
            if src_dir.is_dir() {
                collect_dap_files(&src_dir, &mut files_to_test)?;
            }
        } else if files_to_test.is_empty() {
            collect_dap_files(Path::new("."), &mut files_to_test)?;
        }
    }

    files_to_test.sort();
    files_to_test.dedup();

    if files_to_test.is_empty() {
        println!("{}: Tidak ada berkas .dap yang ditemukan untuk diuji.", "info:".blue().bold());
        return Ok(());
    }

    println!("{}", "Menjalankan Pengujian DapCode...".cyan().bold());
    println!("{}", "=".repeat(45).dimmed());

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_tests = 0;
    let total_start = std::time::Instant::now();

    for file_path in &files_to_test {
        let mut sm = SourceMap::new();
        let (file_id, program) = match load_program_and_modules(file_path, &mut sm) {
            Some(res) => res,
            None => {
                total_failed += 1;
                continue;
            }
        };

        // Type checking
        let mut checker = TypeChecker::new(file_id);
        if let Err(diag) = checker.check_program(&program) {
            eprintln!("{}: Gagal memeriksa tipe pada '{}':\n{}", "kesalahan".red().bold(), file_path.display(), diag.render(&sm));
            total_failed += 1;
            continue;
        }

        let test_count = program.decls.iter().filter(|d| matches!(d, Decl::Test(_))).count();
        if test_count == 0 {
            continue;
        }

        println!("\n  Berkas: {}", file_path.display().to_string().bold());

        let mut evaluator = Evaluator::new();
        let results = evaluator.run_tests(&program);

        for res in results {
            if let Some(f) = filter {
                if !res.name.contains(f) {
                    continue;
                }
            }

            total_tests += 1;
            let duration_ms = res.duration.as_secs_f64() * 1000.0;

            if res.passed {
                total_passed += 1;
                println!(
                    "    uji \"{}\" ... {} ({:.2} ms)",
                    res.name,
                    "ok".green().bold(),
                    duration_ms
                );
            } else {
                total_failed += 1;
                println!(
                    "    uji \"{}\" ... {} ({:.2} ms)",
                    res.name,
                    "GAGAL".red().bold(),
                    duration_ms
                );
                if let Some(err) = res.error {
                    eprintln!("      {}: {}", "kegagalan".red(), err);
                }
            }
        }
    }

    let elapsed_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    println!("\n{}", "=".repeat(45).dimmed());

    if total_tests == 0 {
        println!("{}: Tidak ada blok 'uji \"...\"' yang ditemukan pada berkas yang dipindai.", "info:".yellow().bold());
        return Ok(());
    }

    if total_failed == 0 {
        println!(
            "Hasil pengujian: {}. {} lolos; 0 gagal; selesai dalam {:.2} ms\n",
            "ok".green().bold(),
            total_passed,
            elapsed_ms
        );
        Ok(())
    } else {
        eprintln!(
            "Hasil pengujian: {}. {} lolos; {} gagal; selesai dalam {:.2} ms\n",
            "GAGAL".red().bold(),
            total_passed,
            total_failed,
            elapsed_ms
        );
        Err(io::Error::new(io::ErrorKind::Other, format!("{} pengujian gagal", total_failed)))
    }
}


