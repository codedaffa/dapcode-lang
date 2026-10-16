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
use crate::modules::{
    copy_dir_all, fetch_git_dependency, DependencyLocation, Lockfile, ModuleLoader, ProjectManifest,
};
use crate::ownership::OwnershipAnalyzer;
use crate::parser::{Parser, Precedence};
use crate::span::SourceMap;
use crate::types::TypeChecker;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(ClapParser)]
#[command(
    name = "dapcodec",
    author = "DapCode Core Team",
    version = "0.1.0",
    about = "Kompilator dan Toolchain Resmi untuk Bahasa Pemrograman DapCode (.dap)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
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
    /// Mengompilasi program DapCode ke berkas binary mandiri (native, wasm, atau bundel mobile PWA)
    Build {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
        /// Berkas keluaran binary (.exe atau .wasm) atau folder bundel mobile
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Target kompilasi: "native" (default), "wasm", atau "mobile" / "pwa"
        #[arg(short, long, default_value = "native")]
        target: String,
        /// Pintasan kompilasi ke WebAssembly (--target wasm)
        #[arg(long)]
        wasm: bool,
        /// Bangun sebagai bundel aplikasi mobile lengkap (PWA + Android WebView Template)
        #[arg(long)]
        mobile: bool,
        /// Pintasan untuk membangun PWA (Progressive Web App)
        #[arg(long)]
        pwa: bool,
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
    /// Menjalankan Debug Adapter Protocol (DAP) server untuk debugging interaktif
    Dap,
    /// Memulai debugger interaktif CLI (Interactive Debugger)
    #[command(alias = "awakutu")]
    Debug {
        /// Berkas sumber .dap yang akan di-debug (opsional, default: berkas utama proyek)
        file: Option<PathBuf>,
        /// Titik henti (baris) awal (contoh: -b 10 -b 25)
        #[arg(short = 'b', long = "titik-henti", alias = "breakpoint")]
        breakpoints: Vec<usize>,
        /// Berhenti pada baris pertama eksekusi (stop-on-entry)
        #[arg(long = "henti-awal", alias = "stop-on-entry")]
        stop_on_entry: bool,
    },
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
    /// Menyimpan semua dependensi secara lokal ke folder 'vendor/' (offline vendoring)
    Vendor {
        /// Direktori target vendor (default: "vendor")
        #[arg(short, long, default_value = "vendor")]
        target: PathBuf,
    },
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
    /// Menjalankan server Web Playground DapCode interaktif dengan live-compile WebAssembly & Canvas 2D
    Playground {
        /// Port HTTP server (default: 8080)
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Buka langsung di peramban web
        #[arg(long, default_value_t = false)]
        open: bool,
    },
    /// Membangun, mengemas, atau menyajikan aplikasi mobile DapCode (PWA & Android WebView Bundler)
    #[command(subcommand)]
    Mobile(MobileCommands),
}

#[derive(Subcommand, Debug, Clone)]
pub enum MobileCommands {
    /// Menginisialisasi proyek aplikasi mobile DapCode baru dengan dukungan layar sentuh (touch screen)
    Init {
        /// Nama aplikasi atau direktori target (opsional, default: "app_mobile")
        name: Option<String>,
    },
    /// Membangun bundel aplikasi mobile lengkap (PWA, WebAssembly, dan template Android WebView)
    Build {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
        /// Direktori keluaran bundel aplikasi mobile (default: "`<nama>`_mobile")
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Nama tampilan aplikasi mobile (default: dari nama berkas)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Menjalankan server pengembangan mobile lokal dengan QR Code & akses Wi-Fi untuk pengujian instan di HP
    Serve {
        /// Berkas sumber .dap (opsional jika DapCode.toml ada di direktori saat ini)
        file: Option<PathBuf>,
        /// Port HTTP server (default: 8080)
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Buka langsung di peramban web desktop
        #[arg(long, default_value_t = false)]
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
    /// Menyimpan semua dependensi secara lokal ke direktori 'vendor/' untuk kompilasi offline
    Vendor {
        /// Direktori target vendor (default: "vendor")
        #[arg(short, long, default_value = "vendor")]
        target: PathBuf,
    },
}

/// Menginisialisasi konsol Windows untuk mendukung Unicode UTF-8 (CP 65001)
/// dan pemrosesan kode warna ANSI/VT100 (Virtual Terminal Processing)
pub fn init_windows_console() {
    #[cfg(windows)]
    {
        unsafe {
            unsafe extern "system" {
                fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
                fn SetConsoleCP(wCodePageID: u32) -> i32;
                fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
                fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
                fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
            }

            const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
            const STD_ERROR_HANDLE: u32 = -12i32 as u32;
            const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

            // Aktifkan UTF-8 code page (65001) untuk cmd.exe
            SetConsoleOutputCP(65001);
            SetConsoleCP(65001);

            // Aktifkan mode ANSI escape sequence / Virtual Terminal Processing
            for handle_id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
                let handle = GetStdHandle(handle_id);
                if !handle.is_null() && handle != ((-1i32) as isize as *mut std::ffi::c_void) {
                    let mut mode: u32 = 0;
                    if GetConsoleMode(handle, &mut mode) != 0 {
                        let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                    }
                }
            }
        }
        let _ = colored::control::set_virtual_terminal(true);
    }
}

pub fn run_cli() -> io::Result<()> {
    init_windows_console();
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        print_welcome_dashboard();
        return Ok(());
    };

    match command {
        Commands::Init { name } => cmd_init(name.as_deref()),
        Commands::Run { file, args } => cmd_run(file.as_deref(), args),
        Commands::Check { file } => cmd_check(file.as_deref()),
        Commands::Build {
            file,
            output,
            target,
            wasm,
            mobile,
            pwa,
        } => cmd_build(file.as_deref(), output, &target, wasm, mobile || pwa),
        Commands::Mobile(mob_cmd) => handle_mobile_cmd(mob_cmd),
        Commands::DumpAst { file } => cmd_dump_ast(file.as_deref()),
        Commands::DumpIr { file } => cmd_dump_ir(file.as_deref()),
        Commands::DumpOwnership { file } => cmd_dump_ownership(file.as_deref()),
        Commands::Fmt { path, check } => cmd_fmt(path.as_deref(), check),
        Commands::Lsp => cmd_lsp(),
        Commands::Dap => cmd_dap(),
        Commands::Debug {
            file,
            breakpoints,
            stop_on_entry,
        } => cmd_debug(file.as_deref(), breakpoints, stop_on_entry),
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
        Commands::Vendor { target } => cmd_vendor(Some(target)),
        Commands::Pkg(pkg_cmd) => handle_pkg_cmd(pkg_cmd),
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Test { file, filter } => cmd_test(file.as_deref(), filter.as_deref()),
        Commands::Doc {
            path,
            output,
            format,
            open,
        } => cmd_doc(path.as_deref(), output.as_deref(), &format, open),
        Commands::Playground { port, open } => cmd_playground(port, open),
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

pub fn load_program_and_modules(file: &Path, sm: &mut SourceMap) -> Option<(usize, crate::ast::Program)> {
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
    is_mobile: bool,
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

    if is_mobile || target_arch.to_lowercase() == "mobile" || target_arch.to_lowercase() == "pwa" {
        let app_stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
        let target_dir = output.unwrap_or_else(|| {
            target.parent().unwrap_or(Path::new(".")).join(format!("{}_mobile", app_stem))
        });
        println!(
            "{} Mengompilasi '{}' ke bundel aplikasi Mobile (PWA & Android WebView)...",
            "info:".blue().bold(),
            target.display()
        );

        let mut emitter = crate::wasm::WasmEmitter::new();
        match emitter.compile_program(&program) {
            Ok(wasm_bytes) => {
                if let Err(e) = crate::mobile::build_mobile_bundle(&wasm_bytes, app_stem, &target_dir) {
                    eprintln!("{} Gagal membangun bundel mobile: {}", "kesalahan:".red().bold(), e);
                    return Ok(());
                }
            }
            Err(diag) => {
                eprint!("{}", diag.render(&sm));
            }
        }
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
    let mut type_checker = TypeChecker::new(0);
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
                println!("  {} <berkas>    Muat fungsi dan struktur dari berkas .dap", ":muat / :load".yellow());
                println!("  {} <expr>      Periksa tipe ekspresi tanpa evaluasi", ":tipe / :type".yellow());
                println!("  {}            Daftar fungsi yang didefinisikan sesi ini", ":fungsi".yellow());
                println!("  {}           Tampilkan riwayat perintah sesi ini", ":riwayat / :history".yellow());
                println!("  {}            Bersihkan layar konsol terminal", ":bersih / :clear".yellow());
                println!("  {}             Reset semua variabel dan fungsi ke awal", ":reset".yellow());
                println!("  {}            Keluar dari REPL", ":keluar / :exit".yellow());
                println!("\nSintaks Cepat:");
                println!("  - Muat berkas:        {}  ->  memuat semua fungsi berkas", ":muat examples/tes.dap".green());
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
                type_checker = TypeChecker::new(0);
                println!("{}", "Environment dan fungsi REPL berhasil di-reset ke kondisi awal.".green());
                continue;
            }
            if trimmed_line.starts_with(":muat") || trimmed_line.starts_with(":load") {
                let rest = if trimmed_line.starts_with(":muat") {
                    trimmed_line[5..].trim()
                } else {
                    trimmed_line[5..].trim()
                };
                let raw_path = rest.trim_matches('"').trim_matches('\'').trim();
                if raw_path.is_empty() {
                    println!("{}", "Penggunaan: :muat <jalur_berkas.dap> atau :load <jalur_berkas.dap>".yellow());
                    println!("Contoh: :muat examples/tes.dap");
                    continue;
                }

                let mut path_buf = PathBuf::from(raw_path);
                if !path_buf.exists() && !raw_path.ends_with(".dap") {
                    let with_ext = format!("{}.dap", raw_path);
                    if Path::new(&with_ext).exists() {
                        path_buf = PathBuf::from(with_ext);
                    }
                }

                if !path_buf.exists() {
                    eprintln!("{}: Berkas '{}' tidak ditemukan.", "kesalahan".red().bold(), raw_path);
                    continue;
                }

                let mut sm = SourceMap::new();
                match load_program_and_modules(&path_buf, &mut sm) {
                    Some((_, prog)) => {
                        let _ = type_checker.check_program(&prog);
                        let mut fn_names = Vec::new();
                        let mut struct_names = Vec::new();

                        for decl in &prog.decls {
                            evaluator.register_decl(decl);
                            match decl {
                                Decl::Fn(f) => {
                                    fn_names.push(f.name.clone());
                                }
                                Decl::Struct(s) => {
                                    struct_names.push(s.name.clone());
                                }
                                _ => {}
                            }
                        }

                        println!(
                            "{} Berhasil memuat berkas '{}'",
                            "sukses:".green().bold(),
                            path_buf.display().to_string().cyan()
                        );
                        if !fn_names.is_empty() {
                            println!("  - Fungsi terdaftar ({}): {}", fn_names.len(), fn_names.join(", ").bold());
                        }
                        if !struct_names.is_empty() {
                            println!("  - Struktur terdaftar ({}): {}", struct_names.len(), struct_names.join(", ").bold());
                        }
                        println!("{}", "💡 Fungsi di atas kini dapat langsung dipanggil di REPL!".dimmed());
                    }
                    None => {}
                }
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
                                let mut checker = type_checker.clone();
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
                let _ = type_checker.check_program(&prog);
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

fn cmd_dap() -> io::Result<()> {
    let mut server = crate::debug::DebugServer::new();
    server.start()
}

pub fn cmd_debug(
    file: Option<&Path>,
    breakpoints: Vec<usize>,
    stop_on_entry: bool,
) -> io::Result<()> {
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

    // 4. Setup Interactive CLI Debugger
    let target_str = target.to_string_lossy().to_string();
    let sm_arc = Arc::new(Mutex::new(sm.clone()));
    let effective_stop_on_entry = stop_on_entry || breakpoints.is_empty();
    let dbg = crate::debug::CliDebugger::new_stdio(
        Arc::clone(&sm_arc),
        target_str.clone(),
        breakpoints,
        effective_stop_on_entry,
    );
    let hook = Arc::new(Mutex::new(dbg));

    println!(
        "{}",
        "============================================================".cyan()
    );
    println!(
        "{} {}",
        "=== Debugger Interaktif DapCode (dapcodec debug) ===".cyan().bold(),
        "(.dap)".green().bold()
    );
    println!(
        "Berkas target: {}",
        target.display().to_string().yellow().bold()
    );
    println!(
        "Ketik '{}' atau '{}' untuk bantuan perintah, '{}' untuk keluar.",
        "bantuan".green().bold(),
        "h".green().bold(),
        "keluar".yellow().bold()
    );
    println!(
        "{}",
        "============================================================".cyan()
    );

    let program_args = vec![target_str];
    let mut evaluator = Evaluator::with_cli_args(program_args);
    evaluator.set_debug_hook(hook);

    if let Err(diag) = evaluator.run_program(&program) {
        if diag.message != "Eksekusi dihentikan oleh debugger" {
            eprint!("{}", diag.render(&sm));
        }
    }

    println!("\n{}", "Sesi debugger selesai.".green().bold());
    Ok(())
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

    let (real_name, loc) = if let Some(p) = path {
        (name.to_string(), DependencyLocation::Path(p))
    } else if let Some(g) = git {
        (name.to_string(), DependencyLocation::Git(g))
    } else if let Some(v) = version {
        (name.to_string(), DependencyLocation::Version(v))
    } else if name.starts_with("https://")
        || name.starts_with("http://")
        || name.starts_with("git@")
        || name.starts_with("git://")
        || name.ends_with(".git")
    {
        let clean = name.trim_end_matches(['/', '\\']);
        let clean = clean.strip_suffix(".git").unwrap_or(clean);
        let derived = clean
            .rsplit(['/', '\\', ':'])
            .next()
            .unwrap_or(name)
            .to_string();
        (derived, DependencyLocation::Git(name.to_string()))
    } else {
        let p = PathBuf::from(name);
        if p.exists() && p.is_dir() {
            let pkg_name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(name)
                .to_string();
            (pkg_name, DependencyLocation::Path(p))
        } else {
            (name.to_string(), DependencyLocation::Version("0.1.0".to_string()))
        }
    };

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    // Jika dependensi Git, otomatis unduh/kloning ke cache lokal
    if let DependencyLocation::Git(ref g_url) = loc {
        let cache_dir = manifest_dir.join(".dapcode").join("dependensi").join(&real_name);
        println!(
            "{} Mengunduh repositori Git '{}'...",
            "info:".blue().bold(),
            g_url.cyan()
        );
        match fetch_git_dependency(g_url, &cache_dir) {
            Ok(_) => {
                println!(
                    "  {} Berhasil mengkloning ke '{}'",
                    "v".green().bold(),
                    cache_dir.display()
                );
            }
            Err(e) => {
                println!(
                    "  {} Peringatan pengunduhan git: {}",
                    "!".yellow().bold(),
                    e
                );
            }
        }
    }

    let loc_desc = match &loc {
        DependencyLocation::Path(p) => format!("jalur = \"{}\"", p.display()),
        DependencyLocation::Git(g) => format!("git = \"{}\"", g),
        DependencyLocation::Version(v) => format!("versi = \"{}\"", v),
    };

    manifest.add_dependency(&real_name, loc);
    manifest.save_to_file(&manifest_path)?;

    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    if let Ok(lock) = Lockfile::generate_or_update(&manifest, manifest_dir) {
        let _ = lock.save_to_file(&lockfile_path);
    }

    println!(
        "{} Berhasil menambahkan dependensi '{}' ({}) ke {}",
        "sukses:".green().bold(),
        real_name.cyan(),
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
        // Cek jika sudah di-vendor di vendor/<name>
        let vendor_dir = manifest_dir.join("vendor").join(name);
        if vendor_dir.exists() && vendor_dir.is_dir() {
            println!(
                "  {} [vendor] {} -> {}",
                "v".green().bold(),
                name.bold(),
                vendor_dir.display()
            );
            installed_count += 1;
            continue;
        }

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
                if cache_dir.exists() && cache_dir.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
                    let mut ver = "git-latest".to_string();
                    let dep_manifest = cache_dir.join(ProjectManifest::MANIFEST_FILENAME);
                    if let Ok(m) = ProjectManifest::from_file(&dep_manifest) {
                        ver = format!("v{}", m.versi);
                    }
                    println!(
                        "  {} [git] {} {} (terpasang di {})",
                        "v".green().bold(),
                        name.bold(),
                        ver.cyan(),
                        cache_dir.display()
                    );
                    installed_count += 1;
                } else {
                    match fetch_git_dependency(url, &cache_dir) {
                        Ok(_) => {
                            let mut ver = "git-latest".to_string();
                            let dep_manifest = cache_dir.join(ProjectManifest::MANIFEST_FILENAME);
                            if let Ok(m) = ProjectManifest::from_file(&dep_manifest) {
                                ver = format!("v{}", m.versi);
                            }
                            println!(
                                "  {} [git] {} {} (berhasil dikloning dari {})",
                                "v".green().bold(),
                                name.bold(),
                                ver.cyan(),
                                url
                            );
                            installed_count += 1;
                        }
                        Err(e) => {
                            println!(
                                "  {} [git] {} (peringatan unduhan: {})",
                                "!".yellow().bold(),
                                name.bold(),
                                e
                            );
                            installed_count += 1;
                        }
                    }
                }
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

    for (name, loc) in &manifest.dependensi {
        if let DependencyLocation::Git(url) = loc {
            let cache_dir = manifest_dir.join(".dapcode").join("dependensi").join(name);
            match fetch_git_dependency(url, &cache_dir) {
                Ok(_) => {
                    println!(
                        "  {} [git] {} berhasil diperbarui dari {}",
                        "v".green().bold(),
                        name.bold(),
                        url
                    );
                }
                Err(e) => {
                    println!(
                        "  {} [git] {} (peringatan update: {})",
                        "!".yellow().bold(),
                        name.bold(),
                        e
                    );
                }
            }
        }
    }

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

pub fn cmd_vendor(target: Option<PathBuf>) -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    let t = target.unwrap_or_else(|| PathBuf::from("vendor"));
    cmd_vendor_in(&current_dir, &t)
}

pub fn cmd_vendor_in(dir: &Path, target: &Path) -> io::Result<()> {
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
    let vendor_dir = if target.is_absolute() {
        target.to_path_buf()
    } else {
        manifest_dir.join(target)
    };

    fs::create_dir_all(&vendor_dir)?;
    println!(
        "{} Melakukan vendor dependensi ke '{}'...",
        "info:".blue().bold(),
        vendor_dir.display()
    );

    let mut count = 0;
    for (name, loc) in &manifest.dependensi {
        let dest_pkg_dir = vendor_dir.join(name);
        match loc {
            DependencyLocation::Path(rel_p) => {
                let abs_p = if rel_p.is_absolute() {
                    rel_p.clone()
                } else {
                    manifest_dir.join(rel_p)
                };
                if abs_p.exists() {
                    copy_dir_all(&abs_p, &dest_pkg_dir)?;
                    println!(
                        "  {} [lokal] {} disalin ke vendor",
                        "v".green().bold(),
                        name.bold()
                    );
                    count += 1;
                }
            }
            DependencyLocation::Git(url) => {
                let cache_dir = manifest_dir.join(".dapcode").join("dependensi").join(name);
                if !cache_dir.exists() {
                    let _ = fetch_git_dependency(url, &cache_dir);
                }
                if cache_dir.exists() {
                    copy_dir_all(&cache_dir, &dest_pkg_dir)?;
                    println!(
                        "  {} [git] {} disalin ke vendor",
                        "v".green().bold(),
                        name.bold()
                    );
                    count += 1;
                }
            }
            DependencyLocation::Version(_) => {
                let cache_dir = manifest_dir.join(".dapcode").join("dependensi").join(name);
                if cache_dir.exists() {
                    copy_dir_all(&cache_dir, &dest_pkg_dir)?;
                    println!(
                        "  {} [versi] {} disalin ke vendor",
                        "v".green().bold(),
                        name.bold()
                    );
                    count += 1;
                }
            }
        }
    }

    let lockfile = Lockfile::generate_or_update(&manifest, manifest_dir).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;

    let lockfile_path = manifest_dir.join(Lockfile::LOCKFILE_FILENAME);
    lockfile.save_to_file(&lockfile_path)?;

    println!(
        "{} Berhasil mem-vendor {} dependensi ke '{}'. Proyek kini siap dikompilasi secara offline!",
        "sukses:".green().bold(),
        count,
        vendor_dir.display()
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
        PkgCommands::Vendor { target } => cmd_vendor(Some(target)),
    }
}

pub fn handle_mobile_cmd(cmd: MobileCommands) -> io::Result<()> {
    match cmd {
        MobileCommands::Init { name } => crate::mobile::cmd_mobile_init(name.as_deref()),
        MobileCommands::Build { file, output, name } => {
            let target = resolve_target_file(file.as_deref())?;
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

            let app_title = name.unwrap_or_else(|| {
                target.file_stem().and_then(|s| s.to_str()).unwrap_or("dapcode_app").to_string()
            });

            let out_dir = output.unwrap_or_else(|| {
                target.parent().unwrap_or(Path::new(".")).join(format!("{}_mobile", app_title))
            });

            println!(
                "{} Mengompilasi '{}' ke bundel aplikasi Mobile...",
                "info:".blue().bold(),
                target.display()
            );

            let mut emitter = crate::wasm::WasmEmitter::new();
            match emitter.compile_program(&program) {
                Ok(wasm_bytes) => {
                    if let Err(e) = crate::mobile::build_mobile_bundle(&wasm_bytes, &app_title, &out_dir) {
                        eprintln!("{} Gagal membuat bundel mobile: {}", "kesalahan:".red().bold(), e);
                    }
                }
                Err(diag) => {
                    eprint!("{}", diag.render(&sm));
                }
            }
            Ok(())
        }
        MobileCommands::Serve { file, port, open } => {
            crate::mobile::cmd_mobile_serve(file.as_deref(), port, open)
        }
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

    let default_out = if Path::new("Cargo.toml").exists() && Path::new("docs/index.html").exists() {
        let current_html = fs::read_to_string("docs/index.html").unwrap_or_default();
        if current_html.contains("Compiler & Internals") || current_html.contains("Developer Portal") {
            PathBuf::from("docs/api")
        } else {
            PathBuf::from("docs")
        }
    } else {
        PathBuf::from("docs")
    };
    let out_dir = out_opt.unwrap_or(&default_out);
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
        let file_to_open = if out_opt.is_none() && Path::new("docs/index.html").exists() {
            let html_content = fs::read_to_string("docs/index.html").unwrap_or_default();
            if html_content.contains("Compiler & Internals") || html_content.contains("Developer Portal") {
                Some(PathBuf::from("docs/index.html"))
            } else {
                generated_files.iter().find(|p| p.extension().and_then(|e| e.to_str()) == Some("html")).cloned()
            }
        } else {
            generated_files.iter().find(|p| p.extension().and_then(|e| e.to_str()) == Some("html")).cloned()
        };

        if let Some(html_file) = file_to_open {
            println!("info: Membuka dokumentasi di '{}'...", html_file.display());
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

/// Menjalankan server Web Playground interaktif DapCode lokal
pub fn cmd_playground(port: u16, open: bool) -> io::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       🚀 DAPCODE INTERACTIVE WEB PLAYGROUND & COMPILER       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  🌐 Server URL      : http://localhost:{:<22} ║", port);
    println!("║  ⚡ Live Kompilasi  : http://localhost:{:<9}/api/kompilasi ║", port);
    println!("║  📂 Folder Statis   : examples/publik/                       ║");
    println!("║  🎨 Canvas 2D       : WebAssembly Hardware Accelerated       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\nTekan Ctrl+C untuk menghentikan server playground.\n");

    if open {
        let url = format!("http://localhost:{}", port);
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", "", &url])
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
        }
    }

    let handler = move |req: crate::net::HttpServerRequest| -> Result<crate::net::HttpServerResponse, String> {
        let mut resp = crate::net::HttpServerResponse {
            status: 200,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: String::new(),
            raw_body: None,
            headers: std::collections::HashMap::new(),
        };
        resp.headers.insert("Access-Control-Allow-Origin".to_string(), "*".to_string());
        resp.headers.insert("Access-Control-Allow-Methods".to_string(), "GET, POST, OPTIONS".to_string());
        resp.headers.insert("Access-Control-Allow-Headers".to_string(), "Content-Type".to_string());

        if req.method == "OPTIONS" {
            resp.status = 204;
            return Ok(resp);
        }

        if req.path == "/api/status" {
            resp.content_type = "application/json".to_string();
            resp.body = r#"{"status": "online", "versi": "0.5.0", "playground": true}"#.to_string();
            return Ok(resp);
        }

        if req.path == "/api/kompilasi" && req.method == "POST" {
            resp.content_type = "application/json".to_string();
            let json_parsed = match crate::json::parse_json(&req.body) {
                Ok(j) => j,
                Err(_) => {
                    resp.body = r#"{"sukses": false, "galat": "Format JSON tidak valid"}"#.to_string();
                    return Ok(resp);
                }
            };
            let kode = match json_parsed.get("kode").and_then(|v| v.as_str()) {
                Some(k) => k,
                None => {
                    resp.body = r#"{"sukses": false, "galat": "Properti 'kode' tidak ditemukan pada body JSON"}"#.to_string();
                    return Ok(resp);
                }
            };

            let mut lexer = crate::lexer::Lexer::new(&kode, 0);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(d) => {
                    let err_json = format!(r#"{{"sukses": false, "galat": "{}"}}"#, d.message.replace('"', "\\\"").replace('\n', "\\n"));
                    resp.body = err_json;
                    return Ok(resp);
                }
            };

            let mut parser = crate::parser::Parser::new(tokens, 0);
            let program = match parser.parse_program() {
                Ok(p) => p,
                Err(d) => {
                    let err_json = format!(r#"{{"sukses": false, "galat": "{}"}}"#, d.message.replace('"', "\\\"").replace('\n', "\\n"));
                    resp.body = err_json;
                    return Ok(resp);
                }
            };

            let mut tc = crate::types::TypeChecker::new(0);
            if let Err(d) = tc.check_program(&program) {
                let err_json = format!(r#"{{"sukses": false, "galat": "{}"}}"#, d.message.replace('"', "\\\"").replace('\n', "\\n"));
                resp.body = err_json;
                return Ok(resp);
            }

            let mut emitter = crate::wasm::WasmEmitter::new();
            let wasm_bytes = match emitter.compile_program(&program) {
                Ok(b) => b,
                Err(d) => {
                    let err_json = format!(r#"{{"sukses": false, "galat": "{}"}}"#, d.message.replace('"', "\\\"").replace('\n', "\\n"));
                    resp.body = err_json;
                    return Ok(resp);
                }
            };

            let b64 = crate::crypto::ke_base64_bytes(&wasm_bytes);
            resp.body = format!(
                r#"{{"sukses": true, "wasm_base64": "{}", "ukuran_byte": {}}}"#,
                b64,
                wasm_bytes.len()
            );
            return Ok(resp);
        }

        // Static files from examples/publik
        let raw_path = if req.path == "/" { "/index.html" } else { &req.path };
        let rel_path = raw_path.trim_start_matches('/');
        let file_path = std::path::Path::new("examples").join("publik").join(rel_path);

        if file_path.exists() && file_path.is_file() {
            if let Ok(content_bytes) = std::fs::read(&file_path) {
                let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let mime = match ext {
                    "html" => "text/html; charset=utf-8",
                    "css" => "text/css; charset=utf-8",
                    "js" => "application/javascript; charset=utf-8",
                    "wasm" => "application/wasm",
                    "json" => "application/json",
                    "png" => "image/png",
                    "svg" => "image/svg+xml",
                    _ => "application/octet-stream",
                };
                resp.content_type = mime.to_string();
                resp.raw_body = Some(content_bytes);
                return Ok(resp);
            }
        }

        resp.status = 404;
        resp.content_type = "text/html; charset=utf-8".to_string();
        resp.body = "<h1>404 Halaman Tidak Ditemukan</h1><p>DapCode Playground</p>".to_string();
        Ok(resp)
    };

    let ws_handler = |_fn_name: &str, _incoming_text: &str| -> Option<String> {
        None
    };

    crate::net::run_http_server(port, 0, handler, ws_handler)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}

/// Menampilkan dashboard sambutan interaktif penuh warna (full-color CLI welcome dashboard)
pub fn print_welcome_dashboard() {
    println!();
    // 1. ASCII Art Logo dengan gradasi warna
    println!("{}", "      ____              ______          __   ".bright_cyan().bold());
    println!("{}", "     / __ \\____ _____  / ____/___  ____/ /__ ".cyan().bold());
    println!("{}", "    / / / / __ `/ __ \\/ /   / __ \\/ __  / _ \\".blue().bold());
    println!("{}", "   / /_/ / /_/ / /_/ / /___/ /_/ / /_/ /  __/".bright_purple().bold());
    println!("{}", "  /_____/\\__,_/ .___/\\____/\\____/\\__,_/\\___/ ".magenta().bold());
    println!("{}", "             /_/                             ".dimmed());
    println!();

    // 2. Banner Header
    println!("  {}", "════════════════════════════════════════════════════════════════════════════".cyan());
    println!(
        "  {} {}  {}",
        "🚀".bright_yellow(),
        "DAPCODE PROGRAMMING LANGUAGE & COMPILER TOOLCHAIN".bright_white().bold(),
        "v0.1.0".bright_green().bold()
    );
    println!(
        "     {}",
        "Bahasa Pemrograman Modern Berbahasa Indonesia (AOT C11 • Wasm32 • Interp)".bright_cyan()
    );
    println!("  {}", "════════════════════════════════════════════════════════════════════════════".cyan());
    println!();

    // 3. Status Badges
    println!(
        "  {} {}  {} {}  {} {}  {} {}",
        "STATUS:".bright_black().bold(),
        "● AKTIF".green().bold(),
        "MEMORI:".bright_black().bold(),
        "Auto-Ownership (Perceus)".magenta().bold(),
        "TARGET:".bright_black().bold(),
        "Native C11 & Wasm".cyan().bold(),
        "EDITOR:".bright_black().bold(),
        "VS Code Extension v0.7.2".yellow().bold()
    );
    println!();

    // 4. Kategori: Alur Kerja Utama
    println!("  {}", "⚡ ALUR KERJA UTAMA (PRIMARY WORKFLOW)".bright_yellow().bold());
    println!("  {}", "────────────────────────────────────────────────────────────────────────────".bright_black());
    println!(
        "    {} {:<22} {}",
        "•".bright_green(),
        "init <nama>".green().bold(),
        "Inisialisasi proyek DapCode baru dengan manifes DapCode.toml".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_yellow(),
        "run [file.dap]".yellow().bold(),
        "Jalankan kode sumber secara instan via Runtime Interpreter".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_cyan(),
        "build [file.dap]".cyan().bold(),
        "Kompilasi Ahead-of-Time ke biner mandiri (.exe native / .wasm)".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_blue(),
        "check [file.dap]".blue().bold(),
        "Pemeriksaan tipe statis & borrow-checker analisis kepemilikan".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_magenta(),
        "test [file.dap]".magenta().bold(),
        "Jalankan test runner terisolasi (blok uji & asersi pastikan)".white()
    );
    println!();

    // 5. Kategori: Peralatan Pengembang & Eksplorasi
    println!("  {}", "🛠️  ALAT PENGEMBANG & EKSPLORASI (DEVELOPER TOOLS)".bright_cyan().bold());
    println!("  {}", "────────────────────────────────────────────────────────────────────────────".bright_black());
    println!(
        "    {} {:<22} {}",
        "•".bright_yellow(),
        "repl".bright_yellow().bold(),
        "Buka konsol interaktif REPL (Read-Eval-Print Loop)".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_cyan(),
        "playground --open".bright_cyan().bold(),
        "Jalankan server Web Playground interaktif dengan editor & Canvas 2D".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_blue(),
        "doc --open".bright_blue().bold(),
        "Hasilkan dokumentasi API proyek (Markdown & HTML) dan buka di browser".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_green(),
        "fmt".bright_green().bold(),
        "Format kode sumber DapCode (.dap) secara konsisten dan rapi".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_purple(),
        "lsp".bright_purple().bold(),
        "Jalankan Language Server Protocol untuk editor VS Code / IDE".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_red(),
        "debug [file.dap]".bright_red().bold(),
        "Mulai sesi debugger CLI interaktif (breakpoint & inspeksi variabel)".white()
    );
    println!();

    // 6. Kategori: Manajer Paket Resmi (dappkg)
    println!("  {}", "📦 MANAJER PAKET RESMI (dappkg)".bright_magenta().bold());
    println!("  {}", "────────────────────────────────────────────────────────────────────────────".bright_black());
    println!(
        "    {} {:<22} {}",
        "•".bright_magenta(),
        "dappkg add <paket>".magenta().bold(),
        "Tambah dependensi baru ke DapCode.toml (Git URL atau folder lokal)".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_green(),
        "dappkg install".green().bold(),
        "Unduh semua dependensi dan kunci integritas hash ke DapCode.lock".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_yellow(),
        "dappkg vendor".yellow().bold(),
        "Salin dependensi ke vendor/ untuk kompilasi offline 100% tanpa internet".white()
    );
    println!(
        "    {} {:<22} {}",
        "•".bright_cyan(),
        "dappkg tree".cyan().bold(),
        "Visualisasikan hierarki pohon dependensi proyek".white()
    );
    println!();

    // 7. Kategori: Pustaka Standar Bawaan
    println!("  {}", "🔋 PUSTAKA STANDAR UNGGULAN (BATTERIES INCLUDED)".bright_green().bold());
    println!("  {}", "────────────────────────────────────────────────────────────────────────────".bright_black());
    println!(
        "    {} {:<26} {}",
        "🌐".bright_blue(),
        "HTTP Server & Router".bright_blue().bold(),
        "Server mikroservis REST, cocok_rute, parameter dinamis, middleware & CORS".dimmed()
    );
    println!(
        "    {} {:<26} {}",
        "💾".bright_yellow(),
        "BasisData / DapDb".bright_yellow().bold(),
        "Embedded Key-Value database terformat persisten dengan transaksi ACID".dimmed()
    );
    println!(
        "    {} {:<26} {}",
        "📡".bright_cyan(),
        "WebSocket RFC 6455".bright_cyan().bold(),
        "Komunikasi full-duplex dua arah real-time bawaan mandiri".dimmed()
    );
    println!(
        "    {} {:<26} {}",
        "🕒".bright_green(),
        "Waktu & Kalender".bright_green().bold(),
        "Kalender sipil Howard Hinnant, waktu_sekarang, kabisat, format ISO".dimmed()
    );
    println!(
        "    {} {:<26} {}",
        "🔍".bright_purple(),
        "Reguler Ekspresi".bright_purple().bold(),
        "Mesin Regex mandiri: cocok_regex, cari_regex, ekstrak grup, ganti_regex".dimmed()
    );
    println!(
        "    {} {:<26} {}",
        "🎨".bright_magenta(),
        "Koleksi Fungsional".bright_magenta().bold(),
        "Kamus, Daftar, serta metode pipa: .petakan(), .saring(), .lipat(), .setiap()".dimmed()
    );
    println!(
        "    {} {:<26} {}",
        "🎮".bright_red(),
        "Canvas 2D & Audio".bright_red().bold(),
        "Grafis WebAssembly hardware-accelerated dan sintesis audio frekuensi".dimmed()
    );
    println!();

    // 8. Footer Navigasi
    println!("{}", "  ────────────────────────────────────────────────────────────────────────────".bright_black());
    println!(
        "  {} Ketik {} untuk panduan lengkap setiap opsi.",
        "💡".bright_yellow(),
        "dapcodec --help".cyan().bold()
    );
    println!(
        "  {} Dokumentasi & Sumber: {}",
        "📚".bright_blue(),
        "https://github.com/dapcode-lang (atau 'dapcodec doc --open')".bright_blue().underline()
    );
    println!();
}


