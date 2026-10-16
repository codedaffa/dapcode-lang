fn main() {
    dapcodec::cli::init_windows_console();

    if let Err(e) = dapcodec::cli::run_cli() {
        eprintln!("Kesalahan: {}", e);
        std::process::exit(1);
    }
}
