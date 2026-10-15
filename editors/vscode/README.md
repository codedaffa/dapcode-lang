# DapCode Language Support untuk VS Code

Ekstensi resmi untuk bahasa pemrograman **DapCode (`.dap`)** yang menyediakan pengalaman pengembangan modern, cepat, dan nyaman di Visual Studio Code, Cursor, Windsurf, dan Antigravity IDE.

---

## ✨ Fitur Utama

- 🎨 **TextMate Syntax Highlighting Lengkap:** Pewarnaan sintaksis komprehensif untuk kata kunci bahasa Indonesia (`fungsi`, `struktur`, `sifat`, `terapkan`, `jika`, `selama`, `untuk`, `impor`, `jalan_paralel`, dll), interpolasi string `"{nama}"`, tipe data aljabar, operator walrus `:=`, dan konstanta boolean.
- ⚡ **Language Server Protocol (LSP) Real-Time:** Terhubung langsung ke kompilator resmi `dapcodec lsp`:
  - **Diagnostik Kesalahan Seketika:** Pelaporan kesalahan kompilasi tipe (*TypeMismatch*), sintaks, dan kepemilikan (*borrow check*) langsung di baris kode saat mengetik.
  - **Hover Tooltips:** Dokumentasi penjelasan dan *signature* fungsi bawaan (`ke_json`, `cetak`, `baca_berkas`, dll) maupun deklarasi fungsi dan struktur lokal.
  - **Auto-Completion Cerdas:** Rekomendasi kata kunci, tipe data, pustaka bawaan, dan simbol kode lokal saat mengetik.
  - **Document Formatting:** Format kode sumber otomatis sesuai standar estetika DapCode.
- 📝 **Koleksi Snippets Lengkap:** Cuplikan kode cepat untuk membuat fungsi utama (`utama`), fungsi (`fungsi`), struktur (`struktur`), implementasi sifat (`terapkan`), konkurensi coroutine (`jalan_paralel`), penguraian JSON (`ke_json`), dan penanganan error (`tangkap`).
- 🚀 **Integrasi Perintah & Terminal:**
  - `DapCode: Jalankan Berkas Aktif (Run)` -> `dapcodec run`
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
