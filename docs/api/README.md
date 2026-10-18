# 📚 Dokumentasi Proyek: Proyek DapCode

**Versi:** `0.1.0`  
**Deskripsi:** Dokumentasi kode sumber DapCode

---

## 📑 Daftar Isi

- [Modul: `demo`](#modul-demo) 
  - [struktur: `Produk`](#struktur-produk) 
  - [sifat: `BisaDidiskon`](#sifat-bisadidiskon) 
  - [implementasi: `terapkan Produk`](#implementasi-terapkan-produk) 
  - [fungsi: `cari_produk`](#fungsi-cari_produk) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_audio`](#modul-demo_audio) 
  - [fungsi: `mainkan_melodi_arpegio`](#fungsi-mainkan_melodi_arpegio) 
  - [fungsi: `demo_bentuk_gelombang`](#fungsi-demo_bentuk_gelombang) 
  - [fungsi: `demo_efek_suara_game`](#fungsi-demo_efek_suara_game) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_closure`](#modul-demo_closure) 
  - [fungsi: `demo_closure_dasar`](#fungsi-demo_closure_dasar) 
  - [fungsi: `demo_transformasi_koleksi`](#fungsi-demo_transformasi_koleksi) 
  - [fungsi: `demo_lipat_dan_chaining`](#fungsi-demo_lipat_dan_chaining) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_input_output`](#modul-demo_input_output) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_kamus`](#modul-demo_kamus) 
  - [fungsi: `demo_kamus_dasar`](#fungsi-demo_kamus_dasar) 
  - [fungsi: `demo_kamus_json_dan_kunci`](#fungsi-demo_kamus_json_dan_kunci) 
  - [fungsi: `demo_kamus_penghapusan`](#fungsi-demo_kamus_penghapusan) 
  - [fungsi: `demo_kamus_fungsional`](#fungsi-demo_kamus_fungsional) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_kanvas`](#modul-demo_kanvas) 
  - [fungsi: `gambar_tata_surya`](#fungsi-gambar_tata_surya) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_koleksi_teks`](#modul-demo_koleksi_teks) 
  - [fungsi: `demo_metode_teks`](#fungsi-demo_metode_teks) 
  - [fungsi: `demo_metode_daftar`](#fungsi-demo_metode_daftar) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `demo_uji`](#modul-demo_uji) 
  - [fungsi: `kali`](#fungsi-kali) 
  - [fungsi: `bagi`](#fungsi-bagi) 
  - [uji: `perhitungan perkalian dasar`](#uji-perhitungan-perkalian-dasar) 
  - [uji: `pembagian normal dan error union`](#uji-pembagian-normal-dan-error-union) 
  - [uji: `validasi string dan boolean`](#uji-validasi-string-dan-boolean) 
- [Modul: `demo_waktu`](#modul-demo_waktu) 
  - [fungsi: `cetak_garis`](#fungsi-cetak_garis) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `geometri`](#modul-geometri) 
  - [struktur: `Titik`](#struktur-titik) 
  - [struktur: `Lingkaran`](#struktur-lingkaran) 
  - [fungsi: `buat_titik`](#fungsi-buat_titik) 
  - [fungsi: `buat_lingkaran`](#fungsi-buat_lingkaran) 
  - [fungsi: `luas_lingkaran`](#fungsi-luas_lingkaran) 
  - [fungsi: `keliling_lingkaran`](#fungsi-keliling_lingkaran) 
- [Modul: `matematika`](#modul-matematika) 
  - [fungsi: `pangkat_tiga`](#fungsi-pangkat_tiga) 
  - [fungsi: `jumlah_deret`](#fungsi-jumlah_deret) 
- [Modul: `utama`](#modul-utama) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `tes`](#modul-tes) 
  - [fungsi: `sapaan`](#fungsi-sapaan) 
  - [fungsi: `utama`](#fungsi-utama) 
  - [uji: `hasil cetak`](#uji-hasil-cetak) 
- [Modul: `uji_basis_data`](#modul-uji_basis_data) 
  - [fungsi: `demo_database_dasar`](#fungsi-demo_database_dasar) 
  - [fungsi: `demo_transaksi_acid`](#fungsi-demo_transaksi_acid) 
  - [fungsi: `demo_prefix_dan_json`](#fungsi-demo_prefix_dan_json) 
  - [fungsi: `tangani_api_db`](#fungsi-tangani_api_db) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `uji_matematika`](#modul-uji_matematika) 
  - [fungsi: `demo_trigonometri`](#fungsi-demo_trigonometri) 
  - [fungsi: `demo_geometri_vektor`](#fungsi-demo_geometri_vektor) 
  - [fungsi: `demo_logaritma_dan_eksponensial`](#fungsi-demo_logaritma_dan_eksponensial) 
  - [fungsi: `demo_pembulatan_dan_statistik`](#fungsi-demo_pembulatan_dan_statistik) 
  - [fungsi: `utama`](#fungsi-utama) 
  - [uji: `Trigonometri identitas dasar`](#uji-trigonometri-identitas-dasar) 
  - [uji: `Kalkulasi akar dan pangkat`](#uji-kalkulasi-akar-dan-pangkat) 
  - [uji: `Pembulatan dan ekstremum`](#uji-pembulatan-dan-ekstremum) 
- [Modul: `uji_regex`](#modul-uji_regex) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `uji_router`](#modul-uji_router) 
  - [fungsi: `tangani_web`](#fungsi-tangani_web) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `uji_server_http`](#modul-uji_server_http) 
  - [fungsi: `tangani_permintaan`](#fungsi-tangani_permintaan) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `uji_statis`](#modul-uji_statis) 
  - [fungsi: `tangani_pesan_ws`](#fungsi-tangani_pesan_ws) 
  - [fungsi: `tangani_permintaan`](#fungsi-tangani_permintaan) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `uji_stdlib`](#modul-uji_stdlib) 
  - [fungsi: `demonstrasi_koleksi`](#fungsi-demonstrasi_koleksi) 
  - [fungsi: `demonstrasi_string_dan_matematika`](#fungsi-demonstrasi_string_dan_matematika) 
  - [fungsi: `demonstrasi_berkas_dan_waktu`](#fungsi-demonstrasi_berkas_dan_waktu) 
  - [struktur: `AkunPengguna`](#struktur-akunpengguna) 
  - [fungsi: `demonstrasi_json`](#fungsi-demonstrasi_json) 
  - [fungsi: `demonstrasi_jaringan`](#fungsi-demonstrasi_jaringan) 
  - [fungsi: `demonstrasi_lingkungan_dan_cli`](#fungsi-demonstrasi_lingkungan_dan_cli) 
  - [fungsi: `demonstrasi_kriptografi_dan_enkoding`](#fungsi-demonstrasi_kriptografi_dan_enkoding) 
  - [fungsi: `utama`](#fungsi-utama) 
- [Modul: `uji_wasm`](#modul-uji_wasm) 
  - [fungsi: `faktorial`](#fungsi-faktorial) 
  - [fungsi: `luas_lingkaran`](#fungsi-luas_lingkaran) 
  - [fungsi: `hitung_deret`](#fungsi-hitung_deret) 
  - [fungsi: `utama`](#fungsi-utama) 

---

## 📦 Modul `demo`

**Berkas:** `.\examples\demo.dap`  

### ⚡ Fungsi

#### `fungsi cari_produk`

```dap
fungsi cari_produk(id: u64) -> Produk ! Teks
```

**Parameter:**
- `id`: `u64`

**Nilai Kembalian:** `Produk ! Teks`

#### `fungsi utama`

```dap
fungsi utama()
```

### 🏗️ Struktur Data

#### `struktur Produk`

```dap
struktur Produk {
    id: u64,
    nama: Teks,
    harga: f64
}
```

| Bidang (*Field*) | Tipe Data |
| :--- | :--- |
| `id` | `u64` |
| `nama` | `Teks` |
| `harga` | `f64` |

### 📐 Sifat (Trait)

#### `sifat BisaDidiskon`

**Metode Kontrak:**
- `fungsi hitung_diskon(ini, persen: f64) -> f64`

### 🧩 Implementasi Sifat / Metode

#### `terapkan BisaDidiskon untuk Produk`

- `fungsi hitung_diskon(ini, persen: f64) -> f64`

---

## 📦 Modul `demo_audio`

**Berkas:** `.\examples\demo_audio.dap`  

### ⚡ Fungsi

#### `fungsi mainkan_melodi_arpegio`

```dap
fungsi mainkan_melodi_arpegio() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_bentuk_gelombang`

```dap
fungsi demo_bentuk_gelombang() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_efek_suara_game`

```dap
fungsi demo_efek_suara_game() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi utama`

```dap
fungsi utama() -> u64
```

**Nilai Kembalian:** `u64`

---

## 📦 Modul `demo_closure`

**Berkas:** `.\examples\demo_closure.dap`  

### ⚡ Fungsi

#### `fungsi demo_closure_dasar`

```dap
fungsi demo_closure_dasar() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_transformasi_koleksi`

```dap
fungsi demo_transformasi_koleksi() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_lipat_dan_chaining`

```dap
fungsi demo_lipat_dan_chaining() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi utama`

```dap
fungsi utama() -> u64
```

**Nilai Kembalian:** `u64`

---

## 📦 Modul `demo_input_output`

**Berkas:** `.\examples\demo_input_output.dap`  

### ⚡ Fungsi

#### `fungsi utama`

```dap
fungsi utama()
```

========================================================
Program Demo Resmi: Input & Output (I/O) DapCode
Mendemonstrasikan I/O Terminal, I/O Berkas, & I/O Sistem
========================================================

---

## 📦 Modul `demo_kamus`

**Berkas:** `.\examples\demo_kamus.dap`  

### ⚡ Fungsi

#### `fungsi demo_kamus_dasar`

```dap
fungsi demo_kamus_dasar() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_kamus_json_dan_kunci`

```dap
fungsi demo_kamus_json_dan_kunci() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_kamus_penghapusan`

```dap
fungsi demo_kamus_penghapusan() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_kamus_fungsional`

```dap
fungsi demo_kamus_fungsional() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi utama`

```dap
fungsi utama() -> Unit
```

**Nilai Kembalian:** `Unit`

---

## 📦 Modul `demo_kanvas`

**Berkas:** `.\examples\demo_kanvas.dap`  

### ⚡ Fungsi

#### `fungsi gambar_tata_surya`

```dap
fungsi gambar_tata_surya() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi utama`

```dap
fungsi utama() -> u64
```

**Nilai Kembalian:** `u64`

---

## 📦 Modul `demo_koleksi_teks`

**Berkas:** `.\examples\demo_koleksi_teks.dap`  

### ⚡ Fungsi

#### `fungsi demo_metode_teks`

```dap
fungsi demo_metode_teks() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi demo_metode_daftar`

```dap
fungsi demo_metode_daftar() -> Unit
```

**Nilai Kembalian:** `Unit`

#### `fungsi utama`

```dap
fungsi utama() -> u64
```

**Nilai Kembalian:** `u64`

---

## 📦 Modul `demo_uji`

**Berkas:** `.\examples\demo_uji.dap`  

### ⚡ Fungsi

#### `fungsi kali`

```dap
fungsi kali(a: bulat, b: bulat) -> bulat
```

**Parameter:**
- `a`: `bulat`
- `b`: `bulat`

**Nilai Kembalian:** `bulat`

#### `fungsi bagi`

```dap
fungsi bagi(a: bulat, b: bulat) -> bulat ! teks
```

**Parameter:**
- `a`: `bulat`
- `b`: `bulat`

**Nilai Kembalian:** `bulat ! teks`

### 🧪 Pengujian Unit (Test Cases)

- `uji "perhitungan perkalian dasar"` (baris 12)
- `uji "pembagian normal dan error union"` (baris 19)
- `uji "validasi string dan boolean"` (baris 26)

---

## 📦 Modul `demo_waktu`

**Berkas:** `.\examples\demo_waktu.dap`  

### ⚡ Fungsi

#### `fungsi cetak_garis`

```dap
fungsi cetak_garis()
```

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `geometri`

**Berkas:** `.\examples\proyek_multiberkas\src\geometri.dap`  

### ⚡ Fungsi

#### `fungsi buat_titik`

```dap
fungsi buat_titik(x: desimal, y: desimal) -> Titik
```

**Parameter:**
- `x`: `desimal`
- `y`: `desimal`

**Nilai Kembalian:** `Titik`

#### `fungsi buat_lingkaran`

```dap
fungsi buat_lingkaran(px: desimal, py: desimal, r: desimal) -> Lingkaran
```

**Parameter:**
- `px`: `desimal`
- `py`: `desimal`
- `r`: `desimal`

**Nilai Kembalian:** `Lingkaran`

#### `fungsi luas_lingkaran`

```dap
fungsi luas_lingkaran(l: Lingkaran) -> desimal
```

**Parameter:**
- `l`: `Lingkaran`

**Nilai Kembalian:** `desimal`

#### `fungsi keliling_lingkaran`

```dap
fungsi keliling_lingkaran(l: Lingkaran) -> desimal
```

**Parameter:**
- `l`: `Lingkaran`

**Nilai Kembalian:** `desimal`

### 🏗️ Struktur Data

#### `struktur Titik`

```dap
struktur Titik {
    x: desimal,
    y: desimal
}
```

| Bidang (*Field*) | Tipe Data |
| :--- | :--- |
| `x` | `desimal` |
| `y` | `desimal` |

#### `struktur Lingkaran`

```dap
struktur Lingkaran {
    pusat: Titik,
    radius: desimal
}
```

| Bidang (*Field*) | Tipe Data |
| :--- | :--- |
| `pusat` | `Titik` |
| `radius` | `desimal` |

---

## 📦 Modul `matematika`

**Berkas:** `.\examples\proyek_multiberkas\src\matematika.dap`  

### ⚡ Fungsi

#### `fungsi pangkat_tiga`

```dap
fungsi pangkat_tiga(n: u64) -> u64
```

**Parameter:**
- `n`: `u64`

**Nilai Kembalian:** `u64`

#### `fungsi jumlah_deret`

```dap
fungsi jumlah_deret() -> u64
```

**Nilai Kembalian:** `u64`

---

## 📦 Modul `utama`

**Berkas:** `.\examples\proyek_multiberkas\src\utama.dap`  

### ⚡ Fungsi

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `tes`

**Berkas:** `.\examples\tes.dap`  

### ⚡ Fungsi

#### `fungsi sapaan`

```dap
fungsi sapaan() -> Teks
```

**Nilai Kembalian:** `Teks`

#### `fungsi utama`

```dap
fungsi utama()
```

### 🧪 Pengujian Unit (Test Cases)

- `uji "hasil cetak"` (baris 9)

---

## 📦 Modul `uji_basis_data`

**Berkas:** `.\examples\uji_basis_data.dap`  

### ⚡ Fungsi

#### `fungsi demo_database_dasar`

```dap
fungsi demo_database_dasar()
```

Contoh Basis Data Mandiri & Persistensi Data REST API Bawaan DapCode
Menunjukkan penggunaan BasisData terstruktur lokal dengan Transaksi Atomik (ACID),
Pencarian Awalan (Prefix Query), serta Ekspor & Impor JSON tanpa dependensi eksternal.

#### `fungsi demo_transaksi_acid`

```dap
fungsi demo_transaksi_acid()
```

#### `fungsi demo_prefix_dan_json`

```dap
fungsi demo_prefix_dan_json()
```

#### `fungsi tangani_api_db`

```dap
fungsi tangani_api_db(req: PermintaanHttp) -> ResponsHttp
```

**Parameter:**
- `req`: `PermintaanHttp`

**Nilai Kembalian:** `ResponsHttp`

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `uji_matematika`

**Berkas:** `.\examples\uji_matematika.dap`  

### ⚡ Fungsi

#### `fungsi demo_trigonometri`

```dap
fungsi demo_trigonometri()
```

#### `fungsi demo_geometri_vektor`

```dap
fungsi demo_geometri_vektor()
```

#### `fungsi demo_logaritma_dan_eksponensial`

```dap
fungsi demo_logaritma_dan_eksponensial()
```

#### `fungsi demo_pembulatan_dan_statistik`

```dap
fungsi demo_pembulatan_dan_statistik()
```

#### `fungsi utama`

```dap
fungsi utama()
```

### 🧪 Pengujian Unit (Test Cases)

- `uji "Trigonometri identitas dasar"` (baris 129)
- `uji "Kalkulasi akar dan pangkat"` (baris 139)
- `uji "Pembulatan dan ekstremum"` (baris 148)

---

## 📦 Modul `uji_regex`

**Berkas:** `.\examples\uji_regex.dap`  

### ⚡ Fungsi

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `uji_router`

**Berkas:** `.\examples\uji_router.dap`  

### ⚡ Fungsi

#### `fungsi tangani_web`

```dap
fungsi tangani_web(req: PermintaanHttp) -> ResponsHttp
```

**Parameter:**
- `req`: `PermintaanHttp`

**Nilai Kembalian:** `ResponsHttp`

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `uji_server_http`

**Berkas:** `.\examples\uji_server_http.dap`  

### ⚡ Fungsi

#### `fungsi tangani_permintaan`

```dap
fungsi tangani_permintaan(req: PermintaanHttp) -> ResponsHttp
```

Contoh Server Web HTTP & Backend REST API Bawaan DapCode
Menjalankan server web mikroservis tanpa dependensi pustaka luar.

**Parameter:**
- `req`: `PermintaanHttp`

**Nilai Kembalian:** `ResponsHttp`

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `uji_statis`

**Berkas:** `.\examples\uji_statis.dap`  

### ⚡ Fungsi

#### `fungsi tangani_pesan_ws`

```dap
fungsi tangani_pesan_ws(pesan: Teks) -> Teks
```

**Parameter:**
- `pesan`: `Teks`

**Nilai Kembalian:** `Teks`

#### `fungsi tangani_permintaan`

```dap
fungsi tangani_permintaan(req: PermintaanHttp) -> ResponsHttp
```

**Parameter:**
- `req`: `PermintaanHttp`

**Nilai Kembalian:** `ResponsHttp`

#### `fungsi utama`

```dap
fungsi utama()
```

---

## 📦 Modul `uji_stdlib`

**Berkas:** `.\examples\uji_stdlib.dap`  

### ⚡ Fungsi

#### `fungsi demonstrasi_koleksi`

```dap
fungsi demonstrasi_koleksi()
```

#### `fungsi demonstrasi_string_dan_matematika`

```dap
fungsi demonstrasi_string_dan_matematika()
```

#### `fungsi demonstrasi_berkas_dan_waktu`

```dap
fungsi demonstrasi_berkas_dan_waktu()
```

#### `fungsi demonstrasi_json`

```dap
fungsi demonstrasi_json()
```

#### `fungsi demonstrasi_jaringan`

```dap
fungsi demonstrasi_jaringan()
```

#### `fungsi demonstrasi_lingkungan_dan_cli`

```dap
fungsi demonstrasi_lingkungan_dan_cli()
```

#### `fungsi demonstrasi_kriptografi_dan_enkoding`

```dap
fungsi demonstrasi_kriptografi_dan_enkoding()
```

#### `fungsi utama`

```dap
fungsi utama()
```

### 🏗️ Struktur Data

#### `struktur AkunPengguna`

```dap
struktur AkunPengguna {
    id: u64,
    nama: Teks,
    saldo: f64,
    aktif: Bool
}
```

| Bidang (*Field*) | Tipe Data |
| :--- | :--- |
| `id` | `u64` |
| `nama` | `Teks` |
| `saldo` | `f64` |
| `aktif` | `Bool` |

---

## 📦 Modul `uji_wasm`

**Berkas:** `.\examples\uji_wasm.dap`  

### ⚡ Fungsi

#### `fungsi faktorial`

```dap
fungsi faktorial(n: u64) -> u64
```

**Parameter:**
- `n`: `u64`

**Nilai Kembalian:** `u64`

#### `fungsi luas_lingkaran`

```dap
fungsi luas_lingkaran(r: f64) -> f64
```

**Parameter:**
- `r`: `f64`

**Nilai Kembalian:** `f64`

#### `fungsi hitung_deret`

```dap
fungsi hitung_deret(batas: u64) -> u64
```

**Parameter:**
- `batas`: `u64`

**Nilai Kembalian:** `u64`

#### `fungsi utama`

```dap
fungsi utama() -> u64
```

**Nilai Kembalian:** `u64`

---

Dokumentasi ini dibuat secara otomatis oleh kompilator **`dapcodec doc`**.
