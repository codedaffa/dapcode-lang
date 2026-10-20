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
   - **I/O Berkas & Sistem Berkas (Filesystem & File I/O):** `baca_berkas(jalur) -> Teks ! Teks`, `tulis_berkas(jalur, isi) -> Bool ! Teks`, `ada_berkas(jalur) -> Bool`, `buat_direktori(jalur) -> Bool ! Teks`, `daftar_direktori(jalur) -> Daftar<Teks> ! Teks`, `hapus_berkas(jalur) -> Bool ! Teks`, `hapus_direktori(jalur) -> Bool ! Teks`, `gabung_jalur(awal, akhir) -> Teks`.
   - **Utilitas String:** `panjang_teks(s) -> u64`, `ke_huruf_besar(s) -> Teks`, `ke_huruf_kecil(s) -> Teks`.
   - **Utilitas Matematika:** `akar_kuadrat(x) -> f64`, `pangkat(dasar, eksponen) -> f64`, `nilai_mutlak(x) -> f64`.
   - **Utilitas Waktu & Timer:** `waktu_sekarang_ms() -> u64`, `tidur(ms: u64) -> Unit`.
   - **Lingkungan OS & Argumen Baris Perintah (CLI & Environment):** `ambil_argumen_cli() -> Daftar<Teks>`, `ambil_lingkungan(nama) -> Teks ! Teks`, `atur_lingkungan(nama, nilai) -> Unit`.
   - **Kriptografi & Enkoding (Cryptography & Encoding):** Utilitas hashing dan encoding RFC-compliant bawaan: `sha256(teks) -> Teks` (FIPS 180-4 SHA-256 64-karakter hex), `md5(teks) -> Teks` (RFC 1321 MD5 32-karakter hex), `ke_base64(teks) -> Teks` (RFC 4648 Base64 encoder), `dari_base64(teks) -> Teks ! Teks` (RFC 4648 Base64 decoder dengan validasi format).
   - **Reguler Ekspresi (Regex):** Mesin regex zero-dependency bawaan: `cocok_regex(pola, teks) -> Bool`, `cari_regex(pola, teks) -> Daftar<Teks>`, `ganti_regex(pola, teks, pengganti) -> Teks`, dan `ekstrak_regex(pola, teks) -> Teks ! Teks`. Mendukung kelas karakter (`[a-z]`, `\d`, `\w`, `\s`), kuantifier greedy (`*`, `+`, `?`, `{n}`), jangkar (`^`, `$`), dan alternasi (`a|b`).

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
│   ├── uji_regex.dap             # Demonstrasi Reguler Ekspresi (Regex)
│   ├── demo_uji.dap              # Demonstrasi Test Runner Terbawa (uji, pastikan) Demonstrasi Test Runner Terbawa (uji, pastikan)
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
│   ├── bin/
│   │   └── dappkg.rs             # CLI Manajer Paket Resmi DapCode (dappkg)
│   ├── modules/
│   │   ├── mod.rs                # ModuleLoader rekursif & Parser Manifes DapCode.toml
│   │   └── lockfile.rs           # Engine Penguncian Versi & Verifikasi SHA-256 (DapCode.lock)
│   ├── json/
│   │   └── mod.rs                # Engine Parser & Serializer JSON Zero-Dependency
│   ├── crypto/
│   │   └── mod.rs                # Engine Kriptografi SHA-256, MD5 & Enkoding RFC 4648 Base64
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
│       └── mod.rs                # Subperintah CLI dapcodec & dappkg
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
    ├── dappkg_test.rs            # Uji Manajer Paket dappkg, DapCode.lock & Deteksi Integritas
    ├── test_runner_test.rs       # Uji Test Runner Terbawa (uji, pastikan, pastikan_sama)
    ├── repl_test.rs              # Uji REPL Interaktif (Multi-Baris, Delimiter Delta, Meta-Perintah)
    ├── doc_test.rs               # Uji Generator Dokumentasi Otomatis (Markdown & Web HTML)
    ├── crypto_test.rs            # Uji Kriptografi & Enkoding (SHA-256, MD5, Base64, Native C)
    └── demo_test.rs
```

---

## ⚡ Instalasi Cepat (Tanpa Perlu Clone Git)

DapCode dapat langsung dipasang ke sistem operasi Anda secara otomatis tanpa perlu meng-*clone* repositori Git:

### 1. Di Windows (PowerShell)
Jalankan perintah ini di PowerShell:
```powershell
irm https://raw.githubusercontent.com/dapcode/dapcode-lang/master/scripts/install.ps1 | iex
```

### 2. Di Linux & macOS (Terminal / Bash)
Jalankan perintah ini di terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/dapcode/dapcode-lang/master/scripts/install.sh | bash
```

### 3. Menggunakan Cargo Langsung (Bagi Pengguna Rust)
```bash
cargo install --git https://github.com/dapcode/dapcode-lang.git
```

### 4. Unduhan Biner Siap Pakai (GitHub Releases)
Tersedia paket biner mandiri yang dapat langsung diunduh dan diekstrak dari halaman [GitHub Releases](https://github.com/dapcode/dapcode-lang/releases):
- 🪟 **Windows x64:** `dapcodec-*-windows-x64.zip` (berisi `dapcodec.exe`)
- 🐧 **Linux x64:** `dapcodec-*-linux-x64.tar.gz` (berisi `dapcodec`)
- 🍏 **macOS (Apple Silicon & Intel):** `dapcodec-*-macos-arm64.tar.gz` & `dapcodec-*-macos-x64.tar.gz`
- 🧩 **VS Code Extension:** `dapcode-lang-*.vsix`

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

# Jalankan berkas dengan meneruskan argumen baris perintah
dapcodec run app.dap -- argumen1 argumen2 --opsi=aktif

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

### 4. Mengompilasi Program (`build`)
Perintah `build` mendukung tiga target kompilasi:

```powershell
# A. Mode Native Biner Mesin (.exe di Windows / binary di Linux/macOS)
# Menghasilkan biner mandiri teroptimasi -O2 tanpa dependensi runtime
dapcodec build examples/demo.dap -o demo.exe
./demo.exe

# Kompilasi proyek multi-berkas langsung dari DapCode.toml
cd examples/proyek_multiberkas
dapcodec build -o kalkulator.exe
./kalkulator.exe

# B. Mode WebAssembly (--wasm)
# Menghasilkan trio berkas: .wasm (bytecode), .html (peluncur web), dan .js (peluncur Node.js)
dapcodec build examples/demo_kanvas.dap --wasm
node examples/demo_kanvas.js

# C. Mode Mobile & PWA (--pwa / --mobile)
# Menghasilkan bundel lengkap aplikasi mobile (PWA Offline, ikon resolusi tinggi, & template Android Studio)
dapcodec build examples/demo_kanvas.dap --pwa
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
Konsol interaktif modern dengan keunggulan:
- **Masukan Multi-Baris Cerdas:** Otomatis mendeteksi pembukaan kurung kurawal/blok `{ ... }` dengan prompt lanjutan `... ` hingga blok selesai.
- **Persistensi Deklarasi Tingkat Atas:** Definisi `fungsi`, `struktur`, atau implementasi sifat `terapkan` langsung disimpan dan dapat dipanggil di ekspresi berikutnya.
- **Koleksi Perintah Meta:**
  - `:bantuan` / `:help` — Menampilkan panduan dan daftar perintah meta.
  - `:tipe <expr>` / `:type <expr>` — Memeriksa tipe data ekspresi tanpa evaluasi waktu jalan.
  - `:fungsi` — Menampilkan daftar seluruh fungsi yang telah didefinisikan pengguna pada sesi aktif.
  - `:riwayat` / `:history` — Menampilkan riwayat masukan perintah.
  - `:bersih` / `:clear` — Membersihkan layar konsol terminal.
  - `:reset` — Mereset environment dan fungsi ke kondisi awal.
  - `:keluar` / `:exit` — Keluar dari sesi REPL.

### 11. Manajer Paket Resmi `dappkg` & Penguncian Versi (`DapCode.lock`)

DapCode dilengkapi manajer paket resmi **`dappkg`** (setara `composer` di PHP, `npm` di Node.js, `pip` di Python, atau `cargo` di Rust). Semua perintah juga dapat dipanggil melalui `dapcodec`:

```powershell
# Inisialisasi paket baru
dappkg init kalkulator

# Menambahkan dependensi direktori lokal, git, atau versi semver
dappkg add kalkulator --path ../kalkulator
dappkg add jaringan --git https://github.com/dapcode/jaringan.git
dappkg add basisdata --version 1.0.0

# Memasang dependensi & mengunci integritas ke DapCode.lock
dappkg install

# Memeriksa integritas hash anti-tamper terhadap DapCode.lock
dappkg check

# Menampilkan diagram pohon dependensi proyek
dappkg tree

# Memperbarui versi & hash SHA-256 dependensi di DapCode.lock
dappkg update

# Menghapus dependensi dari DapCode.toml & DapCode.lock
dappkg remove jaringan
```

Format berkas penguncian versi **`DapCode.lock`**:
```toml
# BERKAS INI DIHASILKAN SECARA OTOMATIS OLEH DAPCODE / DAPPKG.
# JANGAN EDIT BERKAS INI SECARA MANUAL.

versi_format = 1

[[paket]]
nama = "kalkulator"
versi = "0.5.0"
sumber = "lokal"
target = "../kalkulator"
hash_sha256 = "c6b9074b62f8373bdf70265691efdf38374d75d35a96de71424ef24483ae3a7c"
```

### 12. Menjalankan Unit Test Terbawa (`test`)
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

### 15. Menghasilkan Dokumentasi Proyek (`doc`)
```powershell
# Buat dokumentasi dari proyek saat ini (otomatis disimpan ke folder docs/)
dapcodec doc

# Buat dokumentasi dengan direktori keluaran kustom
dapcodec doc -o dokumentasi_saya

# Pilih format keluaran: markdown, html, atau semua (default: semua)
dapcodec doc --format html

# Buat dokumentasi sekaligus membukanya langsung di peramban web default
dapcodec doc --open
```
Fitur Dokumentasi Otomatis DapCode:
- **Ekstraksi Komentar Dokumen:** Mendukung komentar tingkat modul `//!` dan komentar deklarasi `///` untuk `fungsi`, `struktur`, `sifat`, `terapkan`, dan `uji`.
- **GitHub-Flavored Markdown (`README.md`):** Disertai daftar isi interaktif, tabel modul, tanda tangan fungsi, parameter, dan tabel bidang struktur.
- **Situs Web HTML Interaktif Mandiri (`index.html`):** Berdesain modern *dark-mode*, responsif di perangkat bergerak, dilengkapi fitur pencarian *live search*, tombol salin tanda tangan (*copy to clipboard*), dan 100% *offline-ready* tanpa dependensi jaringan eksternal.

### 16. Server Web HTTP Bawaan & Backend Mikroservis REST API (`buat_server_http`)
DapCode menyediakan mesin HTTP Web Server bawaan tanpa membutuhkan crate atau dependensi eksternal:
```dap
fungsi tangani_permintaan(req: PermintaanHttp) -> ResponsHttp {
    // 1. Endpoint JSON
    jika req.jalur == "/api/produk" {
        kembali respons_json("{\"id\":101,\"nama\":\"Kopi DapCode\"}")
    }

    // 2. Endpoint Teks
    jika req.jalur == "/halo" {
        kembali respons_teks("Halo dari Server DapCode!")
    }

    // 3. Status kustom
    kembali kirim_respons_http(404, "text/plain", "Rute tidak ditemukan")
}

fungsi utama() {
    // Jalankan server HTTP pada port 8080
    buat_server_http(8080, "tangani_permintaan") ? tangkap err {
        cetak("Gagal menjalankan server: {err}")
    }
}
```

### 17. Basis Data Mandiri & Penyimpanan Persisten Lokal (`BasisData` / DapDb)
DapCode menyertakan mesin basis data persisten mandiri berbasis *append-only log* dengan indeks memori (terinspirasi Bitcask) tanpa dependensi eksternal:
```dap
fungsi utama() {
    // 1. Buka atau buat basis data pada disk
    db := buka_basis_data("toko.db") ? tangkap err {
        cetak("Gagal membuka basis data: {err}")
        kembali
    }

    // 2. Simpan nilai teks atau dokumen JSON
    db.simpan("produk:101", "{\"nama\":\"Kopi Arabika\",\"harga\":35000}") ? tangkap err {
        cetak("Gagal simpan: {err}")
    }

    // 3. Periksa keberadaan kunci & ambil data
    jika db.ada("produk:101") {
        data := db.ambil("produk:101") ? tangkap err { "" }
        cetak("Data: {data}")
    }

    // 4. Ambil semua kunci & padatkan log berkas
    kunci_list := db.daftar_kunci() ? tangkap err { Daftar() }
    cetak("Total data: {kunci_list.panjang()}")
    db.padatkan()
}
```

### 18. Web Framework Tingkat Tinggi & DapRouter (`cocok_rute`, Route Params, Middleware & Query)
DapCode menyediakan fasilitas web framework modern tingkat tinggi lengkap dengan routing parameter dinamis, parsing query string dengan fallback nilai bawaan, middleware autentikasi Bearer token, sub-routing awalan, serta pembangun respons pengalihan (redirect):
```dap
fungsi tangani_web(req: PermintaanHttp) -> ResponsHttp {
    klien := req.header_atau("User-Agent", "Klien Anonim")
    cetak("[HTTP] {req.metode} {req.jalur} dari {klien}")

    // 1. Pengalihan Rute (Redirect Middleware)
    jika req.jalur == "/beranda" {
        kembali respons_redirect("/", 301).cors()
    }

    // 2. Middleware Autentikasi untuk Sub-Rute Terproteksi (/api/admin/*)
    jika req.cocok_awalan("/api/admin") {
        token := req.otorisasi_bearer() ? tangkap err {
            kembali respons_401("Akses ditolak: Token Bearer diperlukan").cors()
        }
        jika token != "kunci-rahasia-2026" {
            kembali respons_403("Akses dilarang: Token otorisasi salah").cors()
        }
        sisa := req.sisa_jalur("/api/admin")
        kembali respons_json("{\"admin\":benar,\"sub_rute\":\"{sisa}\"}").cors()
    }

    // 3. Pencocokan Rute Dinamis & Ekstraksi Parameter (:id)
    jika req.cocok("/api/produk/:id") {
        id := req.param("/api/produk/:id", "id") ? tangkap err {
            kembali kirim_respons_http(400, "text/plain", "ID tidak valid: {err}").cors()
        }
        resp := respons_json("{\"id\":\"{id}\",\"nama\":\"Kopi Gayo\"}").cors()
        cetak("Status respons: {resp.status_teks()}")
        kembali resp
    }

    // 4. Penguraian URL Query String dengan Nilai Bawaan (?kata=kopi&halaman=2&urut=murah)
    jika req.jalur == "/api/cari" {
        kata := req.kueri_atau("kata", "semua")
        hal := req.kueri_atau("halaman", "1")
        urut := req.kueri_atau("urut", "terbaru")
        semua := req.semua_kueri()

        kembali respons_json("{\"kata\":\"{kata}\",\"halaman\":{hal},\"urut\":\"{urut}\",\"total_filter\":{semua.panjang()}}").cors()
    }

    // 5. Header Kustom & Dukungan Preflight CORS (OPTIONS)
    jika req.metode == "OPTIONS" {
        kembali respons_teks("OK").cors()
    }

    // 6. Fallback 404
    kembali kirim_respons_http(404, "text/plain", "404 Halaman Tidak Ditemukan: {req.jalur}").cors()
}

fungsi utama() {
    buat_server_http(8080, "tangani_web") ? tangkap err {
        cetak("Server error: {err}")
    }
}
```

### 19. Sajian Aset Statis & Deteksi MIME Otomatis (`sajikan_berkas_statis`, `respons_berkas`, `req.sajikan_statis`)
DapCode menyediakan subsistem penyajian berkas statis (*static file serving*) siap pakai dengan deteksi tipe MIME otomatis, proteksi directory traversal (`..`), dan integritas binary asset:
```dap
fungsi tangani_web(req: PermintaanHttp) -> ResponsHttp {
    // 1. Endpoint API Dinamis
    jika req.jalur == "/api/status" {
        kembali respons_json("{\"status\":\"online\"}").cors()
    }

    // 2. Sajikan Aset Statis dari direktori publik (HTML, CSS, JS, Gambar, Font)
    // Otomatis mengarahkan '/' ke 'index.html' dan mencegah akses di luar direktori
    aset := req.sajikan_statis("publik") ? tangkap err {
        // 3. Fallback jika berkas tidak ditemukan
        kirim_respons_http(404, "text/html; charset=utf-8", "<h1>404 Tidak Ditemukan</h1>")
    }

    kembali aset
}

fungsi utama() {
    buat_server_http(8080, "tangani_web") ? tangkap err {
        cetak("Server error: {err}")
    }
}
```

### 20. Subsistem WebSocket & Komunikasi Real-Time (`buat_server_websocket`, `req.adalah_websocket`, `req.kunci_websocket`, `req.tingkatkan_websocket`, `respons_upgrade_websocket`)
DapCode menyediakan subsistem WebSocket berbasis standar RFC 6455 untuk komunikasi dua arah (*bidirectional*) real-time berkecepatan tinggi, baik melalui rute HTTP terpadu (*unified routing*) maupun server mandiri (*dedicated server*):

#### A. Rute Terpadu HTTP + WebSocket (Handshake 101 Switching Protocols)
```dap
fungsi tangani_pesan_ws(pesan: Teks) -> Teks {
    cetak("💬 Pesan diterima: {pesan}")
    kembali "DapCode Echo: {pesan}"
}

fungsi tangani_web(req: PermintaanHttp) -> ResponsHttp {
    // 1. Upgrade permintaan HTTP ke WebSocket RFC 6455
    jika req.jalur == "/ws" && req.adalah_websocket() {
        kembali req.tingkatkan_websocket("tangani_pesan_ws") ? tangkap err {
            kirim_respons_http(400, "text/plain", "Gagal upgrade: {err}")
        }
    }

    // 2. Sajikan aset statis dan REST API
    aset := req.sajikan_statis("publik") ? tangkap err {
        kirim_respons_http(404, "text/html", "<h1>404 Tidak Ditemukan</h1>")
    }
    kembali aset
}

fungsi utama() {
    buat_server_http(8080, "tangani_web") ? tangkap err {
        cetak("Server error: {err}")
    }
}
```

#### B. Dedicated Standalone WebSocket Server
```dap
fungsi tangani_pesan(pesan: Teks) -> Teks {
    kembali "Echo: {pesan}"
}

fungsi utama() {
    buat_server_websocket(9001, "tangani_pesan") ? tangkap err {
        cetak("Gagal menjalankan server WebSocket: {err}")
    }
}
```
- **Kepatuhan Penuh RFC 6455:** Mendukung kalkulasi SHA-1 + GUID `258EAFA5-E914-47DA-95CA-C5AB0DC85B11` Base64 key, decoding frame bertopeng (*masked frame*), payload 7-bit, 16-bit, dan 64-bit, unmasking XOR 4-byte, auto-reply ping (opcode `0x9`) ke pong (opcode `0xA`), serta penutupan koneksi graceful (opcode `0x8`).
- **Native C11 Zero-Dependency:** Kompilasi langsung ke biner native C11 berkecepatan tinggi via GCC (`-lws2_32` di Windows dan POSIX BSD sockets di Linux/macOS).

### 21. Mesin Reguler Ekspresi Mandiri (`cocok_regex`, `cari_regex`, `ganti_regex`, `ekstrak_regex`, `ekstrak_grup_regex`, `pisah_regex`)
DapCode menyertakan pustaka standar Reguler Ekspresi (Regex) 100% murni Rust (*zero external dependencies*) dengan dukungan fitur regex modern:
- **Karakter & Shorthand:** `\d` (digit), `\D` (non-digit), `\w` (word), `\W` (non-word), `\s` (whitespace), `\S` (non-whitespace), `.` (semua karakter).
- **Kuantifier Greedy:** `*` (0 atau lebih), `+` (1 atau lebih), `?` (opsional 0 atau 1), `{n}` (tepat n), `{n,}` (minimal n), `{n,m}` (antara n dan m).
- **Jangkar & Batas Kata:** `^` (awal teks), `$` (akhir teks), `\b` (batas kata / *word boundary*), `\B` (bukan batas kata).
- **Grup & Alternasi:** `(...)` (grup penangkapan / *capturing groups*), `|` (alternasi logika ATAU).

```dap
fungsi utama() {
    teks := "Server dimulai pada 2026-10-16 pukul 04:30 WIB"

    // 1. Ekstraksi Grup Penangkapan (...)
    grup := ekstrak_grup_regex("(\\d\{4\})-(\\d\{2\})-(\\d\{2\})", teks) ? tangkap err {
        Daftar()
    }
    jika grup.panjang() > 0 {
        cetak("Tanggal: {grup.ambil(0)}") // 2026-10-16 (Full match)
        cetak("Tahun  : {grup.ambil(1)}") // 2026 (Group 1)
        cetak("Bulan  : {grup.ambil(2)}") // 10   (Group 2)
        cetak("Hari   : {grup.ambil(3)}") // 16   (Group 3)
    }

    // 2. Pemisahan String Berdasarkan Pola Delimiter (pisah_regex)
    csv := "Kopi Arabika, 35000 , 20 , Minuman Segar"
    kolom := pisah_regex(",\\s*", csv)
    cetak("Jumlah kolom: {kolom.panjang()}") // 4

    // 3. Batas Kata (\b)
    ada_kata := cocok_regex("\\bcat\\b", "the cat sat on a mat") // benar
    tidak_ada := cocok_regex("\\bcat\\b", "concatenate")        // salah
}
```

### 22. Metode Kaya untuk String & Koleksi (`Teks` & `Daftar` Fluent Methods)
DapCode menyediakan metode manipulasi string dan koleksi dinamis bergaya berorientasi objek (*method chaining*) yang berjalan konsisten di Interpreter dan kompilasi Native C11:
- **Metode pada `Teks`:**
  - `.panjang() -> u64`: Mengembalikan jumlah karakter Unicode dalam teks.
  - `.huruf_besar() -> Teks`: Mengubah teks menjadi huruf kapital semua (*uppercase*).
  - `.huruf_kecil() -> Teks`: Mengubah teks menjadi huruf kecil semua (*lowercase*).
  - `.berisi(sub: Teks) -> Bool`: Memeriksa apakah teks memuat substring tertentu.
  - `.awalan(prefix: Teks) -> Bool`: Memeriksa apakah teks diawali dengan awalan tertentu (*starts_with*).
  - `.akhiran(suffix: Teks) -> Bool`: Memeriksa apakah teks diakhiri dengan akhiran tertentu (*ends_with*).
  - `.pangkas() -> Teks`: Membersihkan spasi di awal dan akhir string (*trim*).
  - `.ganti(lama: Teks, baru: Teks) -> Teks`: Mengganti seluruh kemunculan teks lama dengan teks baru.
  - `.pisah(pemisah: Teks) -> Daftar<Teks>`: Memecah string menjadi daftar substring berdasarkan pemisah.
  - `.potong(awal: u64, panjang: u64) -> Teks`: Mengambil substring dari indeks awal dengan panjang karakter tertentu.
  - `.ambil(indeks: u64) -> Teks ! Teks`: Mengambil satu karakter pada indeks tertentu (dengan penanganan batas aman via Error Union).
- **Metode pada `Daftar`:**
  - `.panjang() -> u64`: Menghitung jumlah elemen dalam daftar.
  - `.ambil(indeks: u64) -> T ! Teks`: Mengambil elemen pada indeks tertentu (aman dengan Error Union).
  - `.tambah(nilai: T) -> Unit`: Menambahkan elemen baru ke akhir daftar.
  - `.hapus(indeks: u64) -> T ! Teks`: Menghapus dan mengembalikan elemen pada indeks tertentu.
  - `.sisip(indeks: u64, nilai: T) -> Bool ! Teks`: Menyisipkan elemen baru pada indeks tertentu.
  - `.berisi(nilai: T) -> Bool`: Memeriksa apakah suatu nilai ada dalam daftar.
  - `.kosong() -> Bool`: Memeriksa apakah daftar kosong (`panjang == 0`).
  - `.balik() -> Daftar<T>`: Mengembalikan daftar baru dengan urutan elemen terbalik (*reverse*).
  - `.gabung(pemisah: Teks) -> Teks`: Menggabungkan seluruh elemen daftar menjadi satu teks dengan pemisah.
  - `.potong(awal: u64, akhir: u64) -> Daftar<T>`: Mengambil sub-daftar dari indeks `awal` hingga sebelum `akhir`.

```dap
fungsi utama() {
    // 1. Manipulasi Teks & Method Chaining
    pesan := "   Selamat Datang di DapCode   "
    bersih := pesan.pangkas().huruf_besar()
    cetak(bersih) // SELAMAT DATANG DI DAPCODE
    
    // 2. Pemecahan String & Operasi Daftar
    daftar := "kopi,teh,matcha,susu".pisah(",")
    daftar.tambah("cokelat")
    daftar.sisip(1, "espresso") ? tangkap err { salah }
    
    // 3. Pembalikan & Penggabungan
    gabung := daftar.balik().gabung(" - ")
    cetak(gabung) // cokelat - susu - matcha - teh - espresso - kopi
}
```

### 23. Koleksi Asosiatif Kamus (In-Memory Map / Dictionary)
DapCode menyediakan tipe data koleksi asosiatif bawaan `Kamus<V>` untuk penyimpanan pasangan kunci-nilai (*key-value map*) in-memory dengan kunci bertipe `Teks`, pencarian cepat O(log N), serialisasi JSON otomatis, dan dukungan penuh pada interpreter maupun kompilasi native C11:

- **Metode pada `Kamus`:**
  - `.pasang(kunci: Teks, nilai: V) -> Unit`: Menyimpan atau menimpa pasangan kunci dan nilai.
  - `.ambil(kunci: Teks) -> V ! Teks`: Mengambil nilai secara aman melalui Error Union (`? tangkap`).
  - `.ambil_atau(kunci: Teks, bawaan: V) -> V`: Mengambil nilai atau mengembalikan nilai bawaan jika kunci tidak ditemukan.
  - `.ada(kunci: Teks) -> Bool`: Memeriksa apakah suatu kunci terdaftar di dalam kamus.
  - `.hapus(kunci: Teks) -> V ! Teks`: Menghapus entri berdasarkan kunci dan mengembalikan nilai yang terhapus.
  - `.panjang() -> u64`: Menghitung jumlah pasangan entri di dalam kamus.
  - `.kosong() -> Bool`: Memeriksa apakah kamus kosong (`panjang == 0`).
  - `.daftar_kunci() -> Daftar<Teks>`: Mengembalikan seluruh kunci aktif terurut leksikografis sebagai objek `Daftar<Teks>`.
  - `.daftar_nilai() -> Daftar<V>`: Mengembalikan seluruh nilai entri aktif sebagai objek `Daftar<V>`.
  - `.ke_json() -> Teks`: Serialisasi kamus ke string format JSON standar (`{"kunci":nilai,...}`).
  - `.bersihkan() -> Unit`: Mengosongkan dan membersihkan seluruh isi pasangan entri di dalam kamus.
  - **Metode Fungsional Koleksi (`Kamus`):**
    - `.petakan_nilai(f: |nilai: V| -> R) -> Kamus<R>`: Mentransformasikan nilai setiap entri dengan closure pemetaan dan mengembalikan `Kamus<R>` baru (kunci tetap utuh).
    - `.saring(f: |kunci: Teks, nilai: V| -> Bool) -> Kamus<V>`: Menyaring pasangan entri kamus berdasarkan predikat closure dan mengembalikan `Kamus<V>` baru.
    - `.setiap(f: |kunci: Teks, nilai: V| -> Unit) -> Unit`: Menjalankan closure callback untuk setiap pasangan kunci-nilai di dalam kamus.
  - **Perulangan `untuk`:** Mendukung iterasi langsung `untuk kunci di kamus { ... }` yang menghasilkan setiap kunci aktif.

```dap
fungsi utama() {
    // 1. Inisialisasi & Penyimpanan Data
    kamus := Kamus()
    kamus.pasang("id", "usr_101")
    kamus.pasang("nama", "Budi Santoso")
    kamus.pasang("peran", "administrator")
    
    // 2. Pengambilan Nilai Aman & Fallback
    nama := kamus.ambil("nama") ? tangkap err { "Anonim" }
    kota := kamus.ambil_atau("kota", "Jakarta")
    cetak("Nama: {nama}, Kota: {kota}")
    
    // 3. Serialisasi JSON
    json := kamus.ke_json()
    cetak("JSON: {json}") // {"id":"usr_101","nama":"Budi Santoso","peran":"administrator"}
    
    // 4. Metode Fungsional Kamus (.petakan_nilai, .saring, .setiap)
    skor := Kamus()
    skor.pasang("Andi", 80)
    skor.pasang("Budi", 95)
    skor.pasang("Cici", 60)
    
    bonus := skor.petakan_nilai(|v| v + 5)
    lulus := skor.saring(|k, v| v >= 75)
    lulus.setiap(|k, v| cetak("Lulus: {k} skor {v}"))
    
    // 5. Iterasi Kunci
    untuk k di kamus {
        val := kamus.ambil_atau(k, "-")
        cetak("{k} => {val}")
    }
}
```

### 24. Pemrograman Fungsional Koleksi & Closure Lambda (`|x| ...`)
DapCode mendukung paradigma pemrograman fungsional modern tingkat tinggi melalui fungsi anonim (*closures / lambda expressions*) dengan sintaks ringkas pipe `|x| ...`, inferensi tipe otomatis, penangkapan variabel lingkungan leksikal (*lexical capture*), dan metode transformasi koleksi fungsional tingkat tinggi pada `Daftar<T>`:

- **Sintaks Closure Lambda:**
  - Tanpa argumen: `|| expr`
  - Argumen tunggal: `|x| expr`
  - Banyak argumen: `|a, b| expr`
  - Anotasi tipe opsional: `|x: u64, y: u64| -> u64 { x + y }`
- **Metode Fungsional pada `Daftar<T>`:**
  - `.petakan(f: |T| -> R) -> Daftar<R>`: Mentransformasikan setiap elemen daftar dengan fungsi/closure pemetaan (*map*).
  - `.saring(predikat: |T| -> Bool) -> Daftar<T>`: Menyaring elemen daftar yang memenuhi kondisi predikat boolean (*filter*).
  - `.lipat(awal: A, folder: |A, T| -> A) -> A`: Mengakumulasi nilai elemen daftar dari nilai awal ke satu hasil akhir (*fold / reduce*).
  - `.setiap(f: |T| -> Unit) -> Unit`: Menjalankan fungsi callback untuk setiap elemen daftar secara berurutan (*for-each*).
  - **Pipeline / Method Chaining:** Metode-metode koleksi dapat dirangkai secara berantai (misal `.saring(...).petakan(...).lipat(...)`) dengan performa tinggi berkat ekspansi *statement expression* inline pada kompilasi C11 tanpa alokasi trampolin heap.

```dap
fungsi utama() {
    angka := Daftar()
    angka.tambah(1)
    angka.tambah(2)
    angka.tambah(3)
    angka.tambah(4)
    angka.tambah(5)

    // 1. Transformasi elemen (.petakan)
    kuadrat := angka.petakan(|x| x * x)

    // 2. Penyaringan elemen (.saring)
    genap := angka.saring(|x| x % 2 == 0)

    // 3. Akumulasi nilai (.lipat)
    total := angka.lipat(0, |acc, x| acc + x) // 15

    // 4. Chaining pipeline
    total_pipa := angka.saring(|x| x > 2).petakan(|x| x * 10).lipat(0, |acc, x| acc + x) // 120

    // 5. Iterasi callback (.setiap)
    genap.setiap(|x| cetak("Angka genap: {x}"))
}
```

---

### 25. Pustaka Standar Waktu & Kalender (Waktu, Tanggal & Durasi)

DapCode menyediakan pustaka standar kalender sipil dan manipulasi waktu mandiri tanpa ketergantungan eksternal (menggunakan algoritma kalender sipil modern Howard Hinnant C++20 chrono):

- **Model Data Struktur `Waktu`:**
  - `tahun: u64`: Tahun kalender Gregorian (contoh: `2026`).
  - `bulan: u64`: Bulan dalam tahun (`1` s.d. `12`).
  - `hari: u64`: Hari dalam bulan (`1` s.d. `31`).
  - `jam: u64`: Jam 24-jam format (`0` s.d. `23`).
  - `menit: u64`: Menit (`0` s.d. `59`).
  - `detik: u64`: Detik (`0` s.d. `59`).
- **Metode pada Objek `Waktu`:**
  - `.ke_epoch() -> u64`: Mengonversi waktu kalender ke detik sejak UNIX epoch (1970-01-01 00:00:00 UTC).
  - `.ke_iso() -> Teks`: Memformat waktu ke standar representasi internasional ISO 8601 (`"YYYY-MM-DDTHH:MM:SSZ"`).
  - `.format(pola: Teks) -> Teks`: Memformat waktu dengan penentu pola format fleksibel (`%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, `%A` nama hari Indonesia, `%B` nama bulan Indonesia, `%%`).
  - `.hari_dalam_minggu() -> Teks`: Mengembalikan nama hari dalam bahasa Indonesia (`"Senin"`, `"Selasa"`, ..., `"Minggu"`).
- **Fungsi Global Waktu & Kalender:**
  - `waktu_sekarang() -> Waktu`: Mengambil waktu dan tanggal lokal sistem saat ini dalam struktur `Waktu`.
  - `waktu_sekarang_detik() -> u64`: Mengembalikan timestamp UNIX epoch dalam detik.
  - `waktu_sekarang_ms() -> u64`: Mengembalikan timestamp UNIX epoch dalam milidetik.
  - `waktu_komponen(epoch_detik: u64) -> Waktu`: Memecah detik epoch menjadi komponen waktu UTC.
  - `waktu_lokal_komponen(epoch_detik: u64) -> Waktu`: Memecah detik epoch menjadi komponen waktu zona lokal sistem.
  - `waktu_ke_epoch(w: Waktu) -> u64`: Mengonversi struktur `Waktu` kembali ke timestamp detik epoch.
  - `waktu_format_iso(epoch_detik: u64) -> Teks`: Memformat detik epoch langsung ke teks ISO 8601.
  - `waktu_format(epoch_detik: u64, pola: Teks) -> Teks`: Memformat detik epoch dengan pola format kustom.
  - `waktu_hari_dalam_minggu(epoch_detik: u64) -> Teks`: Mengembalikan nama hari Indonesia untuk detik epoch tertentu.
  - `waktu_adalah_kabisat(tahun: u64) -> Bool`: Memeriksa apakah suatu tahun adalah tahun kabisat (Leap Year).
  - `tidur(milidetik: u64) -> Unit`: Menjeda eksekusi thread saat ini selama durasi milidetik tertentu.

```dap
fungsi utama() {
    // 1. Ambil waktu lokal saat ini
    kini := waktu_sekarang()
    cetak("Tanggal: {kini.tahun}-{kini.bulan}-{kini.hari}")
    cetak("Hari: {kini.hari_dalam_minggu()}")
    cetak("ISO: {kini.ke_iso()}")

    pola := "%A, %d %B %Y jam %H:%M:%S"
    cetak("Format Cantik: {kini.format(pola)}")

    // 2. Tahun kabisat
    jika waktu_adalah_kabisat(2024) {
        cetak("2024 adalah tahun kabisat!")
    }

    // 3. Mengukur durasi eksekusi (Benchmarking)
    mulai := waktu_sekarang_ms()
    tidur(50) // jeda 50 milidetik
    durasi := waktu_sekarang_ms() - mulai
    cetak("Waktu tunggu: {durasi} ms")
}
```

---

### 24. Grafis Interaktif Canvas 2D & WebAssembly Live Playground
DapCode memiliki kompilator WebAssembly murni tanpa dependensi LLVM yang terintegrasi dengan akselerasi perangkat keras Canvas 2D dan Web Audio synthesizer:
- **Primitif Grafis:** `kanvas_bersihkan`, `kanvas_warna_isi`, `kanvas_warna_garis`, `kanvas_tebal_garis`, `kanvas_persegi`, `kanvas_bingkai_persegi`, `kanvas_persegi_bulat`, `kanvas_bingkai_persegi_bulat`, `kanvas_lingkaran`, `kanvas_bingkai_lingkaran`, `kanvas_garis`, `kanvas_titik`, `kanvas_teks`, `kanvas_teks_tengah`.
- **Dimensi & Input Interaktif:** `kanvas_lebar()`, `kanvas_tinggi()`, `kanvas_mouse_x()`, `kanvas_mouse_y()`, `kanvas_mouse_klik()`, `kanvas_delta_waktu()`.
- **Sintesis Audio 8-Bit & Melodi:** `audio_mainkan_nada`, `audio_efek_suara(tipe)`, `audio_mainkan_gelombang`, `audio_frekuensi_nada`.

```powershell
# Jalankan web server playground interaktif
dapcodec playground --open
```

---

### 25. Pengembangan Aplikasi Mobile Native Direct GPU 60 FPS (`dapcodec mobile`)

DapCode menghadirkan arsitektur aplikasi mobile native berkinerja tinggi menggunakan pendekatan **Direct GPU Rendering 60–120 FPS** (arsitektur mandiri bare-metal ala Flutter & Kotlin Multiplatform) tanpa ketergantungan WebView atau DOM HTML:
- **100% Native C11:** Seluruh siklus render dan penanganan antarmuka dikompilasi ke biner C11 berakselerasi hardware.
- **Direct GPU Acceleration:** OpenGL ES 2.0 / 3.0 pada Android dan Metal / GLKit pada iOS.
- **Zero Touch Latency:** Latensi sentuhan jari 0ms dengan refresh rate layar hingga 120 FPS.
- **Simulator Desktop Mandiri:** Dilengkapi fitur **Live-Watch Hot-Reload** (simpan berkas dengan `Ctrl+S` untuk pembaruan instan tanpa restart).

---

#### 📱 Tahap 1: Cara Memulai Proyek Mobile Baru (`init`)

Inisialisasi kerangka kerja aplikasi mobile baru dengan struktur modular standar industri:
```powershell
# Inisialisasi proyek aplikasi mobile baru
dapcodec mobile init kalkulator_app
cd kalkulator_app
```

**Konfigurasi Manifes `DapCode.toml`:**
```toml
[paket]
nama = "kalkulator_app"
versi = "0.1.0"
berkas_utama = "src/utama.dap"
deskripsi = "Aplikasi Mobile Native Direct GPU dengan DapCode"

[target]
tipe = "mobile"
engine = "flutter"
```

**Hierarki Direktori Proyek yang Dihasilkan:**
```text
kalkulator_app/
├── DapCode.toml              # Manifes paket dan konfigurasi target mobile
├── README.md                 # Petunjuk alur kerja proyek
├── src/                      # Kode sumber aplikasi DapCode
│   ├── utama.dap             # Entry point & siklus render 60 FPS (Controller State)
│   ├── ui/
│   │   └── komponen.dap      # Komponen antarmuka sentuh (Status Bar, App Bar, Keypad)
│   └── logika/
│       └── kalkulator.dap    # Logika bisnis murni terisolasi dari antarmuka
├── uji/                      # Pengujian unit otomatis
│   └── kalkulator_uji.dap    # Test suite logika matematika (dapcodec test)
├── aset/                     # Ikon aplikasi, gambar, font, dan audio
├── android/                  # Proyek Android Studio NDK CMake (OpenGL ES 2.0/3.0)
├── ios/                      # Proyek iOS Xcode Native (GLKit & Metal)
├── desktop/                  # Simulator layar ponsel desktop mandiri (Win32/POSIX)
└── engine/                   # Source code C11 transpiled & DapCode GPU Canvas Engine
```

**Contoh Kode Sumber Entry Point & Controller (`src/utama.dap`):**
```dap
impor "logika/kalkulator.dap"
impor "ui/komponen.dap"

fungsi utama() {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    mx := kanvas_mouse_x()        // Koordinat X sentuhan jari
    my := kanvas_mouse_y()        // Koordinat Y sentuhan jari
    sentuh := kanvas_mouse_klik() // Status sentuhan (1 = aktif, 0 = lepas)

    // 1. Gambar latar belakang gelap elegan
    kanvas_bersihkan()
    kanvas_warna_isi(0.04, 0.06, 0.11, 1.0)
    kanvas_persegi(0.0, 0.0, w, h)

    // 2. Gambar status bar ponsel & jam
    gambar_status_bar(w)

    // 3. Render kartu display glassmorphic & angka
    layar := kanvas_ambil_teks("tampilan")
    riwayat := kanvas_ambil_teks("riwayat")
    gambar_layar_tampilan(w, 40.0, 80.0, riwayat, layar)

    // 4. Render tombol sentuh interaktif ber-radius membulat (squircle)
    tombol("Hitung Sekarang", 20.0, 140.0, w - 40.0, 50.0, mx, my, sentuh)
}
```

---

#### ⚡ Tahap 2: Cara Menjalankan & Menguji Proyek (`preview` / `run`)

Uji coba aplikasi Anda langsung di PC desktop tanpa perlu emulator Android yang lambat:

```powershell
# 1. Jalankan simulator layar ponsel desktop dengan Live Hot-Reload
dapcodec mobile preview src/utama.dap
# (Atau cukup jalankan: dapcodec mobile preview jika di dalam folder proyek)
```

**Fitur Unggulan Simulator:**
- **Live-Watch Hot-Reload:** Watcher memantau berkas sumber secara otomatis. Cukup simpan kode dengan `Ctrl+S`, simulator seketika mengompilasi ulang C11 dan memperbarui tampilan tanpa menutup jendela!
- **Simulasi Layar Sentuh:** Kursor mouse berfungsi sebagai koordinat sentuhan jari (*single-touch / multi-touch*).
- **Preset Ukuran Layar:** Tekan tombol `1` (Portrait 360x640), `2` (Ponsel Besar 400x750), atau `3` (Landscape 640x360).
- **Pengujian Unit Logika:** Jalankan `dapcodec test` untuk memverifikasi logika bisnis tanpa membuka jendela grafis.
- **Integrasi VS Code:** Tekan `Ctrl+Shift+P` -> Pilih `DapCode: Preview Mobile Screen (Direct GPU Simulator with Hot-Reload)`.

---

#### 📦 Tahap 3: Cara Build Proyek Sampai Siap Distribusi (`build`)

Kompilasi dan bundel kode DapCode Anda ke platform native yang diinginkan:
```powershell
# Rakit seluruh bundel platform native
dapcodec mobile build src/utama.dap
```

Perintah di atas menghasilkan 4 target perakitan:

##### A. Target Android (APK / Android App Bundle - AAB via NDK CMake)
1. Buka folder `android/` di **Android Studio**.
2. Pastikan **Android SDK**, **NDK**, dan **CMake** telah terpasang via *SDK Manager*.
3. Klik tombol **Run** (`Shift + F10`) untuk menguji langsung di Emulator Android atau Ponsel fisik via kabel USB.
4. Atau kompilasi file biner via terminal:
   ```bash
   cd android
   # Build APK Debug:
   ./gradlew assembleDebug
   # Lokasi output: android/app/build/outputs/apk/debug/app-debug.apk

   # Build AAB Release untuk Google Play Store:
   ./gradlew bundleRelease
   # Lokasi output: android/app/build/outputs/bundle/release/app-release.aab
   ```
   *Arsitektur:* `DapCodeEngine.java` dan `DapCodeSurfaceView.java` (GLSurfaceView kontinu 60 FPS) menghubungkan siklus render dan touch event (`MotionEvent`) langsung ke kernel C11 `dapcode_engine.c`.

##### B. Target iOS Native (Xcode / iPhone IPA)
1. Buka folder `ios/` di **Xcode** pada komputer macOS.
2. Framework yang disertakan: `OpenGLES.framework` dan `GLKit.framework` / Metal C-Bridge.
3. Jalankan di **Simulator iOS** atau perangkat **iPhone** fisik dengan menekan `Command + R`.
4. Atau rakit biner via terminal macOS menggunakan perintah `xcodebuild`.
   *Arsitektur:* `DapCodeViewController.m` mengelola `EAGLContext` 60 FPS dan meneruskan event sentuhan (`touchesBegan`, `touchesMoved`, `touchesEnded`) ke `dap_engine_on_touch()`.

##### C. Target Desktop Simulator Mandiri (Windows / Linux)
Buka folder `desktop/` dan jalankan script batch instan:
```cmd
cd desktop
kompilasi_dan_jalankan.bat
```
Atau kompilasi langsung menggunakan MinGW GCC:
```bash
gcc preview_main.c ../engine/app_dapcode.c ../engine/dapcode_engine.c -lopengl32 -lgdi32 -lm -O3 -o pratinjau_mobile.exe
```

##### D. Target WebAssembly & Mobile PWA (Browser Smartphone)
```powershell
# Kompilasi ke modul WASM mandiri
dapcodec wasm src/utama.dap -o web_dist/

# Jalankan server lokal untuk pengujian di browser
dapcodec playground --open
```
Aplikasi dapat dibuka di browser Android Chrome atau iOS Safari, lalu pilih menu **"Add to Home Screen"** untuk menginstalnya sebagai Progressive Web App (PWA) layar penuh (*standalone*).

---

## 🎨 Dukungan Editor & Ekstensi VS Code / Antigravity IDE

Ekstensi resmi DapCode tersedia di folder `editors/vscode/` dan telah dikemas ke paket biner **`.vsix`** siap pakai (Versi `0.8.0`):
- **Paket Instalasi Mandiri (`.vsix`):** Tersedia di `editors/vscode/dapcode-lang-0.8.0.vsix`.
  ```powershell
  code --install-extension editors/vscode/dapcode-lang-0.8.0.vsix
  ```
- **Integrasi LSP Cerdas (`extension.js`):**
  - **Diagnostik Real-Time & Multi-Berkas:** Pelaporan kesalahan sintaks, kepemilikan (*borrow check*), dan tipe secara langsung saat mengetik dengan dukungan resolusi modul antar-berkas (`impor "..."`) tanpa peringatan *false positive*.
  - **Hover Tooltips:** Dokumentasi Markdown instan untuk kata kunci, tipe data (`BasisData`, `PermintaanHttp`, `ResponsHttp`, `Teks`, `Daftar`, `Kamus`), fungsi pustaka standar, serta metode fungsional koleksi.
  - **Auto-Completion Kontekstual:** Rekomendasi otomatis kata kunci bahasa Indonesia, tipe data, fungsi pustaka standar, metode koleksi, serta deklarasi lokal.
  - **Quick Fixes & Code Actions (`Ctrl+.`):** Perbaikan otomatis operator penugasan `=`, generator stub fungsi, perbaikan typo, dan pembuatan fungsi `utama()`.
  - **Document Formatting:** Format kode sumber otomatis saat dokumen diformat atau disimpan.
- **Simulator Mobile Direct GPU & Live Playground 2D:**
  - **Mobile GPU Simulator Hot-Reload (`dapcode.previewMobileGpu`):** Simulator desktop layar sentuh ponsel mandiri 60 FPS dengan hot-reload seketika saat menyimpan berkas (`Ctrl+S`).
  - **Live Webview Playground (`dapcode.openPlayground`):** Jendela preview interaktif langsung di dalam editor dengan kompilasi otomatis WASM, pilihan preset frame (Auto-Fit, Desktop, iPhone, Android, Tablet), tombol rotasi orientasi, dan opsi "🌐 Buka di Web".
  - **Pembangun Mobile Native (`dapcode.buildMobile`):** Bangun bundel aplikasi Android NDK dan iOS Xcode dengan satu klik.
- **Koleksi Snippets Lengkap (`snippets/dapcode.json`):** Cuplikan cepat untuk closure lambda `|x|`, metode koleksi fungsional `petakan`, `saring`, `lipat`, `setiap`, `Kamus`, `pasang`, `ambil_atau`, `server_http`, `kanvas_persegi_bulat`, `kanvas_teks_tengah`, `uji`, `pastikan`, dll.
- **Integrasi Menu & Perintah Cepat:**
  - `dapcode.previewMobileGpu` -> Menjalankan simulator layar ponsel Direct GPU dengan Live Hot-Reload.
  - `dapcode.run` -> Jalankan berkas aktif dengan `dapcodec run`.
  - `dapcode.test` -> Jalankan pengujian unit aktif dengan `dapcodec test`.
  - `dapcode.repl` -> Buka sesi konsol interaktif `dapcodec repl`.
  - `dapcode.openPlayground` -> Buka Live Canvas 2D & Playground Webview.
  - `dapcode.buildMobile` -> Bangun aplikasi mobile native (Android, iOS & Desktop).
  - `dapcode.doc` -> Hasilkan dokumentasi dan buka di browser dengan `dapcodec doc --open`.
  - `dapcode.check` -> Periksa tipe dan kepemilikan dengan `dapcodec check`.
  - `dapcode.build` -> Kompilasi berkas aktif ke biner mandiri dengan `dapcodec build`.
  - `dapcode.restartLsp` -> Mulai ulang Language Server.

---

## 🧪 Menjalankan Seluruh Test Suite

```powershell
cargo test
```
Semua 34 modul test (120+ pengujian: Lexer, Parser, Typechecker, Auto-Ownership, Comptime, Concurrency, Error Handling, Stdlib, Modules & Imports, JSON Engine, LSP Server, Jaringan & HTTP Client, HTTP Web Server & Primitives, High-Level Web Router & Middleware, Static Files & MIME Asset Serving, Subsistem WebSocket RFC 6455 & Real-Time Communication, Embedded Database (BasisData & DapDb), Mesin Reguler Ekspresi Mandiri (Regex & Capturing Groups), Metode Kaya Koleksi & String (Fluent Teks & Daftar), Koleksi Asosiatif Kamus (In-Memory Map/Dictionary), Pemrograman Fungsional Koleksi & Closure Lambda (|x| ...), Pustaka Standar Waktu & Kalender (Waktu, Tanggal & Durasi), Audio Synthesis & WebAudio Mock, Manajemen Dependensi & Multi-Paket, Test Runner Bawaan, Interactive REPL, Generator Dokumentasi Otomatis, Kriptografi & Enkoding, Manajer Paket dappkg & DapCode.lock, Demo Pipeline) terverifikasi 100% lolos uji tanpa kesalahan.



