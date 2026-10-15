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
- [Modul: `demo_uji`](#modul-demo_uji) 
  - [fungsi: `kali`](#fungsi-kali) 
  - [fungsi: `bagi`](#fungsi-bagi) 
  - [uji: `perhitungan perkalian dasar`](#uji-perhitungan-perkalian-dasar) 
  - [uji: `pembagian normal dan error union`](#uji-pembagian-normal-dan-error-union) 
  - [uji: `validasi string dan boolean`](#uji-validasi-string-dan-boolean) 
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

Dokumentasi ini dibuat secara otomatis oleh kompilator **`dapcodec doc`**.
