use clap::{Parser as ClapParser, Subcommand};
use colored::*;
use crate::ast::Decl;
use crate::codegen::CodeGenerator;
use crate::comptime::ComptimeEvaluator;
use crate::doc::{DocExtractor, DocGenerator, ProjectDoc};
use crate::eval::{ControlFlow, Environment, Evaluator, Value};
use crate::fmt::Formatter;
use crate::ir::IRBuilder;
use crate::lexer::Lexer;
use crate::lsp::LspServer;
use crate::modules::{DependencyLocation, Lockfile, ModuleLoader, ProjectManifest};
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
        /// Argumen baris perintah yang diteruskan ke program DapCode
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Memeriksa tipe (type check) dan analisis kepemilikan (ownership) tanpa eksekusi
    Check {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
    },
    /// Mengompilasi program DapCode ke berkas binary mandiri (native executable atau WebAssembly .wasm)
    Build {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
        /// Berkas keluaran binary (.exe atau .wasm)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Target kompilasi: "native" (default) atau "wasm"
        #[arg(short, long, default_value = "native")]
        target: String,
        /// Pintasan kompilasi ke WebAssembly (--target wasm)
        #[arg(long)]
        wasm: bool,
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
    /// Memverifikasi dan memasang semua dependensi yang tertera di DapCode.toml serta mengunci ke DapCode.lock
    Install,
    /// Memperbarui versi dan hash dependensi di DapCode.lock
    Update,
    /// Menampilkan diagram pohon dependensi proyek
    Tree,
    /// Subperintah manajer paket DapCode (dappkg)
    #[command(subcommand)]
    Pkg(PkgCommands),
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
    /// Menghasilkan dokumentasi kode sumber DapCode (.dap) ke format HTML atau Markdown
    Doc {
        /// Berkas sumber .dap atau direktori proyek (opsional)
        path: Option<PathBuf>,
        /// Direktori keluaran dokumentasi (default: "docs")
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Format keluaran: "html", "markdown", atau "semua" (default: "semua")
        #[arg(short, long, default_value = "semua")]
        format: String,
        /// Buka dokumentasi HTML di browser setelah selesai dibuat
        #[arg(long)]
        open: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum PkgCommands {
    /// Menginisialisasi paket DapCode baru (DapCode.toml & src/utama.dap)
    Init {
        /// Nama paket atau direktori target (opsional)
        name: Option<String>,
    },
    /// Menambahkan dependensi paket baru ke DapCode.toml dan DapCode.lock
    Add {
        /// Nama paket dependensi
        name: String,
        /// Jalur direktori lokal ke paket (opsional)
        #[arg(long, alias = "jalur")]
        path: Option<PathBuf>,
        /// URL Git repositori paket (opsional)
        #[arg(long)]
        git: Option<String>,
        /// Versi semver paket (opsional)
        #[arg(short, long, alias = "versi")]
        version: Option<String>,
    },
    /// Menghapus dependensi dari DapCode.toml dan DapCode.lock
    Remove {
        /// Nama paket dependensi yang akan dihapus
        name: String,
    },
    /// Memasang semua dependensi dan mengunci integritas ke DapCode.lock
    Install,
    /// Memperbarui penguncian versi dan hash SHA-256 di DapCode.lock
    Update,
    /// Menampilkan diagram pohon dependensi proyek
    Tree,
    /// Memverifikasi integritas hash SHA-256 semua dependensi terhadap DapCode.lock
    Check,
}

pub fn run_cli() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => cmd_init(name.as_deref()),
        Commands::Run { file, args } => cmd_run(file.as_deref(), args),
        Commands::Check { file } => cmd_check(file.as_deref()),
        Commands::Build {
            file,
            output,
            target,
            wasm,
        } => cmd_build(file.as_deref(), output, &target, wasm),
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
        Commands::Update => cmd_update(),
        Commands::Tree => cmd_tree(),
        Commands::Pkg(pkg_cmd) => handle_pkg_cmd(pkg_cmd),
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Test { file, filter } => cmd_test(file.as_deref(), filter.as_deref()),
        Commands::Doc {
            path,
            output,
            format,
            open,
        } => cmd_doc(path.as_deref(), output.as_deref(), &format, open),
    }
}

pub fn resolve_target_file(file: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(f) = file {
        if f.is_dir() {
            if let Some(manifest_path) = ProjectManifest::find_in_dir(f) {
                if let Ok(manifest) = ProjectManifest::from_file(&manifest_path) {
                    let entry = f.join(&manifest.berkas_utama);
                    if entry.exists() {
                        return Ok(entry);
                    }
                }
            }
            let default_entry = f.join("src").join("utama.dap");
            if default_entry.exists() {
                return Ok(default_entry);
            }
        } else if f.file_name().and_then(|n| n.to_str()) == Some(ProjectManifest::MANIFEST_FILENAME) {
            let dir = f.parent().unwrap_or_else(|| Path::new("."));
            if let Ok(manifest) = ProjectManifest::from_file(f) {
                let entry = dir.join(&manifest.berkas_utama);
                if entry.exists() {
                    return Ok(entry);
                }
            }
        }
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

pub fn cmd_init(name: Option<&str>) -> io::Result<()> {
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
    cmd_init_in(target_dir, &project_name)
}

pub fn cmd_init_in(target_dir: &Path, project_name: &str) -> io::Result<()> {
    match ProjectManifest::init_project(target_dir, project_name) {
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

fn cmd_run(file: Option<&Path>, args: Vec<String>) -> io::Result<()> {
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
    let mut program_args = vec![target.to_string_lossy().to_string()];
    program_args.extend(args);
    let mut evaluator = Evaluator::with_cli_args(program_args);
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

fn cmd_build(
    file: Option<&Path>,
    output: Option<PathBuf>,
    target_arch: &str,
    is_wasm: bool,
) -> io::Result<()> {
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

    if is_wasm || target_arch.to_lowercase() == "wasm" {
        let target_out = output.unwrap_or_else(|| target.with_extension("wasm"));
        println!(
            "{} Mengompilasi '{}' ke modul WebAssembly '{}'...",
            "info:".blue().bold(),
            target.display(),
            target_out.display()
        );

        let mut emitter = crate::wasm::WasmEmitter::new();
        match emitter.compile_program(&program) {
            Ok(wasm_bytes) => {
                if let Err(e) = crate::wasm::write_wasm_artifacts(&wasm_bytes, &target_out) {
                    eprintln!(
                        "{} Gagal menulis berkas WebAssembly: {}",
                        "kesalahan:".red().bold(),
                        e
                    );
                    return Ok(());
                }
                println!(
                    "{} Berhasil mengompilasi WebAssembly '{}' ({} byte) beserta peluncur HTML & JS!",
                    "sukses:".green().bold(),
                    target_out.display(),
                    wasm_bytes.len()
                );
            }
            Err(diag) => {
                eprint!("{}", diag.render(&sm));
            }
        }
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

pub fn calculate_delimiter_delta(line: &str) -> i32 {
    let mut delta = 0;
    let mut in_str = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && in_str {
            chars.next(); // Lewati karakter escape
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if !in_str {
            if c == '/' && chars.peek() == Some(&'/') {
                break; // Komentar baris
            }
            match c {
                '{' | '(' | '[' => delta += 1,
                '}' | ')' | ']' => delta -= 1,
                _ => {}
            }
        }
    }
    delta
}

fn cmd_repl() -> io::Result<()> {
    println!(
        "{}",
        "============================================================".cyan()
    );
    println!(
        "{} {}",
        "=== Konsol Interaktif DapCode REPL v0.1.0 ===".cyan().bold(),
        "(.dap)".green().bold()
    );
    println!(
        "Ketik '{}' untuk bantuan, '{}' untuk keluar.",
        ":bantuan".yellow().bold(),
        ":keluar".yellow().bold()
    );
    println!(
        "{}",
        "============================================================".cyan()
    );

    let mut env = Environment::new();
    let mut evaluator = Evaluator::new();
    let mut history: Vec<String> = Vec::new();
    let mut accumulated = String::new();
    let mut depth: i32 = 0;

    let stdin = io::stdin();
    let mut line_buf = String::new();

    loop {
        if depth == 0 {
            print!("{} ", "dap>".yellow().bold());
        } else {
            print!("{} ", "... ".dimmed());
        }
        io::stdout().flush()?;

        line_buf.clear();
        if stdin.read_line(&mut line_buf)? == 0 {
            // EOF (Ctrl+D / Ctrl+Z)
            println!("\n{}", "Sampai jumpa! 👋".cyan());
            break;
        }

        let trimmed_line = line_buf.trim();

        // 1. Meta Commands handling (only when not currently in a multiline block)
        if depth == 0 && accumulated.is_empty() {
            if trimmed_line.is_empty() {
                continue;
            }
            if trimmed_line == ":keluar" || trimmed_line == ":exit" || trimmed_line == "keluar" || trimmed_line == "exit" {
                println!("{}", "Sampai jumpa! 👋".cyan());
                break;
            }
            if trimmed_line == ":bantuan" || trimmed_line == ":help" {
                println!("{}", "Panduan Penggunaan DapCode REPL:".cyan().bold());
                println!("  {}             Tampilkan panduan perintah meta", ":bantuan / :help".yellow());
                println!("  {} <expr>      Periksa tipe ekspresi tanpa evaluasi", ":tipe / :type".yellow());
                println!("  {}            Daftar fungsi yang didefinisikan sesi ini", ":fungsi".yellow());
                println!("  {}           Tampilkan riwayat perintah sesi ini", ":riwayat / :history".yellow());
                println!("  {}            Bersihkan layar konsol terminal", ":bersih / :clear".yellow());
                println!("  {}             Reset semua variabel dan fungsi ke awal", ":reset".yellow());
                println!("  {}            Keluar dari REPL", ":keluar / :exit".yellow());
                println!("\nSintaks Cepat:");
                println!("  - Evaluasi langsung:  {}  ->  {}", "10 + 20 * 3".green(), "70".bold());
                println!("  - Variabel:           {}  ->  {}", "nama := \"DapCode\"".green(), "\"DapCode\"".bold());
                println!("  - Multi-baris:        Membuka kurung kurawal '{}' akan melanjutkan masukan dengan prompt '... '", "{".bold());
                continue;
            }
            if trimmed_line == ":bersih" || trimmed_line == ":clear" {
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush()?;
                continue;
            }
            if trimmed_line == ":reset" {
                env = Environment::new();
                evaluator = Evaluator::new();
                println!("{}", "Environment dan fungsi REPL berhasil di-reset ke kondisi awal.".green());
                continue;
            }
            if trimmed_line == ":riwayat" || trimmed_line == ":history" {
                if history.is_empty() {
                    println!("{}", "Riwayat masukan masih kosong.".dimmed());
                } else {
                    println!("{}", "Riwayat Masukan REPL:".cyan().bold());
                    for (idx, cmd) in history.iter().enumerate() {
                        let preview = if cmd.contains('\n') {
                            format!("{} ...", cmd.lines().next().unwrap_or(""))
                        } else {
                            cmd.clone()
                        };
                        println!("  {:3}. {}", (idx + 1).to_string().dimmed(), preview);
                    }
                }
                continue;
            }
            if trimmed_line == ":fungsi" {
                let user_fns = evaluator.get_defined_functions();
                if user_fns.is_empty() {
                    println!("{}", "Belum ada fungsi yang didefinisikan oleh pengguna.".dimmed());
                } else {
                    println!("{}", "Daftar Fungsi Terdaftar:".cyan().bold());
                    for f in user_fns {
                        println!("  - {}", f.bold());
                    }
                }
                continue;
            }
            if trimmed_line.starts_with(":tipe ") || trimmed_line.starts_with(":type ") {
                let expr_str = if trimmed_line.starts_with(":tipe ") {
                    &trimmed_line[6..]
                } else {
                    &trimmed_line[6..]
                }
                .trim();
                let mut sm = SourceMap::new();
                let (file_id, _) = sm.add_file("<repl_tipe>", expr_str.to_string());
                let mut lexer = Lexer::new(expr_str, file_id);
                match lexer.tokenize() {
                    Ok(tokens) => {
                        let mut parser = Parser::new(tokens, file_id);
                        match parser.parse_expr(Precedence::Lowest) {
                            Ok(expr) => {
                                let mut checker = TypeChecker::new(file_id);
                                match checker.check_expr(&expr) {
                                    Ok(ty) => {
                                        let resolved = checker.resolve_type(&ty);
                                        println!("{} {} : {}", "tipe:".cyan().bold(), expr_str, resolved.to_string().green().bold());
                                    }
                                    Err(diag) => eprintln!("{}", diag.render(&sm)),
                                }
                            }
                            Err(diag) => eprintln!("{}", diag.render(&sm)),
                        }
                    }
                    Err(diag) => eprintln!("{}", diag.render(&sm)),
                }
                continue;
            }
            if trimmed_line.starts_with(':') {
                eprintln!("{}: Perintah meta '{}' tidak dikenal. Ketik '{}' untuk panduan.", "peringatan".yellow().bold(), trimmed_line, ":bantuan".cyan());
                continue;
            }
        }

        // 2. Accumulate lines and track block depth
        let delta = calculate_delimiter_delta(trimmed_line);
        depth += delta;
        if depth < 0 {
            depth = 0;
        }

        if !accumulated.is_empty() {
            accumulated.push('\n');
        }
        accumulated.push_str(line_buf.trim_end());

        // If we still have unclosed braces, continue reading next lines
        if depth > 0 {
            continue;
        }

        let input_to_run = accumulated.trim().to_string();
        accumulated.clear();
        depth = 0;

        if input_to_run.is_empty() {
            continue;
        }

        history.push(input_to_run.clone());

        // 3. Try parsing input
        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file("<repl>", input_to_run.clone());
        let mut lexer = Lexer::new(&input_to_run, file_id);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(diag) => {
                eprint!("{}", diag.render(&sm));
                continue;
            }
        };

        // Case A: Top-level declarations (fungsi, struktur, terapkan)
        let mut parser_prog = Parser::new(tokens.clone(), file_id);
        if let Ok(prog) = parser_prog.parse_program() {
            if !prog.decls.is_empty() {
                let mut registered_names = Vec::new();
                for decl in &prog.decls {
                    match decl {
                        Decl::Fn(f) => {
                            evaluator.register_decl(decl);
                            registered_names.push(format!("fungsi '{}'", f.name.bold()));
                        }
                        Decl::Struct(s) => {
                            evaluator.register_decl(decl);
                            registered_names.push(format!("struktur '{}'", s.name.bold()));
                        }
                        Decl::Impl(i) => {
                            evaluator.register_decl(decl);
                            registered_names.push(format!("metode untuk '{}'", i.target_struct.bold()));
                        }
                        _ => {
                            evaluator.register_decl(decl);
                        }
                    }
                }
                if !registered_names.is_empty() {
                    println!("{} Berhasil mendefinisikan {}.", "=>".green().bold(), registered_names.join(", "));
                    continue;
                }
            }
        }

        // Case B: Single expression (10 + 20, ke_huruf_besar("abc"), dll.)
        let mut parser_expr = Parser::new(tokens.clone(), file_id);
        if let Ok(expr) = parser_expr.parse_expr(Precedence::Lowest) {
            match evaluator.eval_expr(&expr, &mut env) {
                Ok(v) => {
                    println!("{} {}", "=>".green().bold(), v);
                    continue;
                }
                Err(_) => {
                    // Fallthrough to statement block wrap
                }
            }
        }

        // Case C: Sequence of statements / variable assignment (x := 42; y := x + 1)
        let wrapped = format!("fungsi __repl_block() {{\n{}\n}}", input_to_run);
        let mut sm_wrap = SourceMap::new();
        let (w_id, _) = sm_wrap.add_file("<repl_wrap>", wrapped.clone());
        let mut w_lex = Lexer::new(&wrapped, w_id);
        if let Ok(w_tokens) = w_lex.tokenize() {
            let mut w_parser = Parser::new(w_tokens, w_id);
            if let Ok(prog) = w_parser.parse_program() {
                if let Some(Decl::Fn(f)) = prog.decls.into_iter().next() {
                    match evaluator.eval_block(&f.body, &mut env) {
                        Ok(ControlFlow::Next(v) | ControlFlow::Return(v)) => {
                            if !matches!(v, Value::Unit) {
                                println!("{} {}", "=>".green().bold(), v);
                            }
                        }
                        Ok(ControlFlow::Fail(e)) => {
                            eprintln!("{}: {}", "Gagal".red().bold(), e);
                        }
                        Err(diag) => eprintln!("{}", diag.render(&sm_wrap)),
                    }
                    continue;
                }
            }
        }

        // If all parsing attempts failed, display parser diagnostic from direct parse
        let mut parser_diag = Parser::new(tokens, file_id);
        if let Err(diag) = parser_diag.parse_program() {
            eprint!("{}", diag.render(&sm));
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

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    if let Ok(lock) = Lockfile::generate_or_update(&manifest, manifest_dir) {
        let _ = lock.save_to_file(&lockfile_path);
    }

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

        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
        if lockfile_path.exists() {
            if let Ok(mut lock) = Lockfile::from_file(&lockfile_path) {
                lock.paket.remove(name);
                let _ = lock.save_to_file(&lockfile_path);
            }
        }

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

    // Kunci dependensi ke DapCode.lock dan verifikasi integritas hash
    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    if lockfile_path.exists() {
        if let Ok(lock) = Lockfile::from_file(&lockfile_path) {
            match lock.verify_integrity(&manifest, manifest_dir) {
                Ok(reports) => {
                    println!(
                        "  {} [lockfile] Integritas {} paket terverifikasi cocok dengan {}",
                        "v".green().bold(),
                        reports.len(),
                        Lockfile::LOCKFILE_FILENAME.bold()
                    );
                }
                Err(diag) => {
                    eprintln!("  {} [lockfile] {}", "!".yellow().bold(), diag);
                    println!(
                        "  {} Memperbarui {} sesuai hash berkas terbaru...",
                        "info:".blue().bold(),
                        Lockfile::LOCKFILE_FILENAME.bold()
                    );
                    if let Ok(new_lock) = Lockfile::generate_or_update(&manifest, manifest_dir) {
                        let _ = new_lock.save_to_file(&lockfile_path);
                    }
                }
            }
        } else if let Ok(new_lock) = Lockfile::generate_or_update(&manifest, manifest_dir) {
            let _ = new_lock.save_to_file(&lockfile_path);
        }
    } else if let Ok(new_lock) = Lockfile::generate_or_update(&manifest, manifest_dir) {
        let _ = new_lock.save_to_file(&lockfile_path);
        println!(
            "  {} [lockfile] Berhasil mengunci dependensi ke {}",
            "v".green().bold(),
            Lockfile::LOCKFILE_FILENAME.bold()
        );
    }

    println!(
        "{} {}/{} dependensi siap digunakan!",
        "sukses:".green().bold(),
        installed_count,
        manifest.dependensi.len()
    );

    Ok(())
}

pub fn cmd_update() -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    cmd_update_in(&current_dir)
}

pub fn cmd_update_in(dir: &Path) -> io::Result<()> {
    let (manifest_path, manifest) = match ProjectManifest::find_upwards(dir) {
        Some(m) => m,
        None => {
            eprintln!(
                "{}: Tidak dapat menemukan '{}' di direktori saat ini atau induk.",
                "kesalahan".red().bold(),
                ProjectManifest::MANIFEST_FILENAME
            );
            return Err(io::Error::new(io::ErrorKind::NotFound, "DapCode.toml tidak ditemukan"));
        }
    };

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    println!(
        "{} Memperbarui dependensi & menghitung ulang hash SHA-256 untuk '{}'...",
        "info:".blue().bold(),
        manifest.nama.cyan()
    );

    let lockfile = Lockfile::generate_or_update(&manifest, manifest_dir).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;

    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    lockfile.save_to_file(&lockfile_path)?;

    println!(
        "{} Berhasil memperbarui {} ({} paket terkunci)",
        "sukses:".green().bold(),
        Lockfile::LOCKFILE_FILENAME.bold(),
        lockfile.paket.len()
    );

    Ok(())
}

pub fn cmd_pkg_check() -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    cmd_pkg_check_in(&current_dir)
}

pub fn cmd_pkg_check_in(dir: &Path) -> io::Result<()> {
    let (manifest_path, manifest) = match ProjectManifest::find_upwards(dir) {
        Some(m) => m,
        None => {
            eprintln!(
                "{}: Tidak dapat menemukan '{}' di direktori saat ini atau induk.",
                "kesalahan".red().bold(),
                ProjectManifest::MANIFEST_FILENAME
            );
            return Err(io::Error::new(io::ErrorKind::NotFound, "DapCode.toml tidak ditemukan"));
        }
    };

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    if !lockfile_path.exists() {
        eprintln!(
            "{}: Berkas '{}' belum ada. Jalankan '{}' untuk mengunci dependensi.",
            "kesalahan".red().bold(),
            Lockfile::LOCKFILE_FILENAME,
            "dappkg install".cyan()
        );
        return Err(io::Error::new(io::ErrorKind::NotFound, "DapCode.lock tidak ditemukan"));
    }

    let lockfile = Lockfile::from_file(&lockfile_path).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, e.to_string())
    })?;

    match lockfile.verify_integrity(&manifest, manifest_dir) {
        Ok(reports) => {
            println!(
                "{} Integritas dependensi terverifikasi ({} paket valid):",
                "sukses:".green().bold(),
                reports.len()
            );
            for rep in reports {
                let short_hash = if rep.hash_terkunci.len() >= 16 {
                    format!("{}...", &rep.hash_terkunci[..16])
                } else {
                    rep.hash_terkunci.clone()
                };
                println!(
                    "  {} {} (sha256: {})",
                    "v".green().bold(),
                    rep.nama.bold(),
                    short_hash
                );
            }
            Ok(())
        }
        Err(diag) => {
            eprintln!("{}: {}", "kesalahan integritas".red().bold(), diag);
            Err(io::Error::new(io::ErrorKind::InvalidData, "Integritas hash dependensi tidak cocok"))
        }
    }
}

pub fn cmd_tree() -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    cmd_tree_in(&current_dir)
}

pub fn cmd_tree_in(dir: &Path) -> io::Result<()> {
    let (manifest_path, manifest) = match ProjectManifest::find_upwards(dir) {
        Some(m) => m,
        None => {
            eprintln!(
                "{}: Tidak dapat menemukan '{}' di direktori saat ini atau induk.",
                "kesalahan".red().bold(),
                ProjectManifest::MANIFEST_FILENAME
            );
            return Err(io::Error::new(io::ErrorKind::NotFound, "DapCode.toml tidak ditemukan"));
        }
    };

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    let lockfile = if lockfile_path.exists() {
        Lockfile::from_file(&lockfile_path).ok()
    } else {
        None
    };

    println!(
        "{} v{} ({})",
        manifest.nama.cyan().bold(),
        manifest.versi,
        manifest_dir.display()
    );

    if manifest.dependensi.is_empty() {
        println!("  └── (tidak ada dependensi yang dideklarasikan)");
        return Ok(());
    }

    let count = manifest.dependensi.len();
    for (i, (name, loc)) in manifest.dependensi.iter().enumerate() {
        let is_last = i + 1 == count;
        let prefix = if is_last { "└── " } else { "├── " };
        let lock_info = lockfile.as_ref().and_then(|lf| lf.paket.get(name));
        let ver = lock_info.map(|p| p.versi.as_str()).unwrap_or("?");
        let hash_short = lock_info.map(|p| {
            if p.hash_sha256.len() >= 12 {
                format!("{}...", &p.hash_sha256[..12])
            } else {
                p.hash_sha256.clone()
            }
        }).unwrap_or_else(|| "tidak terkunci".to_string());

        let (source_desc, target_desc) = match loc {
            DependencyLocation::Path(p) => ("lokal", p.display().to_string()),
            DependencyLocation::Git(url) => ("git", url.clone()),
            DependencyLocation::Version(v) => ("versi", format!("v{}", v)),
        };

        println!(
            "  {}{} v{} [{}] -> {} (sha256: {})",
            prefix,
            name.bold(),
            ver,
            source_desc.yellow(),
            target_desc.cyan(),
            hash_short
        );
    }

    Ok(())
}

pub fn handle_pkg_cmd(cmd: PkgCommands) -> io::Result<()> {
    match cmd {
        PkgCommands::Init { name } => cmd_init(name.as_deref()),
        PkgCommands::Add {
            name,
            path,
            git,
            version,
        } => cmd_add(&name, path, git, version),
        PkgCommands::Remove { name } => cmd_remove(&name),
        PkgCommands::Install => cmd_install(),
        PkgCommands::Update => cmd_update(),
        PkgCommands::Tree => cmd_tree(),
        PkgCommands::Check => cmd_pkg_check(),
    }
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

pub fn cmd_doc(
    path_opt: Option<&Path>,
    out_opt: Option<&Path>,
    format: &str,
    open: bool,
) -> io::Result<()> {
    let mut files_to_doc = Vec::new();
    let mut project_name = "Proyek DapCode".to_string();
    let mut project_version = "0.1.0".to_string();
    let mut project_desc = "Dokumentasi kode sumber DapCode".to_string();

    if let Some(target) = path_opt {
        if target.is_file() {
            files_to_doc.push(target.to_path_buf());
            project_name = target
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(project_name);
        } else if target.is_dir() {
            collect_dap_files(target, &mut files_to_doc)?;
            if let Some((_, manifest)) = ProjectManifest::find_upwards(target) {
                let author = if manifest.penulis.is_empty() { "DapCode Core".to_string() } else { manifest.penulis.join(", ") };
                project_name = manifest.nama.clone();
                project_version = manifest.versi.clone();
                project_desc = format!("Paket {} v{} oleh {}", manifest.nama, manifest.versi, author);
            } else {
                project_name = target
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(project_name);
            }
        }
    } else {
        if let Some((manifest_path, manifest)) = ProjectManifest::find_upwards(Path::new(".")) {
            let author = if manifest.penulis.is_empty() { "DapCode Core".to_string() } else { manifest.penulis.join(", ") };
            project_name = manifest.nama.clone();
            project_version = manifest.versi.clone();
            project_desc = format!("Paket {} v{} oleh {}", manifest.nama, manifest.versi, author);

            let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
            let src_dir = manifest_dir.join("src");
            if src_dir.is_dir() {
                collect_dap_files(&src_dir, &mut files_to_doc)?;
            }
            let main_path = manifest_dir.join(&manifest.berkas_utama);
            if main_path.exists() && !files_to_doc.contains(&main_path) {
                files_to_doc.push(main_path);
            }
        } else {
            collect_dap_files(Path::new("."), &mut files_to_doc)?;
        }
    }

    files_to_doc.sort();
    files_to_doc.dedup();

    if files_to_doc.is_empty() {
        println!("{}: Tidak ada berkas .dap yang ditemukan untuk didokumentasikan.", "info:".yellow().bold());
        return Ok(());
    }

    println!(
        "{} Menghasilkan dokumentasi untuk '{}' ({} berkas)...",
        "info:".blue().bold(),
        project_name.cyan(),
        files_to_doc.len()
    );

    let mut modules = Vec::new();
    for file_path in &files_to_doc {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: Gagal membaca '{}': {}", "peringatan".yellow().bold(), file_path.display(), e);
                continue;
            }
        };

        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file(file_path, content.clone());
        let mut lexer = Lexer::new(&content, file_id);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(diag) => {
                eprintln!("{}: {}", file_path.display(), diag.render(&sm));
                continue;
            }
        };

        let mut parser = Parser::new(tokens, file_id);
        let program = match parser.parse_program() {
            Ok(p) => p,
            Err(diag) => {
                eprintln!("{}: {}", file_path.display(), diag.render(&sm));
                continue;
            }
        };

        let mod_doc = DocExtractor::extract_module(&content, file_path, &program, &sm);
        modules.push(mod_doc);
    }

    let project_doc = ProjectDoc {
        name: project_name.clone(),
        version: project_version,
        description: project_desc,
        modules,
    };

    let out_dir = out_opt.unwrap_or_else(|| Path::new("docs"));
    fs::create_dir_all(out_dir)?;

    let generator = DocGenerator::new(&project_doc);
    let mut generated_files = Vec::new();

    let fmt_lower = format.to_lowercase();
    if fmt_lower == "markdown" || fmt_lower == "semua" || fmt_lower == "all" {
        let md = generator.generate_markdown();
        let md_path = out_dir.join("README.md");
        fs::write(&md_path, md)?;
        generated_files.push(md_path);
    }

    if fmt_lower == "html" || fmt_lower == "semua" || fmt_lower == "all" {
        let html = generator.generate_html();
        let html_path = out_dir.join("index.html");
        fs::write(&html_path, html)?;
        generated_files.push(html_path);
    }

    println!(
        "{} Dokumentasi berhasil dibuat di '{}':",
        "sukses:".green().bold(),
        out_dir.display()
    );
    for f in &generated_files {
        println!("  - {}", f.display().to_string().bold());
    }

    if open {
        if let Some(html_file) = generated_files.iter().find(|p| p.extension().and_then(|e| e.to_str()) == Some("html")) {
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", "", &html_file.to_string_lossy()])
                    .spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open").arg(html_file).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("xdg-open").arg(html_file).spawn();
            }
        }
    }

    Ok(())
}


