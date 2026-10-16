# DapCode Language Support untuk Visual Studio Code

Ekstensi resmi untuk bahasa pemrograman **DapCode (`.dap`)** yang menghadirkan pengalaman pengembangan modern, cepat, cerdas, dan terintegrasi penuh di Visual Studio Code, Cursor, Windsurf, dan Antigravity IDE.

---

## 📦 Instalasi Cepat

Paket biner mandiri **`.vsix`** siap pakai tersedia di folder `editors/vscode/`:

### Cara 1: Melalui Command Line (PowerShell / Terminal)
```powershell
code --install-extension editors/vscode/dapcode-lang-0.7.2.vsix
```

### Cara 2: Melalui Antarmuka VS Code
1. Buka Visual Studio Code.
2. Buka panel **Extensions** (`Ctrl+Shift+X`).
3. Klik ikon menu titik tiga (`...`) di pojok kanan atas panel Extensions.
4. Pilih **"Install from VSIX..."**.
5. Arahkan dan pilih berkas `dapcode-lang-0.7.2.vsix`.

---

## ✨ Fitur Utama

### 🎨 1. TextMate Syntax Highlighting Komprehensif
Pewarnaan sintaksis kaya dan estetis untuk seluruh elemen bahasa DapCode:
- **Kata Kunci Bahasa Indonesia:** `fungsi`, `struktur`, `sifat`, `terapkan`, `jika`, `lainnya`, `selama`, `untuk`, `di`, `impor`, `jalan_paralel`, `uji`, `kembali`, `tangkap`, `rusak`, `lanjut`.
- **Closure Lambda & Pemrograman Fungsional:** Penyorotan sintaksis pipa `|x| ...`, `|a, b| ...`, serta metode koleksi fungsional (`.petakan()`, `.petakan_nilai()`, `.saring()`, `.lipat()`, `.setiap()`).
- **Tipe Data Bawaan & Primitif:** `Teks`, `Daftar`, `Kamus`, `BasisData`, `PermintaanHttp`, `ResponsHttp`, `u64`, `i64`, `f64`, `Bool`, `Unit`.
- **Konstanta & Operator:** `benar`, `salah`, operator walrus `:=`, error handling union `? tangkap`, dan string interpolation `"{nama}"`.

---

### ⚡ 2. Language Server Protocol (LSP) Terintegrasi
Terhubung langsung secara *real-time* ke engine kompilator resmi `dapcodec lsp`:
- **Navigasi Go to Definition (`F12`):** Lompat seketika ke definisi simbol lokal (variabel, parameter), fungsi, struktur, sifat, hingga impor lintas berkas (`impor "modul.dap"`).
- **Penjelajah Hierarki Dokumen (Outline & Document Symbols):** Visualisasi struktur berkas lengkap mencakup struktur data, sifat, blok implementasi, fungsi, dan blok pengujian `uji`.
- **Diagnostik Galat Seketika:** Pelaporan kesalahan kompilasi tipe (*TypeMismatch*), sintaksis, dan pelacak kepemilikan (*borrow check* Perceus) langsung di baris kode saat mengetik.
- **Quick Fixes & Code Actions (`Ctrl+.`):**
  - Saran koreksi otomatis untuk salah ketik (*typo*) nama variabel atau fungsi.
  - Perbaikan otomatis operator penugasan `=` menjadi `:=`.
  - Generator otomatis stub fungsi yang belum terdefinisi (*auto-stub function*).
  - Generator otomatis fungsi titik masuk `fungsi utama() -> Unit` jika berkas belum memilikinya.
- **Hover Tooltips Interaktif:** Dokumentasi Markdown instan lengkap dengan tipe tanda tangan (*signature*) dan contoh penggunaan untuk semua kata kunci, tipe data, serta pustaka standar.
- **Auto-Completion Kontekstual:** Rekomendasi pintar nama kata kunci, metode koleksi, tipe data, dan simbol lokal saat mengetik.
- **Document Formatting:** Format otomatis kode sumber sesuai standar kerapian DapCode.

---

### 🐞 3. Debug Adapter Protocol (DAP) - Debugging Interaktif
Debugger grafis bawaan yang terintegrasi penuh dengan panel debugger VS Code:
- **Breakpoint Visual:** Pasang dan lepas titik henti (*breakpoint*) langsung pada gutter baris editor (`F9`).
- **Kontrol Eksekusi Penuh:**
  - `F5` / *Continue*: Jalankan eksekusi hingga breakpoint berikutnya.
  - `F10` / *Step Over*: Melompati satu baris pernyataan tanpa masuk ke fungsi.
  - `F11` / *Step In*: Masuk ke dalam fungsi yang sedang dipanggil.
- **Inspeksi Variabel Lokal:** Telusuri nilai dan tipe variabel lokal secara langsung pada panel **Variables** saat eksekusi berhenti di breakpoint.
- **Call Stack Navigator:** Lacak tumpukan panggilan fungsi dan closure secara mendalam.
- **Debug Console Evaluator:** Evaluasi variabel atau ekspresi secara dinamis melalui konsol debug.

---

### 📝 4. Koleksi Snippets Produktivitas Cepat
Cuplikan kode siap pakai untuk mempercepat pengembangan:
- **Dasar & Kontrol Alur:** `utama`, `fungsi`, `struktur`, `terapkan`, `jika`, `selama`, `untuk`, `tangkap`.
- **Lambda & Fungsional Koleksi:**
  - `closure`: `|item| item * 2`
  - `petakan`: `hasil := daftar.petakan(|x| x * 2)`
  - `petakan_nilai`: `hasil := kamus.petakan_nilai(|v| v * 2)`
  - `saring`: `hasil := daftar.saring(|x| x > 0)`
  - `kamus_saring`: `hasil := kamus.saring(|k, v| v > 0)`
  - `lipat`: `total := daftar.lipat(0, |acc, x| acc + x)`
  - `setiap`: `daftar.setiap(|x| cetak("{x}"))`
  - `kamus_setiap`: `kamus.setiap(|k, v| cetak("{k}: {v}"))`
- **Koleksi Asosiatif (Kamus):** `Kamus`, `pasang`, `ambil_atau`, `daftar_kunci`, `daftar_nilai`, `bersihkan`.
- **Basis Data Persisten:** `buka_db`, `db_simpan`, `db_ambil`.
- **HTTP REST API Server:** `server_http`, `respons_json`, `respons_teks`.
- **Real-Time WebSocket Server:** `buat_server_websocket`.
- **Mesin Reguler Ekspresi:** `cocok_regex`, `cari_regex`, `ganti_regex`, `ekstrak_regex`.
- **Kanvas Grafis 2D WebAssembly:** `kanvas_bersihkan`, `kanvas_warna_isi`, `kanvas_persegi`, `kanvas_lingkaran`, `kanvas_garis`, `kanvas_teks`.
- **Pengujian & Asersi:** `uji`, `pastikan`, `pastikan_sama`.

---

### 🚀 5. Menu Editor & Tombol Aksi Cepat
Tombol aksi langsung di pojok kanan atas editor dan menu klik kanan:
- ▶️ **DapCode: Jalankan Berkas Aktif (Run)** -> Menjalankan interpreter `dapcodec run`.
- 🧪 **DapCode: Jalankan Pengujian Unit (Test)** -> Menjalankan test runner terintegrasi `dapcodec test`.
- 💬 **DapCode: Buka Interactive REPL** -> Meluncurkan sesi interaktif `dapcodec repl`.
- 🎨 **DapCode: Buka Web Playground & Canvas Preview** -> Menjalankan server live compile WebAssembly & kanvas 2D lokal.
- 📖 **DapCode: Hasilkan Dokumentasi Proyek (Doc)** -> Membuat dokumentasi proyek HTML/Markdown otomatis.
- 🔍 **DapCode: Periksa Tipe & Kepemilikan (Check)** -> Menjalankan typecheck & analisis Perceus statis.
- ⚙️ **DapCode: Kompilasi ke Biner Mandiri (Build)** -> Menghasilkan executable mandiri `.exe` via GCC/Clang.
- 🔄 **DapCode: Mulai Ulang Language Server (LSP)** -> Me-restart background server bahasa.

---

## ⚙️ Pengaturan Ekstensi (Settings)

| Opsi | Default | Deskripsi |
| :--- | :--- | :--- |
| `dapcode.compilerPath` | `"dapcodec"` | Jalur path ke biner kompilator `dapcodec`. Otomatis mencari di sistem `PATH` atau direktori build lokal proyek jika tidak ditentukan. |
| `dapcode.enableLsp` | `true` | Mengaktifkan Language Server Protocol untuk diagnostik, hover, auto-completion, dan code action. |

---

## 📦 Lisensi

Dilisensikan di bawah lisensi resmi MIT License.
