fn main() {
    if let Err(e) = dapcodec::cli::run_cli() {
        eprintln!("Kesalahan: {}", e);
        std::process::exit(1);
    }
}
