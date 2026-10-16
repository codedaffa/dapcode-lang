# 🛠️ Panduan Arsitektur & Rekayasa Kompilator DapCode
## *DapCode Language & Compiler Internals Developer Guide*

Selamat datang di Panduan Resmi Pengembang Internal Kompilator dan Ekosistem Bahasa Pemrograman **DapCode** (`dapcodec`). Dokumen ini disusun khusus bagi kontributor, pemelihara (*maintainers*), dan pengembang inti (*core developers*) untuk memahami rancangan arsitektur, struktur kode internal di `src/`, alur kerja pipeline kompilasi, serta tata cara menambah fitur baru ke dalam bahasa DapCode.

Versi HTML interaktif yang dilengkapi pencarian instan dan diagram visual dapat dibuka di peramban web melalui berkas:
👉 **[docs/index.html](index.html)** atau perintah `dapcodec doc --open`.

---

## 📑 Daftar Isi

1. [Filosofi Desain & Arsitektur Umum](#1-filosofi-desain--arsitektur-umum)
2. [Pipeline Alur Kompilasi (End-to-End Compiler Pipeline)](#2-pipeline-alur-kompilasi-end-to-end-compiler-pipeline)
3. [Peta & Anatomi Modul Internal (`src/`)](#3-peta--anatomi-modul-internal-src)
4. [Sistem Tipe & Inferensi Hindley-Milner (`src/types/`)](#4-sistem-tipe--inferensi-hindley-milner-srctypes)
5. [Manajemen Memori Deterministik Perceus (`src/ownership/`)](#5-manajemen-memori-deterministik-perceus-srcownership)
6. [Empat Target Kompilasi & Mobile Bundler (Interpreter, C11 AOT, Wasm32, Mobile PWA)](#6-empat-target-kompilasi--mobile-bundler)
7. [Pustaka Standar Bawaan (*Standard Library Batteries*)](#7-pustaka-standar-bawaan-standard-library-batteries)
8. [Language Server Protocol (LSP) & Debug Adapter Protocol (DAP)](#8-language-server-protocol-lsp--debug-adapter-protocol-dap)
9. [Manajer Paket `dappkg` & Sistem Modul Multi-Berkas](#9-manajer-paket-dappkg--sistem-modul-multi-berkas)
10. [Panduan Praktis: Menambahkan Sintaksis / Kata Kunci Baru](#10-panduan-praktis-menambahkan-sintaksis--kata-kunci-baru)
11. [Panduan Praktis: Menambahkan Fungsi Bawaan ke Stdlib](#11-panduan-praktis-menambahkan-fungsi-bawaan-ke-stdlib)
12. [Panduan Praktis: Menambahkan Diagnostik & Code Action LSP](#12-panduan-praktis-menambahkan-diagnostik--code-action-lsp)
13. [Pengujian, Verifikasi, & Alat Debugging Kompilator](#13-pengujian-verifikasi--alat-debugging-kompilator)

---

## 1. Filosofi Desain & Arsitektur Umum

DapCode dirancang dengan prinsip:
* **Keamanan Memori 100% Tanpa Garbage Collector (GC):** Memanfaatkan analisis kepemilikan kompilasi (*compile-time ownership*) dan *Perceus reference counting* deterministik, menyisipkan pelepasan memori (*drop*) secara otomatis pada titik penggunaan terakhir (*last-use*).
* **Zero Null Pointer Exception:** Nilai tidak pernah bernilai `null` / `nil`. Hasil yang dapat gagal dimodelkan secara matematis dengan *Error Union* (`T ! E`).
* **Sintaksis Alami Bahasa Indonesia:** Menggantikan kata kunci tradisional dengan ekspresi bahasa Indonesia yang ergonomis (`fungsi`, `jika`, `selama`, `untuk`, `struktur`, `sifat`, `terapkan`, `kembali`, `gagal`, `tangkap`, `uji`, `pastikan`).
* **Portabilitas Empat Target Eksekusi:**
  1. *Runtime Tree-walk Interpreter*: Eksekusi instan tanpa waktu kompilasi (`dapcodec run`).
  2. *Ahead-Of-Time (AOT) C11 Transpiler*: Menghasilkan biner mesin mandiri berperforma tinggi (`dapcodec build`).
  3. *Standalone Wasm32 Bytecode Generator*: Menghasilkan biner WebAssembly tanpa ketergantungan pada LLVM (`dapcodec build --wasm`).
  4. *Mobile PWA & Hybrid Android WebView Bundler*: Menghasilkan aplikasi mobile mandiri lengkap dengan Web App Manifest, Service Worker offline, dan template proyek Android Studio siap rilis APK (`dapcodec mobile build` / `dapcodec build --pwa`).

---

## 2. Pipeline Alur Kompilasi (End-to-End Compiler Pipeline)

Setiap berkas sumber `.dap` diproses melalui tahapan-tahapan berikut:

```text
 ┌──────────────────────┐
 │  Kode Sumber (.dap)  │
 └──────────┬───────────┘
            │
            ▼
 ┌──────────────────────────────────────┐
 │ 1. Lexer & Scanner (src/lexer/)      │ --> Menghasilkan Vec<Token> & TokenKind
 └──────────┬───────────────────────────┘
            │
            ▼
 ┌──────────────────────────────────────┐
 │ 2. Pratt Parser (src/parser/)        │ --> Menghasilkan AST (Program, Decl, Stmt, Expr)
 └──────────┬───────────────────────────┘
            │
            ▼
 ┌──────────────────────────────────────┐
 │ 3. Type Checker (src/types/)         │ --> Inferensi tipe Hindley-Milner, ErrorUnion, Trait vtables
 └──────────┬───────────────────────────┘
            │
            ▼
 ┌──────────────────────────────────────┐
 │ 4. Comptime Evaluator (src/comptime/)│ --> Evaluasi ekspresi saat kompilasi (zero runtime overhead)
 └──────────┬───────────────────────────┘
            │
            ▼
 ┌──────────────────────────────────────┐
 │ 5. Ownership & Perceus               │ --> Analisis peminjaman (&/&mut), injeksi drop deterministik
 │    (src/ownership/)                  │
 └──────────┬───────────────────────────┘
            │
            ▼
 ┌──────────────────────────────────────┐
 │ 6. IR Lowering (src/ir/)             │ --> Representasi perantara dengan BasicBlocks & drop annotations
 └──────────┬───────────────────────────┘
            │
     ┌──────┴───────────────────────────┬───────────────────────────┐
     ▼                                  ▼                           ▼
[Backend 1: Interpreter]       [Backend 2: AOT C11]       [Backend 3: Wasm32]
  src/eval/mod.rs              src/codegen/mod.rs          src/wasm/mod.rs
         │                              │                           │
         ▼                              ▼                           ▼
 Eksekusi langsung             Biner Native (.exe)         Biner Wasm (.wasm)
 (dapcodec run)                (dapcodec build)            (dapcodec build --wasm)
```

---

## 3. Peta & Anatomi Modul Internal (`src/`)

Berikut adalah pemetaan berkas-berkas Rust di dalam `src/` beserta tanggung jawab intinya:

| Modul / Berkas | Ukuran | Tanggung Jawab Utama |
|---|---|---|
| [`src/lib.rs`](../src/lib.rs) | ~0.5 KB | Root library crate, mendeklarasikan seluruh modul publik `dapcodec`. |
| [`src/main.rs`](../src/main.rs) | ~0.2 KB | Titik masuk eksekusi biner `dapcodec`, inisialisasi Windows console UTF-8 & VT. |
| [`src/span.rs`](../src/span.rs) | ~3.3 KB | Struktur pelacakan posisi `Span`, `FileId`, `Position`, dan manajemen `SourceMap` multi-berkas. |
| [`src/diag.rs`](../src/diag.rs) | ~5.1 KB | Mesin pelaporan kesalahan (diagnostik kesalahan, peringatan, *syntax highlighting* cuplikan baris kode). |
| [`src/ast/mod.rs`](../src/ast/mod.rs) | ~6.9 KB | Definisi struktur pohon sintaksis abstrak: `Decl`, `Stmt`, `Expr`, `TypeExpr`, `Literal`. |
| [`src/lexer/`](../src/lexer/) | ~22.2 KB | `token.rs` & `mod.rs`: Scanner karakter UTF-8, konversi kata kunci Indonesia, operator `:=`, `!`, `->`. |
| [`src/parser/`](../src/parser/) | ~35.4 KB | Recursive descent parser + Pratt parsing untuk ekspresi bertingkat, pemulihan error parsing (*error recovery*). |
| [`src/types/`](../src/types/) | ~128.4 KB | Mesin inferensi tipe statis Hindley-Milner, unifikasi subtipe, pengecekan *trait* & struktur data, registry built-in types. |
| [`src/comptime/`](../src/comptime/) | ~4.8 KB | Evaluator ekspresi waktu kompilasi Zig-style `comptime`. |
| [`src/ownership/`](../src/ownership/) | ~15.3 KB | Analisis kepemilikan variabel, *borrow checking*, inferensi referensi, dan injeksi *drop* deterministik ala Perceus. |
| [`src/ir/`](../src/ir/) | ~21.7 KB | Representasi Perantara (IR) berbasis BasicBlock, linierisasi instruksi dengan anotasi *drop*. |
| [`src/codegen/`](../src/codegen/) | ~180.0 KB | Transpiler AOT ke kode sumber C11, runtime header `c_runtime.h`, pemanggilan GCC/Clang/MSVC ke biner native. |
| [`src/wasm/`](../src/wasm/) | ~76.4 KB | Generator biner WebAssembly (Wasm32) mandiri dengan encoding LEB128 langsung tanpa LLVM. |
| [`src/eval/`](../src/eval/) | ~199.4 KB | Tree-walk Runtime Interpreter, frame memori berlingkup (*scoped environments*), coroutine *colorless* (`jalan_paralel` / `tunggu`). |
| [`src/stdlib/`](../src/) | ~110+ KB | Kumpulan pustaka standar: Jaringan (`src/net/`), Database (`src/db/`), Regex (`src/regex/`), Kalender (`src/time/`), Kriptografi (`src/crypto/`), JSON (`src/json/`). |
| [`src/lsp/`](../src/lsp/) | ~130.5 KB | Server Language Server Protocol (LSP) untuk VS Code: diagnosa real-time, autocompletion, hover tipe, definisi simbol, dan quick fixes. |
| [`src/debug/`](../src/debug/) | ~49.3 KB | Server Debug Adapter Protocol (DAP) dan konsol interaktif debugger CLI (`dapcodec debug`). |
| [`src/modules/`](../src/modules/) | ~33.9 KB | Resolusi modul multi-berkas (`impor`), siklus pencegahan impor, manifes `DapCode.toml`, manajer paket `dappkg`, penguncian integritas SHA-256 `DapCode.lock`. |
| [`src/fmt/`](../src/fmt/) | ~14.9 KB | Pemformat kode sumber otomatis (`dapcodec fmt`). |
| [`src/doc/`](../src/doc/) | ~42.0 KB | Generator dokumentasi API proyek (`dapcodec doc`). |
| [`src/cli/`](../src/cli/) | ~97.5 KB | Penangan baris perintah (*subcommands*), dashboard konsol interaktif, verifikasi lingkungan. |
| [`src/mobile/`](../src/mobile/) | ~48.5 KB | Bundler PWA Mobile, generator Web App Manifest, offline Service Worker, terminal QR Code Wi-Fi dev-server, dan template Android WebView. |

---

## 4. Sistem Tipe & Inferensi Hindley-Milner (`src/types/`)

Sistem tipe DapCode berbasis statis dengan inferensi penuh:

### Representasi Tipe (`Type` Enum di `src/types/mod.rs`):
* `Int`: Bilangan bulat 64-bit (`bulat` / `u64`).
* `Float`: Bilangan desimal 64-bit IEEE 754 (`desimal` / `f64`).
* `String`: Teks UTF-8 (`teks` / `Teks`).
* `Bool`: Boolean `benar` / `salah` (`bool` / `Bool`).
* `Unit`: Tipe kosong tanpa nilai (`unit` / `Unit`).
* `List(Box<Type>)`: Koleksi dinamis homogen `Daftar<T>`.
* `Map(Box<Type>, Box<Type>)`: Koleksi pasangan kunci-nilai `Kamus<K, V>`.
* `ErrorUnion(Box<Type>, Box<Type>)`: Tipe kesatuan kesalahan `T ! E`.
* `Struct(String, Vec<(String, Type)>)`: Struktur rekaman data bernama.
* `Trait(String, Vec<TraitMethod>)`: Kontrak interface/sifat fungsional.
* `Function(Vec<Type>, Box<Type>)`: Fungsi first-class dan penutupan (*closure*).
* `Var(usize)`: Variabel tipe inferensi untuk substitusi unifikasi Hindley-Milner.

### Aturan Error Union (`T ! E`):
1. Jika suatu ekspresi menghasilkan `Type::ErrorUnion(T, E)`, pemanggil dapat mengekstrak nilai sukses `T` dengan operator `?` (yang mempropagasi `E` ke fungsi pemanggil jika gagal).
2. Alternatifnya, pemanggil dapat menangani kegagalan menggunakan blok `? tangkap err { ... }`.
3. Kata kunci `gagal <expr>` memvalidasi bahwa tipe fungsi saat ini adalah `ErrorUnion(_, E)` dan `<expr>` bertipe `E`.

---

## 5. Manajemen Memori Deterministik Perceus (`src/ownership/`)

DapCode tidak menggunakan Garbage Collector (GC) yang menghentikan eksekusi program (*stop-the-world*).

### Prinsip Utama Perceus:
1. **Affine Types & Linear Consumption:** Setiap alokasi memiliki tepat satu pemilik (*single owner*).
2. **Borrow Inference:** Saat variabel diteruskan ke fungsi tanpa kata kunci kepemilikan khusus, kompilator secara otomatis menerapkannya sebagai pinjaman (*borrow* `&T` atau `&mut T`).
3. **Penyisipan Drop pada Penggunaan Terakhir (*Last-Use Drop*):** Kompilator menghitung grafik aliran kontrol (*control-flow graph*). Tepat setelah sebuah variabel dibaca untuk terakhir kalinya di dalam cakupan (*scope*), instruksi pelepasan memori disisipkan deterministik.
4. **Buffer Reuse (*In-place Mutation*):** Jika suatu struktur atau daftar memiliki *reference count* bernilai 1 saat akan dimutasi, mutasi dilakukan langsung di tempat memori lama tanpa alokasi baru.

---

## 6. Empat Target Kompilasi & Mobile Bundler

### A. Runtime Interpreter (`src/eval/mod.rs`)
* Menggunakan representasi runtime `Value` yang ramah evaluasi cepat.
* Mengelola tumpukan lingkungan (*environment frames*) dan coroutine stackless tanpa warna (*colorless coroutines*) melalui `jalan_paralel` dan `tunggu`.
* Sangat cepat untuk iterasi pengembangan harian, REPL, dan skrip singkat (`dapcodec run program.dap`).

### B. Ahead-of-Time C11 Codegen (`src/codegen/mod.rs`)
* Mentranspilasi AST dan IR DapCode ke kode C11 yang mematuhi standar ISO.
* Mengikutsertakan header runtime C mandiri yang mendefinisikan struktur `DapString`, `DapList`, `DapMap`, coroutine coroutine C, dan Perceus RC drops.
* Mengarahkan kompilasi langsung melalui GCC, Clang, atau MSVC untuk menghasilkan berkas `.exe` native mandiri dengan optimasi `-O2` atau `-O3`.

### C. Standalone WebAssembly Bytecode Generator (`src/wasm/mod.rs`)
* Mengkodekan modul biner Wasm32 langsung menggunakan encoder LEB128 internal tanpa bantuan LLVM.
* Menghasilkan seksi Type, Function, Table, Memory, Global, Export, Code, dan Data.
* Menghasilkan berkas `.wasm` mandiri yang siap dijalankan di browser, Node.js, atau runtime Wasmtime/Wasmer.

### D. Mobile PWA & Hybrid Android WebView Bundler (`src/mobile/mod.rs`)
* Mengintegrasikan biner WebAssembly ke dalam shell Progressive Web App (PWA) layar penuh yang dioptimalkan untuk perangkat layar sentuh ponsel Android dan iOS.
* Otomatis menghasilkan `manifest.webmanifest`, `sw.js` (offline caching), dan ikon resolusi tinggi (SVG, 192px PNG, 512px PNG).
* Menyertakan template proyek Android Studio mandiri (`android/`) dengan akselerasi perangkat keras WebView yang siap dirakit ke berkas biner `.apk` via Gradle.
* Menyediakan dev-server lokal Wi-Fi dengan tampilan **QR Code langsung di terminal** (`dapcodec mobile serve`) untuk pengujian instan pada ponsel fisik.

---

## 7. Pustaka Standar Bawaan (*Standard Library Batteries*)

Pustaka standar DapCode diimplementasikan secara terintegrasi dengan kode native pada kompilator:

* **HTTP Server REST & WebSocket (`src/net/mod.rs`):** Engine server HTTP mikroservis mandiri dengan router parameter dinamis, penangan CORS, dan implementasi frame WebSocket RFC 6455 dua arah.
* **BasisData Tersemat DapDb (`src/db/mod.rs`):** Database Key-Value terformat persisten dengan transaksi ACID, Write-Ahead Logging (WAL), dan pencarian awalan (*prefix scan*).
* **Reguler Ekspresi (`src/regex/mod.rs`):** Mesin Regex zero-dependency dengan kuantifier greedy, karakter grup, alternasi, dan jangkar.
* **Kalender Sipil & Waktu (`src/time/mod.rs`):** Perhitungan kalender sipil Howard Hinnant, penentuan tahun kabisat, waktu sekarang berpresisi milidetik, dan format ISO 8601.
* **Kriptografi & Enkoding (`src/crypto/mod.rs`):** Hashing SHA-256 (FIPS 180-4), MD5 (RFC 1321), serta enkoder & dekoder Base64 (RFC 4648).
* **JSON Serializer & Parser (`src/json/mod.rs`):** Serialisasi rekursif dan penguraian string JSON ke struktur data DapCode.
* **Grafis 2D & Sintesis Audio:** Perintah Canvas 2D WebAssembly dan generator frekuensi gelombang sinus audio untuk pengembangan game dan multimedia.

---

## 8. Language Server Protocol (LSP) & Debug Adapter Protocol (DAP)

### LSP (`src/lsp/mod.rs`):
DapCode menyediakan server LSP bawaan penuh yang dapat dijalankan melalui perintah `dapcodec lsp`:
* **Diagnostik Real-Time:** Menangkap kesalahan parsing dan kesalahan tipe saat pengguna mengetik di VS Code.
* **Auto-completion:** Melengkapi nama fungsi bawaan, modul yang diimpor, metode daftar/kamus, dan kata kunci bahasa Indonesia.
* **Hover Documentation:** Menampilkan tanda tangan tipe, deskripsi fungsi, parameter, dan contoh penggunaan markdown.
* **Go to Definition:** Melompat langsung ke deklarasi fungsi, struktur, atau sifat baik di berkas lokal maupun antar berkas modul.
* **Code Actions / Quick Fixes:** Koreksi otomatis untuk kesalahan umum (misalnya menyarankan pergantian `=` menjadi `:=` pada deklarasi baru, atau auto-stubbing fungsi `utama`).

### DAP & CLI Debugger (`src/debug/mod.rs` & `src/debug/cli.rs`):
Dijalankan melalui `dapcodec debug <berkas.dap>`:
* Mendukung *breakpoint* pada baris tertentu.
* Langkah eksekusi interaktif: *Step Over* (`n`), *Step In* (`s`), *Continue* (`c`).
* Inspeksi nilai variabel lokal dan tumpukan panggilan (*call stack*).

---

## 9. Manajer Paket `dappkg` & Sistem Modul Multi-Berkas

Modul di DapCode dirancang modular dengan pemisahan dependensi yang aman:

* **Berkas Manifes `DapCode.toml`:** Mendefinisikan metadata paket, versi semver, titik masuk utama (`src/utama.dap`), dan dependensi paket lain.
* **Penguncian Versi `DapCode.lock`:** Menyimpan pohon dependensi terperinci beserta hash SHA-256 integritas paket untuk menjamin *reproducible builds*.
* **Penyalinan Offline `dappkg vendor`:** Mendownload semua dependensi eksternal ke dalam direktori lokal `vendor/` sehingga proyek dapat dikompilasi 100% tanpa koneksi internet.
* **Deteksi Siklus Impor:** `src/modules/mod.rs` mendeteksi dependensi melingkar (*circular imports*) pada fase kompilasi awal dan memberikan pesan kesalahan yang ramah.

---

## 10. Panduan Praktis: Menambahkan Sintaksis / Kata Kunci Baru

Jika Anda ingin menambahkan kata kunci baru (contoh: kata kunci `ulangi` untuk perulangan loop):

### Langkah 1: Tambahkan Token di `src/lexer/token.rs`
1. Buka `src/lexer/token.rs`.
2. Tambahkan varian baru pada `enum TokenKind`:
   ```rust
   pub enum TokenKind {
       // ...
       Ulangi, // kata kunci 'ulangi'
   }
   ```
3. Tambahkan representasi string-nya pada implementasi `Display for TokenKind`.

### Langkah 2: Daftarkan Kata Kunci di `src/lexer/mod.rs`
Buka `src/lexer/mod.rs` pada fungsi pencocokan kata kunci (`match_keyword`):
```rust
"ulangi" => TokenKind::Ulangi,
```

### Langkah 3: Tambahkan Node AST di `src/ast/mod.rs`
Tentukan apakah kata kunci ini merupakan Pernyataan (`Stmt`) atau Ekspresi (`Expr`):
```rust
pub enum Stmt {
    // ...
    UlangiLoop {
        jumlah: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
}
```

### Langkah 4: Tulis Logika Parsing di `src/parser/mod.rs`
Pada `parse_stmt`:
```rust
if self.peek_kind() == TokenKind::Ulangi {
    self.advance(); // konsumsi 'ulangi'
    let jumlah = self.parse_expr(Precedence::Lowest)?;
    let body = self.parse_block()?;
    return Ok(Stmt::UlangiLoop { jumlah, body, span });
}
```

### Langkah 5: Tambahkan Validasi Tipe di `src/types/mod.rs`
Pastikan ekspresi `jumlah` bertipe bilangan bulat (`Type::Int`):
```rust
Stmt::UlangiLoop { jumlah, body, span } => {
    let ty = self.check_expr(jumlah)?;
    self.unify(&ty, &Type::Int, *span)?;
    self.check_block(body)?;
}
```

### Langkah 6: Implementasikan di Evaluator & Codegen
1. **Evaluator (`src/eval/mod.rs`):** Tambahkan eksekusi loop pada `eval_stmt`.
2. **C11 Codegen (`src/codegen/mod.rs`):** Emit perulangan `for (int64_t i = 0; i < count; i++) { ... }`.
3. **Format (`src/fmt/mod.rs`):** Tambahkan aturan pencetakan rapi pada formatter.

---

## 11. Panduan Praktis: Menambahkan Fungsi Bawaan ke Stdlib

Jika Anda ingin menambahkan fungsi bawaan baru (misalnya `balik_teks(s: teks) -> teks`):

### Langkah 1: Daftarkan Tanda Tangan Tipe di `src/types/mod.rs`
Cari method `register_builtins` atau `lookup_builtin_type` di `src/types/mod.rs`:
```rust
"balik_teks" => Some(Type::Function(
    vec![Type::String],
    Box::new(Type::String),
)),
```

### Langkah 2: Implementasikan di Evaluator (`src/eval/mod.rs`)
Tambahkan cabang penanganan pada fungsi bawaan di interpreter:
```rust
"balik_teks" => {
    let s = args[0].as_string()?;
    let reversed: String = s.chars().rev().collect();
    Ok(Value::String(reversed))
}
```

### Langkah 3: Implementasikan di C11 Codegen (`src/codegen/mod.rs`)
1. Tambahkan fungsi helper C di bagian header runtime C:
   ```c
   DapString dap_balik_teks(DapString s);
   ```
2. Pada pemetaan panggilan fungsi bawaan di `codegen`, petakan nama DapCode `balik_teks` ke fungsi C `dap_balik_teks`.

### Langkah 4: Tambahkan Dokumentasi Hover di LSP (`src/lsp/mod.rs`)
Tambahkan cuplikan dokumentasi hover agar pengguna mendapatkan bantuan otomatis saat mengetik di editor:
```rust
"balik_teks" => Some("### `fungsi balik_teks(s: teks) -> teks`\n\nMembalik urutan karakter teks dari belakang ke depan."),
```

---

## 12. Panduan Praktis: Menambahkan Diagnostik & Code Action LSP

Ketika ingin membuat kompilator lebih pintar dalam memberikan bantuan ke pengembang:

1. Buka `src/lsp/mod.rs`.
2. Pada fungsi `generate_diagnostics`, periksa pola kode yang rawan salah (misalnya penulisan operator penugasan pada deklarasi variabel baru):
   ```rust
   // Deteksi penulisan '=' alih-alih ':='
   if let Some(err) = detect_assignment_typo(doc) {
       diagnostics.push(Diagnostic {
           range: err.range,
           severity: DiagnosticSeverity::ERROR,
           message: "Variabel belum dideklarasikan. Gunakan ':=' untuk deklarasi awal.".into(),
           // ...
       });
   }
   ```
3. Daftarkan perbaikan instan (*Quick Fix*) pada penangan `textDocument/codeAction` agar editor menampilkan tombol "Koreksi Otomatis".

---

## 13. Pengujian, Verifikasi, & Alat Debugging Kompilator

Kompilator DapCode memiliki cakupan pengujian otomatis yang sangat ketat:

### Menjalankan Pengujian:
```bash
# Menjalankan seluruh 34 unit test & integration test suites
cargo test

# Menjalankan test suite spesifik
cargo test --test ownership_test
cargo test --test codegen_test
cargo test --test lsp_test
```

### Alat Inspeksi Kompilator (Debugging CLI):
Gunakan perintah khusus di bawah ini untuk melihat apa yang sedang terjadi di dalam internal kompilator pada berkas `.dap`:

1. **Inspeksi Abstract Syntax Tree (AST):**
   ```bash
   dapcodec dump-ast berkas.dap
   ```
   Menampilkan pohon sintaks lengkap yang dihasilkan oleh parser.

2. **Inspeksi Representasi Perantara (IR):**
   ```bash
   dapcodec dump-ir berkas.dap
   ```
   Menampilkan instruksi intermediate linier beserta anotasi basic block.

3. **Inspeksi Laporan Kepemilikan & Perceus Drop:**
   ```bash
   dapcodec dump-ownership berkas.dap
   ```
   Menampilkan diagram analisis kepemilikan variabel, peminjaman aktif, dan titik injeksi pelepasan memori (*deterministic drops*).

4. **Konsol Interaktif REPL:**
   ```bash
   dapcodec repl
   ```
   Memungkinkan pengembang menguji evaluasi potongan ekspresi atau tipe data secara instan di terminal.

---

*Dokumen ini diperbarui secara berkala oleh Tim Pengembang Inti DapCode Lang.*  
*Untuk kontribusi atau pelaporan isu internal, silakan buat Pull Request atau Issue di repositori resmi DapCode.*
