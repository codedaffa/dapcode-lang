# DapCode Language Support untuk VS Code

Ekstensi resmi untuk bahasa pemrograman **DapCode (`.dap`)** yang menyediakan pengalaman pengembangan modern, cepat, dan nyaman di Visual Studio Code, Cursor, Windsurf, dan Antigravity IDE.

---

## ✨ Fitur Utama

- 🎨 **TextMate Syntax Highlighting Lengkap:** Pewarnaan sintaksis komprehensif untuk kata kunci bahasa Indonesia (`fungsi`, `struktur`, `sifat`, `terapkan`, `jika`, `selama`, `untuk`, `impor`, `jalan_paralel`, `uji`, dll), komentar dokumentasi tingkat modul (`//!`) dan deklarasi (`///`), interpolasi string `"{nama}"`, tipe data aljabar, operator walrus `:=`, dan konstanta boolean.
- ⚡ **Language Server Protocol (LSP) Real-Time:** Terhubung langsung ke kompilator resmi `dapcodec lsp`:
  - **Go to Definition (F12):** Navigasi lompat ke definisi simbol lokal (variabel lokal, parameter fungsi), deklarasi fungsi/struktur/sifat, hingga lintas berkas (`impor "modul.dap"`).
  - **Document Symbols & Outline Tree:** Penjelajah hierarki simbol berkas lengkap (struktur beserta bidang-bidangnya, sifat beserta tanda tangan metodenya, blok implementasi, fungsi, pengujian unit, dan modul impor).
  - **Diagnostik Kesalahan Seketika:** Pelaporan kesalahan kompilasi tipe (*TypeMismatch*), sintaks, dan kepemilikan (*borrow check*) langsung di baris kode saat mengetik.
  - **Hover Tooltips:** Dokumentasi penjelasan dan *signature* fungsi pustaka standar bawaan (`ke_json`, `cetak`, `baca_berkas`, `buat_direktori`, `ambil_argumen_cli`, `pastikan_sama`, dll) maupun deklarasi fungsi dan struktur lokal.
  - **Auto-Completion Cerdas:** Rekomendasi kata kunci, tipe data, fungsi pustaka standar, serta simbol kode lokal saat mengetik.
  - **Document Formatting:** Format kode sumber otomatis sesuai standar estetika DapCode.
- 📝 **Koleksi Snippets Lengkap:** Cuplikan kode cepat untuk membuat fungsi utama (`utama`), fungsi (`fungsi`), struktur (`struktur`), implementasi sifat (`terapkan`), unit test (`uji`), asersi (`pastikan`, `pastikan_sama`), komentar dokumen (`doc`, `docm`), filesystem (`buat_direktori`, `daftar_direktori`), argumen CLI (`ambil_argumen_cli`), konkurensi coroutine (`jalan_paralel`), penguraian JSON (`ke_json`), dan penanganan error (`tangkap`).
- 🚀 **Integrasi Perintah & Terminal:**
  - `DapCode: Jalankan Berkas Aktif (Run)` -> `dapcodec run`
  - `DapCode: Jalankan Pengujian Unit (Test)` -> `dapcodec test`
  - `DapCode: Buka Interactive REPL` -> `dapcodec repl`
  - `DapCode: Hasilkan Dokumentasi Proyek (Doc)` -> `dapcodec doc --open`
  - `DapCode: Periksa Tipe & Kepemilikan (Check)` -> `dapcodec check`
  - `DapCode: Kompilasi ke Biner Mandiri (Build)` -> `dapcodec build`
  - `DapCode: Mulai Ulang Language Server (LSP)`

---

## ⚙️ Pengaturan Ekstensi

| Opsi | Default | Deskripsi |
| :--- | :--- | :--- |
| `dapcode.compilerPath` | `"dapcodec"` | Jalur path ke executable kompilator `dapcodec`. Otomatis mencari di `PATH` dan target folder proyek jika tidak disetel. |
| `dapcode.enableLsp` | `true` | Mengaktifkan Language Server Protocol untuk diagnostik, hover, dan auto-completion. |

---

## 📦 Lisensi

Dilisensikan di bawah lisensi MIT.
