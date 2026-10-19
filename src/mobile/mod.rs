//! Modul Mobile Native Direct GPU untuk DapCode (Arsitektur ala Flutter)
//!
//! Mengimplementasikan arsitektur kompilasi native bare-metal:
//! - Direct GPU Canvas Renderer (OpenGL ES 2.0 / 3.0 & C11)
//! - 100% Native tanpa ketergantungan WebView, DOM HTML, atau PWA
//! - Performa tinggi 60-120 FPS dengan latensi sentuhan (touch latency) 0ms
//! - Proyek Android NDK (CMake + GLSurfaceView)
//! - Proyek iOS Native (Xcode + EAGL / Metal C-Bridge)
//! - Lingkungan Pratinjau Desktop Langsung dengan Live Hot-Reload

use std::fs;
use std::io;
use std::path::Path;
use colored::*;

pub mod gpu_engine;
pub use gpu_engine::{
    build_android_native_gpu_project, build_desktop_gpu_preview, build_ios_native_gpu_project,
    build_native_gpu_bundle, get_dapcode_engine_c, get_dapcode_engine_h, run_desktop_gpu_preview,
};

/// Menginisialisasi proyek baru dengan template aplikasi mobile (Struktur Proyek Produksi Penuh)
pub fn cmd_mobile_init(name: Option<&str>) -> io::Result<()> {
    let app_name = name.unwrap_or("app_mobile");
    let dir = Path::new(app_name);

    let src_dir = dir.join("src");
    let ui_dir = src_dir.join("ui");
    let logika_dir = src_dir.join("logika");
    let uji_dir = dir.join("uji");
    let aset_dir = dir.join("aset");

    fs::create_dir_all(&ui_dir)?;
    fs::create_dir_all(&logika_dir)?;
    fs::create_dir_all(&uji_dir)?;
    fs::create_dir_all(&aset_dir)?;

    // 1. DapCode.toml
    let toml_content = format!(
        r#"[paket]
nama = "{}"
versi = "0.1.0"
penulis = ["Pengembang DapCode"]
berkas_utama = "src/utama.dap"
deskripsi = "Aplikasi Mobile Native Direct GPU (Flutter-style) dengan DapCode"

[target]
tipe = "mobile"
engine = "flutter"
"#,
        app_name
    );
    fs::write(dir.join("DapCode.toml"), toml_content)?;

    // 2. src/logika/kalkulator.dap (Logika Bisnis Murni)
    let logika_code = r#"// ============================================================================
// 🧮 Logika Bisnis Murni Kalkulator DapCode
// Mengimplementasikan operasi aritmatika tanpa ketergantungan UI/Kanvas
// ============================================================================

fungsi hitung_aritmatika(n1: f64, op: Teks, n2: f64) -> f64 {
    jika op == "+" {
        kembali n1 + n2
    } selain_itu jika op == "-" {
        kembali n1 - n2
    } selain_itu jika op == "*" {
        kembali n1 * n2
    } selain_itu jika op == "/" {
        jika n2 != 0.0 {
            kembali n1 / n2
        } selain_itu {
            kembali 0.0
        }
    }
    kembali n2
}

fungsi hitung_persen(n: f64) -> f64 {
    kembali n / 100.0
}

fungsi hitung_negasi(n: f64) -> f64 {
    kembali n * -1.0
}

fungsi potong_digit_terakhir(n: f64) -> f64 {
    kembali lantai(n / 10.0)
}
"#;
    fs::write(logika_dir.join("kalkulator.dap"), logika_code)?;

    // 3. src/ui/komponen.dap (Komponen Layar Sentuh & Kanvas GPU)
    let ui_code = r#"// ============================================================================
// 📱 Komponen Antarmuka Layar Sentuh & GPU
// ============================================================================

fungsi gambar_status_bar(w: f64) -> Unit {
    kanvas_warna_isi(0.04, 0.06, 0.10, 0.95)
    kanvas_persegi(0.0, 0.0, w, 24.0)
    
    // Teks Jam
    kanvas_warna_isi(0.92, 0.95, 1.0, 0.95)
    kanvas_teks("09:41", 14.0, 16.0, 10.5)
    
    // Ikon Sinyal Seluler (4 Bar Bertingkat)
    kanvas_warna_isi(0.20, 0.85, 0.50, 0.95)
    kanvas_persegi_bulat(w - 62.0, 12.0, 2.5, 5.0, 1.0)
    kanvas_persegi_bulat(w - 56.0, 10.0, 2.5, 7.0, 1.0)
    kanvas_persegi_bulat(w - 50.0, 8.0, 2.5, 9.0, 1.0)
    kanvas_persegi_bulat(w - 44.0, 6.0, 2.5, 11.0, 1.0)
    
    // Ikon Kapsul Baterai
    kanvas_warna_garis(0.90, 0.95, 1.0, 0.75)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_persegi_bulat(w - 32.0, 6.0, 18.0, 10.0, 3.0)
    kanvas_warna_isi(0.20, 0.88, 0.55, 0.95)
    kanvas_persegi_bulat(w - 30.0, 8.0, 12.0, 6.0, 2.0)
    kanvas_persegi(w - 14.0, 9.0, 1.5, 4.0)
}

fungsi gambar_app_bar(w: f64, judul: Teks) -> Unit {
    kanvas_warna_isi(0.06, 0.09, 0.16, 0.98)
    kanvas_persegi(0.0, 24.0, w, 54.0)
    
    // Garis Aksen Bawah App Bar
    kanvas_warna_isi(0.12, 0.45, 0.85, 0.8)
    kanvas_persegi(0.0, 77.0, w, 1.5)
    
    // Judul Aplikasi
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks(judul, 14.0, 48.0, 15.0)
    
    // Badge Status Pill
    kanvas_warna_isi(0.06, 0.28, 0.58, 0.45)
    kanvas_persegi_bulat(14.0, 56.0, 156.0, 14.0, 7.0)
    kanvas_warna_garis(0.25, 0.65, 1.0, 0.7)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_persegi_bulat(14.0, 56.0, 156.0, 14.0, 7.0)
    
    kanvas_warna_isi(0.45, 0.90, 1.0, 1.0)
    kanvas_teks_tengah("DIRECT GPU • 60 FPS NATIVE", 14.0 + 78.0, 63.0, 8.0)
}

fungsi gambar_layar_tampilan(w: f64, sy: f64, sh: f64, riwayat: Teks, tampilan: Teks) -> Unit {
    sx := 14.0
    sw := w - 28.0

    // Kotak Display Glassmorphic Sudut Membulat
    kanvas_warna_isi(0.07, 0.11, 0.19, 0.95)
    kanvas_persegi_bulat(sx, sy, sw, sh, 16.0)

    kanvas_warna_garis(0.20, 0.36, 0.62, 0.6)
    kanvas_tebal_garis(1.2)
    kanvas_bingkai_persegi_bulat(sx, sy, sw, sh, 16.0)

    // Indikator Titik Status Hijau
    kanvas_warna_isi(0.15, 0.88, 0.45, 0.9)
    kanvas_lingkaran(sx + 16.0, sy + 18.0, 3.5)
    kanvas_warna_isi(0.55, 0.75, 0.95, 0.85)
    kanvas_teks("Display Aktif", sx + 26.0, sy + 21.0, 9.0)

    // Riwayat Operasi Sebelumnya (Baris Atas)
    kanvas_warna_isi(0.55, 0.72, 0.92, 0.85)
    kanvas_teks(riwayat, sx + 16.0, sy + sh * 0.48, 12.0)

    // Angka Utama / Hasil (Baris Bawah Besar)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks(tampilan, sx + 16.0, sy + sh * 0.84, 20.0)
}

fungsi gambar_tombol(label: Teks, bx: f64, by: f64, bw: f64, bh: f64, jenis: u64, mx: f64, my: f64, klik: u64) -> Unit {
    hover := (mx >= bx && mx <= bx + bw && my >= by && my <= by + bh)
    ditekan := hover && (klik == 1)
    radius := bh * 0.28
    jika radius > 16.0 { radius = 16.0 }

    // Pemilihan warna latar belakang tombol (Gaya Elegan iOS / Flutter)
    jika jenis == 3 {
        // Tombol Sama Dengan (=): Aksen Emerald Hijau Modern
        jika ditekan {
            kanvas_warna_isi(0.18, 0.90, 0.58, 1.0)
        } selain_itu jika hover {
            kanvas_warna_isi(0.12, 0.78, 0.48, 1.0)
        } selain_itu {
            kanvas_warna_isi(0.06, 0.68, 0.40, 0.95)
        }
    } selain_itu jika jenis == 2 {
        // Tombol Operator (+, -, *, /): Aksen Amber / Oranye Dinamis
        jika ditekan {
            kanvas_warna_isi(1.0, 0.72, 0.25, 1.0)
        } selain_itu jika hover {
            kanvas_warna_isi(0.98, 0.60, 0.14, 1.0)
        } selain_itu {
            kanvas_warna_isi(0.92, 0.50, 0.08, 0.95)
        }
    } selain_itu jika jenis == 1 {
        // Tombol Fungsi (C, +/-, %, <): Slate Gray Modern
        jika ditekan {
            kanvas_warna_isi(0.40, 0.46, 0.58, 1.0)
        } selain_itu jika hover {
            kanvas_warna_isi(0.30, 0.36, 0.48, 1.0)
        } selain_itu {
            kanvas_warna_isi(0.22, 0.27, 0.38, 0.95)
        }
    } selain_itu {
        // Tombol Angka (0-9, .): Dark Obsidian Mewah
        jika ditekan {
            kanvas_warna_isi(0.28, 0.35, 0.48, 1.0)
        } selain_itu jika hover {
            kanvas_warna_isi(0.19, 0.25, 0.36, 1.0)
        } selain_itu {
            kanvas_warna_isi(0.12, 0.16, 0.25, 0.95)
        }
    }

    // Gambar Persegi Sudut Membulat (Squircle)
    kanvas_persegi_bulat(bx, by, bw, bh, radius)

    // Garis Outline / Border Halus
    jika jenis == 3 {
        kanvas_warna_garis(0.30, 0.90, 0.60, 0.6)
    } selain_itu jika jenis == 2 {
        kanvas_warna_garis(1.0, 0.75, 0.35, 0.55)
    } selain_itu jika jenis == 1 {
        kanvas_warna_garis(0.40, 0.50, 0.65, 0.45)
    } selain_itu {
        kanvas_warna_garis(0.24, 0.32, 0.46, 0.40)
    }
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_persegi_bulat(bx, by, bw, bh, radius)

    // Label Teks Terpusat Sempurna (Presisi Tengah Horizontal & Vertikal)
    jika jenis == 1 {
        kanvas_warna_isi(0.50, 0.90, 1.0, 1.0)
    } selain_itu {
        kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    }

    font_sz := 16.0
    jika label == "+/-" || label == "DEL" {
        font_sz = 13.0
    }
    kanvas_teks_tengah(label, bx + bw / 2.0, by + bh / 2.0, font_sz)
}

fungsi tombol_ditekan(bx: f64, by: f64, bw: f64, bh: f64, mx: f64, my: f64, baru_klik: Bool) -> Bool {
    jika baru_klik {
        jika mx >= bx && mx <= bx + bw && my >= by && my <= by + bh {
            kembali benar
        }
    }
    kembali salah
}
"#;
    fs::write(ui_dir.join("komponen.dap"), ui_code)?;

    // 4. src/utama.dap (Entry Point & Render Loop)
    let utama_code = r#"// ============================================================================
// 📱 Aplikasi Mobile DapCode (Entry Point & Controller)
// Arsitektur Produksi Multi-File: logika murni, komponen UI, & siklus render
// ============================================================================

impor "logika/kalkulator.dap"
impor "ui/komponen.dap"

// ----------------------------------------------------------------------------
// Pengendali Aksi State (Action Handlers)
// ----------------------------------------------------------------------------

fungsi input_angka(digit: Teks) -> Unit {
    cur := kanvas_ambil_teks("tampilan")
    reset := kanvas_ambil_u64("reset")

    jika reset == 1 || cur == "0" {
        kanvas_simpan_teks("tampilan", digit)
        kanvas_simpan_u64("reset", 0)
    } selain_itu {
        baru := "{cur}{digit}"
        kanvas_simpan_teks("tampilan", baru)
    }
}

fungsi input_operator(op: Teks) -> Unit {
    cur := kanvas_ambil_teks("tampilan")
    num := ke_desimal(cur)

    kanvas_simpan_angka("angka1", num)
    kanvas_simpan_teks("op", op)
    kanvas_simpan_teks("riwayat", "{cur} {op}")
    kanvas_simpan_u64("reset", 1)
}

fungsi hitung_hasil() -> Unit {
    op := kanvas_ambil_teks("op")
    jika op == "" {
        kembali
    }

    n1 := kanvas_ambil_angka("angka1")
    cur_str := kanvas_ambil_teks("tampilan")
    n2 := ke_desimal(cur_str)

    // Validasi pembagian dengan nol
    jika op == "/" && n2 == 0.0 {
        kanvas_simpan_teks("tampilan", "Error (Bagi 0)")
        kanvas_simpan_teks("riwayat", "")
        kanvas_simpan_teks("op", "")
        kanvas_simpan_u64("reset", 1)
        kembali
    }

    // Panggil logika bisnis murni dari modul kalkulator
    hasil := hitung_aritmatika(n1, op, n2)
    hasil_str := format_desimal(hasil)

    kanvas_simpan_teks("tampilan", hasil_str)
    kanvas_simpan_teks("riwayat", "{n1} {op} {n2} =")
    kanvas_simpan_teks("op", "")
    kanvas_simpan_u64("reset", 1)
}

fungsi bersihkan_kalkulator() -> Unit {
    kanvas_simpan_teks("tampilan", "0")
    kanvas_simpan_teks("riwayat", "")
    kanvas_simpan_angka("angka1", 0.0)
    kanvas_simpan_teks("op", "")
    kanvas_simpan_u64("reset", 0)
}

fungsi input_persen() -> Unit {
    cur := kanvas_ambil_teks("tampilan")
    num := ke_desimal(cur)
    hasil := hitung_persen(num)
    kanvas_simpan_teks("tampilan", format_desimal(hasil))
}

fungsi input_negasi() -> Unit {
    cur := kanvas_ambil_teks("tampilan")
    num := ke_desimal(cur)
    hasil := hitung_negasi(num)
    kanvas_simpan_teks("tampilan", format_desimal(hasil))
}

fungsi hapus_satu_karakter() -> Unit {
    cur := kanvas_ambil_teks("tampilan")
    pjg := panjang_teks(cur)
    jika pjg <= 1 {
        kanvas_simpan_teks("tampilan", "0")
    } selain_itu {
        num := ke_desimal(cur)
        dipotong := potong_digit_terakhir(num)
        kanvas_simpan_teks("tampilan", format_desimal(dipotong))
    }
}

// ----------------------------------------------------------------------------
// Siklus Render Animasi Layar (Dipanggil 60 FPS oleh Direct GPU Engine)
// ----------------------------------------------------------------------------
fungsi render() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()

    // 1. Inisialisasi State Pertama Kali
    jika kanvas_ambil_u64("diinisialisasi") == 0 {
        bersihkan_kalkulator()
        kanvas_simpan_u64("diinisialisasi", 1)
    }

    // 2. Deteksi Tap / Klik Tunggal (Edge Trigger)
    klik_lama := kanvas_ambil_u64("klik_lama")
    baru_klik := (klik == 1) && (klik_lama == 0)
    kanvas_simpan_u64("klik_lama", klik)

    // 3. Bersihkan Latar Belakang Kanvas (Deep Navy Premium)
    kanvas_bersihkan(0.03, 0.05, 0.10, 1.0)

    // 4. Header Atas (Status Bar & App Bar)
    gambar_status_bar(w)
    gambar_app_bar(w, "Kalkulator DapCode")

    // 5. Tata Letak Responsif Keypad & Display
    margin_x := 14.0
    gap := 8.0
    cols := 4.0
    usable_w := w - margin_x * 2.0
    btn_w := (usable_w - gap * (cols - 1.0)) / cols

    footer_h := 36.0
    keypad_bottom := h - footer_h - 10.0

    // Keypad 5 Baris
    rows := 5.0
    keypad_h := btn_w * 5.0
    jika keypad_h > (keypad_bottom - 170.0) {
        keypad_h = keypad_bottom - 170.0
    }
    btn_h := (keypad_h - gap * (rows - 1.0)) / rows
    keypad_y := keypad_bottom - keypad_h

    // Display Layar Tampilan di Atas Keypad
    disp_y := 86.0
    disp_h := keypad_y - disp_y - 12.0
    jika disp_h < 50.0 { disp_h = 50.0 }

    tampilan := kanvas_ambil_teks("tampilan")
    riwayat := kanvas_ambil_teks("riwayat")
    gambar_layar_tampilan(w, disp_y, disp_h, riwayat, tampilan)

    // Baris 0: [ C ] [ +/- ] [ % ] [ / ]
    bx0 := margin_x + 0.0 * (btn_w + gap)
    bx1 := margin_x + 1.0 * (btn_w + gap)
    bx2 := margin_x + 2.0 * (btn_w + gap)
    bx3 := margin_x + 3.0 * (btn_w + gap)
    by0 := keypad_y + 0.0 * (btn_h + gap)

    gambar_tombol("C", bx0, by0, btn_w, btn_h, 1, mx, my, klik)
    jika tombol_ditekan(bx0, by0, btn_w, btn_h, mx, my, baru_klik) { bersihkan_kalkulator() }

    gambar_tombol("+/-", bx1, by0, btn_w, btn_h, 1, mx, my, klik)
    jika tombol_ditekan(bx1, by0, btn_w, btn_h, mx, my, baru_klik) { input_negasi() }

    gambar_tombol("%", bx2, by0, btn_w, btn_h, 1, mx, my, klik)
    jika tombol_ditekan(bx2, by0, btn_w, btn_h, mx, my, baru_klik) { input_persen() }

    gambar_tombol("/", bx3, by0, btn_w, btn_h, 2, mx, my, klik)
    jika tombol_ditekan(bx3, by0, btn_w, btn_h, mx, my, baru_klik) { input_operator("/") }

    // Baris 1: [ 7 ] [ 8 ] [ 9 ] [ * ]
    by1 := keypad_y + 1.0 * (btn_h + gap)
    gambar_tombol("7", bx0, by1, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx0, by1, btn_w, btn_h, mx, my, baru_klik) { input_angka("7") }

    gambar_tombol("8", bx1, by1, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx1, by1, btn_w, btn_h, mx, my, baru_klik) { input_angka("8") }

    gambar_tombol("9", bx2, by1, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx2, by1, btn_w, btn_h, mx, my, baru_klik) { input_angka("9") }

    gambar_tombol("*", bx3, by1, btn_w, btn_h, 2, mx, my, klik)
    jika tombol_ditekan(bx3, by1, btn_w, btn_h, mx, my, baru_klik) { input_operator("*") }

    // Baris 2: [ 4 ] [ 5 ] [ 6 ] [ - ]
    by2 := keypad_y + 2.0 * (btn_h + gap)
    gambar_tombol("4", bx0, by2, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx0, by2, btn_w, btn_h, mx, my, baru_klik) { input_angka("4") }

    gambar_tombol("5", bx1, by2, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx1, by2, btn_w, btn_h, mx, my, baru_klik) { input_angka("5") }

    gambar_tombol("6", bx2, by2, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx2, by2, btn_w, btn_h, mx, my, baru_klik) { input_angka("6") }

    gambar_tombol("-", bx3, by2, btn_w, btn_h, 2, mx, my, klik)
    jika tombol_ditekan(bx3, by2, btn_w, btn_h, mx, my, baru_klik) { input_operator("-") }

    // Baris 3: [ 1 ] [ 2 ] [ 3 ] [ + ]
    by3 := keypad_y + 3.0 * (btn_h + gap)
    gambar_tombol("1", bx0, by3, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx0, by3, btn_w, btn_h, mx, my, baru_klik) { input_angka("1") }

    gambar_tombol("2", bx1, by3, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx1, by3, btn_w, btn_h, mx, my, baru_klik) { input_angka("2") }

    gambar_tombol("3", bx2, by3, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx2, by3, btn_w, btn_h, mx, my, baru_klik) { input_angka("3") }

    gambar_tombol("+", bx3, by3, btn_w, btn_h, 2, mx, my, klik)
    jika tombol_ditekan(bx3, by3, btn_w, btn_h, mx, my, baru_klik) { input_operator("+") }

    // Baris 4: [ 0 ] [ . ] [ < ] [ = ]
    by4 := keypad_y + 4.0 * (btn_h + gap)
    gambar_tombol("0", bx0, by4, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx0, by4, btn_w, btn_h, mx, my, baru_klik) { input_angka("0") }

    gambar_tombol(".", bx1, by4, btn_w, btn_h, 0, mx, my, klik)
    jika tombol_ditekan(bx1, by4, btn_w, btn_h, mx, my, baru_klik) { input_angka(".") }

    gambar_tombol("<", bx2, by4, btn_w, btn_h, 1, mx, my, klik)
    jika tombol_ditekan(bx2, by4, btn_w, btn_h, mx, my, baru_klik) { hapus_satu_karakter() }

    gambar_tombol("=", bx3, by4, btn_w, btn_h, 3, mx, my, klik)
    jika tombol_ditekan(bx3, by4, btn_w, btn_h, mx, my, baru_klik) { hitung_hasil() }

    // 6. Footer Info
    kanvas_warna_isi(0.4, 0.6, 0.85, 0.7)
    kanvas_teks("Sentuh tombol untuk menghitung • 60 FPS", margin_x, h - 14.0, 9.0)
}

fungsi utama() -> u64 {
    render()
    kembali 0
}
"#;
    let entry_file = src_dir.join("utama.dap");
    fs::write(&entry_file, utama_code)?;

    // 5. uji/kalkulator_uji.dap (Pengujian Unit Otomatis)
    let uji_code = r#"// ============================================================================
// 🧪 Pengujian Unit Otomatis (Automated Unit Tests)
// Menguji logika bisnis kalkulator secara terisolasi tanpa engine grafis
// ============================================================================

impor "../src/logika/kalkulator.dap"

uji "Operasi penjumlahan dasar" {
    hasil := hitung_aritmatika(15.0, "+", 27.0)
    pastikan_sama(hasil, 42.0)
}

uji "Operasi pengurangan" {
    hasil := hitung_aritmatika(100.0, "-", 35.0)
    pastikan_sama(hasil, 65.0)
}

uji "Operasi perkalian" {
    hasil := hitung_aritmatika(6.0, "*", 7.0)
    pastikan_sama(hasil, 42.0)
}

uji "Operasi pembagian normal" {
    hasil := hitung_aritmatika(88.0, "/", 4.0)
    pastikan_sama(hasil, 22.0)
}

uji "Operasi pembagian dengan nol" {
    hasil := hitung_aritmatika(50.0, "/", 0.0)
    pastikan_sama(hasil, 0.0)
}

uji "Kalkulasi persentase" {
    hasil := hitung_persen(250.0)
    pastikan_sama(hasil, 2.5)
}

uji "Kalkulasi negasi tanda angka" {
    hasil := hitung_negasi(42.0)
    pastikan_sama(hasil, -42.0)

    balik := hitung_negasi(-10.0)
    pastikan_sama(balik, 10.0)
}

uji "Potong digit terakhir (Backspace)" {
    hasil := potong_digit_terakhir(1234.0)
    pastikan_sama(hasil, 123.0)

    satu_digit := potong_digit_terakhir(9.0)
    pastikan_sama(satu_digit, 0.0)
}
"#;
    fs::write(uji_dir.join("kalkulator_uji.dap"), uji_code)?;

    // 6. aset/README.md
    let aset_readme = r#"# Direktori Aset & Sumber Daya Aplikasi

Direktori ini digunakan untuk menyimpan seluruh berkas aset statis aplikasi mobile:
- **Ikon Aplikasi:** `icon.svg`, `icon-192.png`, `icon-512.png`
- **Font Khusus:** `.ttf` atau `.otf`
- **Efek Suara & Audio:** Nada dering, efek sentuhan tombol
- **Gambar & Tekstur Kanvas:** Spritesheet, latar belakang, ilustrasi
"#;
    fs::write(aset_dir.join("README.md"), aset_readme)?;

    // 7. Hasilkan runner host (android/, ios/, desktop/, engine/) secara langsung
    let mut sm = crate::span::SourceMap::new();
    let mut loader = crate::modules::ModuleLoader::new();
    if let Ok((_fid, prog)) = loader.load_program(&entry_file, &mut sm) {
        let ownership = crate::ownership::OwnershipAnalyzer::new().analyze_program(&prog);
        let mut codegen = crate::codegen::CodeGenerator::new(&ownership);
        let c_code = codegen.generate_c(&prog);
        let _ = gpu_engine::build_native_gpu_bundle(&c_code, app_name, dir);
    }

    // 8. README.md di root proyek
    let root_readme = format!(
        r#"# 📱 {} - Proyek Aplikasi Mobile DapCode (Struktur Penuh)

Aplikasi seluler terakselerasi GPU (Flutter-style Direct GPU Rendering 60 FPS) dengan bahasa DapCode.

## 🏛️ Struktur Direktori:
```text
{}/
├── DapCode.toml              # Manifes konfigurasi proyek DapCode
├── README.md                 # Dokumentasi proyek aplikasi
├── src/                      # Kode sumber aplikasi DapCode
│   ├── utama.dap             # Entry point, penanganan state, & siklus render 60 FPS
│   ├── ui/
│   │   └── komponen.dap      # Komponen UI layar sentuh (App Bar, Display, Keypad)
│   └── logika/
│       └── kalkulator.dap    # Logika bisnis murni (Aritmatika, Persen, Negasi, Backspace)
├── uji/                      # Pengujian unit otomatis (Automated Unit Tests)
│   └── kalkulator_uji.dap    # Pengujian otomatis logika matematika
├── aset/                     # Aset gambar, ikon aplikasi, font, dan audio
│   └── README.md
├── android/                  # Proyek Android Studio NDK CMake (OpenGL ES 2.0/3.0)
├── ios/                      # Proyek iOS Xcode Native (GLKit & Metal)
├── desktop/                  # Simulator layar ponsel desktop mandiri (C11 MinGW/GCC)
└── engine/                   # Source code C11 transpiled & DapCode GPU Canvas Engine
```

## ⚡ Alur Kerja Pengembangan:
- **Simulator Ponsel (Hot-Reload):** `dapcodec mobile preview`
- **Pengujian Unit Otomatis:** `dapcodec test`
- **Rakit Platform Native:** `dapcodec mobile build`
"#,
        app_name, app_name
    );
    fs::write(dir.join("README.md"), root_readme)?;

    println!(
        "\n{} Proyek aplikasi mobile '{}' berhasil dibuat dengan Struktur Proyek Aplikasi Penuh!",
        "sukses:".green().bold(),
        app_name.bold()
    );
    println!("Hierarki Direktori Proyek:");
    println!("  ├── 📄 DapCode.toml          (Manifes Proyek)");
    println!("  ├── 📂 src/");
    println!("  │   ├── 🚀 utama.dap         (Entry Point & Render Loop 60 FPS)");
    println!("  │   ├── 🎨 ui/komponen.dap   (Komponen UI Layar Sentuh)");
    println!("  │   └── 🧮 logika/kalkulator.dap (Logika Bisnis Terisolasi)");
    println!("  ├── 🧪 uji/kalkulator_uji.dap (Pengujian Unit Otomatis)");
    println!("  ├── 📁 aset/                 (Ikon & Aset Media)");
    println!("  ├── 🤖 android/              (Proyek Android Studio NDK CMake)");
    println!("  ├── 🍎 ios/                  (Proyek iOS Xcode GLKit/Metal)");
    println!("  ├── 🖥️  desktop/              (Simulator Layar Ponsel 60 FPS)");
    println!("  └── ⚡ engine/               (DapCode GPU Engine C11 Source)");
    println!("\nLangkah selanjutnya:");
    println!("  1. cd {}", app_name);
    println!("  2. dapcodec mobile preview   # Buka simulator mobile desktop dengan Hot-Reload");
    println!("  3. dapcodec test             # Jalankan pengujian unit otomatis");
    println!("  4. dapcodec mobile build     # Perbarui kode C11 di android/ & ios/\n");

    Ok(())
}
