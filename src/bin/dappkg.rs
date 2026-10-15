use clap::{Parser as ClapParser, Subcommand};
use dapcodec::cli::{
    cmd_add, cmd_init, cmd_install, cmd_pkg_check, cmd_remove, cmd_tree, cmd_update,
};
use std::io;
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(
    name = "dappkg",
    author = "DapCode Core Team",
    version = "0.1.0",
    about = "Manajer Paket Resmi untuk Bahasa Pemrograman DapCode (DapCode Package Manager)"
)]
pub struct DappkgCli {
    #[command(subcommand)]
    pub command: DappkgCommands,
}

#[derive(Subcommand)]
pub enum DappkgCommands {
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

fn main() {
    let cli = DappkgCli::parse();

    let res: io::Result<()> = match cli.command {
        DappkgCommands::Init { name } => cmd_init(name.as_deref()),
        DappkgCommands::Add {
            name,
            path,
            git,
            version,
        } => cmd_add(&name, path, git, version),
        DappkgCommands::Remove { name } => cmd_remove(&name),
        DappkgCommands::Install => cmd_install(),
        DappkgCommands::Update => cmd_update(),
        DappkgCommands::Tree => cmd_tree(),
        DappkgCommands::Check => cmd_pkg_check(),
    };

    if let Err(e) = res {
        eprintln!("Kesalahan: {}", e);
        std::process::exit(1);
    }
}
