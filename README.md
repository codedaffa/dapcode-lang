# DapCode (`.dap`) & `dapcodec` Toolchain

**DapCode** adalah bahasa pemrograman modern, aman, dan efisien dengan sintaks bersih tanpa titik koma (*semicolon-free*) yang terinspirasi oleh Kotlin dan Python, dipadukan dengan kata kunci ekspresif berbahasa Indonesia.

Kompilator dan toolchain resminya, **`dapcodec`**, diimplementasikan sepenuhnya dalam bahasa Rust dengan keunggulan sistem tipe aljabar statis, inferensi tipe Hindley-Milner, manajemen memori otomatis berbasis *Compile-Time Ownership* dan *Perceus Reference Counting*, metaprogramming Zig-style `comptime`, konkurensi coroutine stackless tanpa warna (*colorless*), serta sistem modul multi-berkas dengan manifes proyek `DapCode.toml`.

---

## 🚀 Fitur Unggulan

1. **Sintaksis Ergonomis & Alami (Tanpa Titik Koma):**
   Menggunakan kata kunci ekspresif berbahasa Indonesia:
   - `struktur`, `sifat`, `terapkan ... untuk ...`
   - `fungsi`, `jika`, `selain_itu`, `selama`, `untuk ... di ...`, `kembali`
   - `gagal`, `tangkap`, `jalan_paralel`, `tunggu`, `comptime`, `ini`, `impor`

2. **Sistem Tipe Statis Hindley-Milner:**
   - Inferensi tipe otomatis dengan operator walrus `:=`
   - Zero `null`/`nil` untuk menjamin keamanan dari *Null Pointer Exception*
   - Komposisi ketat Struktur (`struktur`) + Sifat/Trait (`sifat`) tanpa *class inheritance*
   - Alias tipe bawaan: `u64`/`bulat`, `f64`/`desimal`, `Teks`/`teks`, `Bool`/`bool`, `Unit`/`unit`

3. **Penanganan Kesalahan Inferred Error Union (`T ! E`):**
   - Propagasi instan dengan operator `?`
   - Blok pemulihan kesalahan inline `? tangkap err { ... }`

4. **Manajemen Memori Perceus & Auto-Ownership (100% Memory-Safe Tanpa GC):**
   - Inferensi peminjaman otomatis (`&` dan `&mut`)
   - Injeksi instruksi `drop()` deterministik pada titik penggunaan terakhir (*last-use*) variabel
   - Deteksi pemakaian ulang buffer (*in-place memory reuse*) ala Perceus

5. **Metaprogramming Zig-Style `comptime`:**
   - Evaluasi blok/fungsi langsung saat fase kompilasi untuk komputasi tanpa beban waktu jalan (*zero runtime overhead*).

6. **Konkurensi Coroutine Colorless:**
   - `jalan_paralel { ... }` untuk meluncurkan tugas asinkron/paralel
   - `tunggu(tugas)` untuk mensinkronisasi dan mengekstrak hasil

7. **Pustaka Standar Bawaan (Standard Library):**
   - **Koleksi Dinamis `Daftar<T>`:** Koleksi ukuran fleksibel dengan metode `.tambah(item)`, `.panjang() -> u64`, dan `.ambil(indeks) -> T ! Teks` (proteksi *zero-null* terpadu dengan *error union*).
   - **Serialisasi & Penguraian JSON:** Parser & serializer JSON zero-dependency terintegrasi: `ke_json(nilai) -> Teks`, `uraikan_json(teks) -> Teks ! Teks`, `ambil_kunci_json(teks, kunci) -> Teks ! Teks`, `ambil_angka_json(teks, kunci) -> f64 ! Teks`, `ambil_bool_json(teks, kunci) -> Bool ! Teks`, `ada_kunci_json(teks, kunci) -> Bool`, `valid_json(teks) -> Bool`.
   - **Jaringan & HTTP Client:** Client HTTP bawaan zero-dependency dengan integrasi TLS/HTTPS native: `ambil_http(url) -> Teks ! Teks` (HTTP GET), `kirim_http_post(url, data_json) -> Teks ! Teks` (HTTP POST dengan payload JSON), dan `ambil_status_http(url) -> u64 ! Teks` (status code numerik).
   - **I/O Berkas (File I/O):** `baca_berkas(jalur) -> Teks ! Teks`, `tulis_berkas(jalur, isi) -> Bool ! Teks`, `ada_berkas(jalur) -> Bool`.
   - **Utilitas String:** `panjang_teks(s) -> u64`, `ke_huruf_besar(s) -> Teks`, `ke_huruf_kecil(s) -> Teks`.
   - **Utilitas Matematika:** `akar_kuadrat(x) -> f64`, `pangkat(dasar, eksponen) -> f64`, `nilai_mutlak(x) -> f64`.
   - **Utilitas Waktu:** `waktu_sekarang_ms() -> u64`.

8. **Sistem Modul & Multi-Berkas (`impor` & `DapCode.toml`):**
   - Kata kunci `impor "nama_berkas.dap"` untuk memecah kode menjadi modul-modul independen.
   - Resolusi rekursif dengan pencegahan siklus (*cycle-safe*) dan pelacakan posisi multi-file akurat pada `SourceMap`.
   - Manifes proyek standar `DapCode.toml` dengan konfigurasi paket dan titik masuk utama.
   - Otomatisasi CLI: jalankan `dapcodec run`, `check`, atau `build` langsung di folder proyek tanpa argumen.

---

## 📂 Struktur Proyek

```
dapcode-lang/
├── Cargo.toml
├── README.md
├── examples/
│   ├── demo.dap                  # Program percontohan resmi DapCode
│   ├── uji_stdlib.dap            # Demonstrasi fitur Pustaka Standar (Stdlib)
│   ├── demo_uji.dap              # Demonstrasi Test Runner Terbawa (uji, pastikan)
│   └── proyek_multiberkas/       # Contoh proyek multi-berkas dengan DapCode.toml
│       ├── DapCode.toml
│       └── src/
│           ├── geometri.dap      # Modul struktur Titik dan Lingkaran
│           ├── matematika.dap    # Modul perhitungan numerik dan deret
│           └── utama.dap         # Titik masuk utama dengan impor modul
├── src/
│   ├── lib.rs                    # Library root dan re-eksport modul
│   ├── main.rs                   # CLI entry point (binary: dapcodec)
│   ├── span.rs                   # Pelacak posisi berkas, baris, dan kolom
│   ├── diag.rs                   # Pelapor diagnostik kesalahan kompilator
│   ├── ast/
│   │   └── mod.rs                # Definisi Abstract Syntax Tree (AST) & ImportDecl
│   ├── lexer/
│   │   ├── mod.rs                # Tokenizer dan lexer DapCode
│   │   └── token.rs              # Definisi Token dan TokenKind (impor, di, dalam)
│   ├── parser/
│   │   └── mod.rs                # Pratt & Recursive Descent Parser
│   ├── modules/
│   │   └── mod.rs                # ModuleLoader rekursif & Parser Manifes DapCode.toml
│   ├── json/
│   │   └── mod.rs                # Engine Parser & Serializer JSON Zero-Dependency
│   ├── net/
│   │   └── mod.rs                # Client Jaringan & HTTP Socket / TLS Native
│   ├── types/
│   │   └── mod.rs                # Type Checker Hindley-Milner, Trait Engine & Stdlib
│   ├── ownership/
│   │   └── mod.rs                # Pass Auto-Ownership & Perceus Analysis
│   ├── ir/
│   │   └── mod.rs                # Representasi Perantara (IR) & Drop Annotations
│   ├── comptime/
│   │   └── mod.rs                # Evaluator Comptime Metaprogramming
│   ├── eval/
│   │   └── mod.rs                # Runtime Engine, Evaluator, Coroutines & Stdlib Runtime
│   ├── codegen/
│   │   └── mod.rs                # Native Code Generator (C99 + GCC Backend)
│   ├── fmt/
│   │   └── mod.rs                # Pemformat Kode Sumber Otomatis (dapcodec fmt)
│   ├── lsp/
│   │   └── mod.rs                # Language Server Protocol (dapcodec lsp)
│   └── cli/
│       └── mod.rs                # Subperintah CLI dapcodec
└── tests/
    ├── lexer_test.rs
    ├── parser_test.rs
    ├── typecheck_test.rs
    ├── ownership_test.rs
    ├── comptime_test.rs
    ├── concurrency_test.rs
    ├── error_handling_test.rs
    ├── stdlib_test.rs            # Uji Pustaka Standar (Daftar, String, Math, Berkas, Waktu, JSON)
    ├── module_test.rs            # Uji Sistem Modul, Impor, & DapCode.toml
    ├── json_test.rs              # Uji Parser, Serializer, & Error Handling JSON
    ├── lsp_test.rs               # Uji LSP (Init, Diagnostics, Hover, Completion)
    ├── net_test.rs               # Uji Jaringan & HTTP Client (GET, POST, Status, 404 Catch)
    ├── dependency_test.rs        # Uji Manajemen Dependensi & Multi-Paket (add, install, remove)
    ├── test_runner_test.rs       # Uji Test Runner Terbawa (uji, pastikan, pastikan_sama)
    └── demo_test.rs
```

---

## 💻 Penggunaan CLI `dapcodec`

### 1. Menginisialisasi Proyek Baru (`init`)
```powershell
# Inisialisasi proyek DapCode baru dengan struktur berkas dan DapCode.toml
dapcodec init nama_proyek
```
Perintah ini otomatis membuat:
- `DapCode.toml`
- `src/utama.dap` (berisi templat `fungsi utama()`)
- `.gitignore`

### 2. Menjalankan Kode Langsung (`run`)
```powershell
# Jalankan berkas tunggal
dapcodec run examples/demo.dap
dapcodec run examples/uji_stdlib.dap

# Jalankan proyek multi-berkas (otomatis membaca DapCode.toml)
cd examples/proyek_multiberkas
dapcodec run
```

### 3. Memeriksa Tipe & Kepemilikan (`check`)
```powershell
# Periksa berkas spesifik
dapcodec check examples/demo.dap

# Periksa proyek aktif (otomatis membaca DapCode.toml)
dapcodec check
```

### 4. Mengompilasi ke Native Executable (`build`)
```powershell
# Kompilasi berkas tunggal
dapcodec build examples/demo.dap -o demo.exe
./demo.exe

# Kompilasi proyek multi-berkas langsung dari DapCode.toml
cd examples/proyek_multiberkas
dapcodec build -o kalkulator.exe
./kalkulator.exe
```

### 5. Menampilkan Representasi Perantara (IR) & Perceus Drops (`dump-ir`)
```powershell
dapcodec dump-ir examples/demo.dap
```

### 6. Menampilkan Laporan Auto-Ownership (`dump-ownership`)
```powershell
dapcodec dump-ownership examples/demo.dap
```

### 7. Menampilkan AST (`dump-ast`)
```powershell
dapcodec dump-ast examples/demo.dap
```

### 8. Memformat Kode Sumber (`fmt`)
```powershell
# Format berkas tertentu
dapcodec fmt examples/demo.dap

# Periksa format tanpa menulis perubahan (cocok untuk CI/CD)
dapcodec fmt --check .
```

### 9. Menjalankan Language Server Protocol (`lsp`)
```powershell
dapcodec lsp
```
Menyediakan diagnostik *real-time*, *hover type info*, *code completion*, dan *formatting* untuk integrasi dengan VS Code, Cursor, Windsurf, Antigravity IDE, Neovim, dsb.

### 10. Membuka Interactive REPL (`repl`)
```powershell
dapcodec repl
```

### 11. Menambahkan Dependensi Paket (`add`)
```powershell
# Tambah dependensi direktori lokal
dapcodec add kalkulator --path ../kalkulator

# Tambah dependensi dari Git
dapcodec add jaringan --git https://github.com/dapcode/jaringan.git

# Tambah dependensi versi semver
dapcodec add basisdata --version 1.0.0
```

### 12. Memasang & Memverifikasi Dependensi (`install`)
```powershell
# Verifikasi semua paket lokal/git/versi yang dideklarasikan di DapCode.toml
dapcodec install
```

### 13. Menghapus Dependensi (`remove`)
```powershell
dapcodec remove jaringan
```

### 14. Menjalankan Unit Test Terbawa (`test`)
```powershell
# Jalankan seluruh pengujian di proyek atau folder tests/
dapcodec test

# Jalankan pengujian pada berkas spesifik
dapcodec test examples/demo_uji.dap

# Jalankan pengujian dengan filter nama tertentu
dapcodec test examples/demo_uji.dap --filter perkalian
```
Mendukung sintaks bawaan:
```dap
uji "perhitungan perkalian" {
    hasil := kali(6, 7)
    pastikan_sama(hasil, 42)
    pastikan(hasil > 40)
    pastikan_tidak_sama(hasil, 0)
}
```

---

## 🎨 Dukungan Editor & Ekstensi VS Code

Ekstensi resmi DapCode tersedia di folder `editors/vscode/` dan telah dikemas ke paket biner **`.vsix`** siap pakai:
- **Paket Instalasi Mandiri (`.vsix`):** Tersedia di `editors/vscode/dapcode-lang-0.1.0.vsix`.
  ```powershell
  code --install-extension editors/vscode/dapcode-lang-0.1.0.vsix
  ```
- **Integrasi LSP Cerdas (`extension.js`):**
  - **Diagnostik Real-Time:** Pelaporan kesalahan sintaks dan tipe secara langsung saat mengetik.
  - **Hover Tooltips:** Dokumentasi Markdown instan untuk kata kunci, fungsi bawaan (`ke_json`, `cetak`, dll), dan simbol lokal.
  - **Auto-Completion Kontekstual:** Rekomendasi otomatis kata kunci bahasa Indonesia, tipe data, fungsi pustaka standar, serta deklarasi lokal.
  - **Document Formatting:** Format kode sumber otomatis saat dokumen diformat atau disimpan.
- **Koleksi Snippets Lengkap (`snippets/dapcode.json`):** Cuplikan cepat untuk `utama`, `fungsi`, `struktur`, `sifat`, `terapkan`, `jika`, `selama`, `untuk`, `jalan_paralel`, `tangkap`, `ke_json`, dan `ambil_kunci_json`.
- **Integrasi Menu & Perintah Cepat:**
  - `dapcode.run` -> Jalankan berkas aktif dengan `dapcodec run`.
  - `dapcode.check` -> Periksa tipe dan kepemilikan dengan `dapcodec check`.
  - `dapcode.build` -> Kompilasi berkas aktif ke file `.exe` mandiri dengan `dapcodec build`.
  - `dapcode.restartLsp` -> Mulai ulang Language Server.

---

## 🧪 Menjalankan Seluruh Test Suite

```powershell
cargo test
```
Semua 15 modul test (39 pengujian: Lexer, Parser, Typechecker, Auto-Ownership, Comptime, Concurrency, Error Handling, Stdlib, Modules & Imports, JSON Engine, LSP Server, Jaringan & HTTP Client, Manajemen Dependensi & Multi-Paket, Test Runner Bawaan, Demo Pipeline) terverifikasi 100% lolos uji tanpa kesalahan.

