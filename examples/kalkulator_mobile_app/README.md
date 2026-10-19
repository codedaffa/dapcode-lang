# 📱 Aplikasi Kalkulator Mobile DapCode (Struktur Proyek Produksi Penuh)

Aplikasi kalkulator mobile berkinerja tinggi yang dibangun dengan **Bahasa DapCode (.dap)** menggunakan arsitektur **Direct GPU Rendering 60 FPS** (Pendekatan ala Flutter & Kotlin Multiplatform).

---

## 🏛️ Struktur Direktori Proyek

Proyek ini mengadopsi struktur proyek aplikasi penuh berstandar industri dengan pemisahan tanggung jawab yang jelas:

```text
kalkulator_mobile_app/
├── DapCode.toml              # Manifes konfigurasi proyek DapCode
├── README.md                 # Dokumentasi proyek aplikasi
├── src/                      # Kode sumber aplikasi DapCode
│   ├── utama.dap             # Entry point, penanganan state, & siklus render 60 FPS
│   ├── ui/
│   │   └── komponen.dap      # Komponen UI layar sentuh (App Bar, Display, Keypad)
│   └── logika/
│       └── kalkulator.dap    # Logika bisnis murni (Aritmatika, Persen, Negasi, Backspace)
├── uji/                      # Pengujian unit otomatis (Automated Unit Tests)
│   └── kalkulator_uji.dap    # 8 pengujian otomatis logika matematika
├── aset/                     # Aset gambar, ikon aplikasi, font, dan audio
│   └── README.md
├── android/                  # Proyek Android Studio NDK CMake (OpenGL ES 2.0/3.0)
├── ios/                      # Proyek iOS Xcode Native (GLKit & Metal)
├── desktop/                  # Simulator layar ponsel desktop mandiri (C11 MinGW/GCC)
└── engine/                   # Source code C11 transpiled & DapCode GPU Canvas Engine
```

---

## ⚡ Alur Kerja Pengembangan

### 1. Menjalankan Simulator Ponsel Desktop (Live Hot-Reload)
Jalankan simulator layar sentuh 60 FPS langsung di komputer Anda tanpa memerlukan emulator Android:
```bash
dapcodec mobile preview
```
> **Catatan:** Mode pantau aktif (*Live-Watch*). Setiap perubahan kode di folder `src/` yang disimpan (Ctrl+S) akan langsung diperbarui ke simulator secara instan!

### 2. Menjalankan Pengujian Unit Otomatis (Unit Testing)
Jalankan seluruh rangkaian pengujian logika kalkulator yang terisolasi dari kanvas:
```bash
dapcodec test
```

### 3. Merakit ke Seluruh Target Platform (Android & iOS)
Untuk memperbarui dan merakit kode native C11 ke folder runner host:
```bash
dapcodec mobile build
```
- **Android:** Buka folder `android/` di Android Studio dan klik **Run** atau jalankan `./gradlew assembleDebug`.
- **iOS:** Buka folder `ios/` di Xcode pada macOS dan jalankan pada Simulator iOS atau perangkat fisik.
- **Desktop:** Buka folder `desktop/` dan klik ganda `kompilasi_dan_jalankan.bat`.
