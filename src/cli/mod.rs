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
use crate::modules::{ModuleLoader, ProjectManifest};
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
