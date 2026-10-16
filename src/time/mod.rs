//! Pustaka Standar Waktu & Kalender DapCode (Waktu, Tanggal & Durasi)
//!
//! Menyediakan fungsi dan manipulasi kalender sipil menggunakan
//! algoritma Howard Hinnant (C++20 chrono) tanpa dependensi eksternal.

use std::time::{SystemTime, UNIX_EPOCH};

/// Nama hari dalam Bahasa Indonesia (0 = Minggu, 1 = Senin, ..., 6 = Sabtu)
pub const NAMA_HARI: [&str; 7] = [
    "Minggu",
    "Senin",
    "Selasa",
    "Rabu",
    "Kamis",
    "Jumat",
    "Sabtu",
];

/// Nama bulan dalam Bahasa Indonesia (1-indexed: 1 = Januari, ..., 12 = Desember)
pub const NAMA_BULAN: [&str; 12] = [
    "Januari",
    "Februari",
    "Maret",
    "April",
    "Mei",
    "Juni",
    "Juli",
    "Agustus",
    "September",
    "Oktober",
    "November",
    "Desember",
];

/// Mengembalikan waktu saat ini dalam milidetik sejak UNIX epoch (1970-01-01 00:00:00 UTC)
pub fn waktu_sekarang_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Mengembalikan waktu saat ini dalam detik sejak UNIX epoch
pub fn waktu_sekarang_detik() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Memeriksa apakah suatu tahun adalah tahun kabisat (Leap Year)
pub fn waktu_adalah_kabisat(tahun: u64) -> bool {
    (tahun % 4 == 0 && tahun % 100 != 0) || (tahun % 400 == 0)
}

/// Mengonversi detik sejak UNIX epoch ke komponen kalender UTC (tahun, bulan, hari, jam, menit, detik)
/// Menggunakan algoritma civil calendar Howard Hinnant.
pub fn epoch_ke_komponen(epoch: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = epoch % 60;
    let min = (epoch / 60) % 60;
    let hour = (epoch / 3600) % 24;
    let days = (epoch / 86400) as i64;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if m <= 2 { y + 1 } else { y } as u64;

    (yr, m, d, hour, min, sec)
}

/// Mengonversi komponen kalender (tahun, bulan, hari, jam, menit, detik) ke detik sejak UNIX epoch.
pub fn komponen_ke_epoch(tahun: u64, bulan: u64, hari: u64, jam: u64, menit: u64, detik: u64) -> u64 {
    let y = if bulan <= 2 { tahun as i64 - 1 } else { tahun as i64 };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if bulan <= 2 { bulan + 9 } else { bulan - 3 };
    let doy = (153 * mp + 2) / 5 + hari - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + (doe as i64) - 719468;
    (days * 86400) as u64 + jam * 3600 + menit * 60 + detik
}

/// Menghitung nama hari dalam Bahasa Indonesia untuk detik UNIX epoch yang diberikan.
pub fn waktu_hari_dalam_minggu(epoch: u64) -> &'static str {
    let days = (epoch / 86400) as i64;
    // 1970-01-01 adalah hari Kamis (indeks 4)
    let idx = ((days + 4) % 7 + 7) % 7;
    NAMA_HARI[idx as usize]
}

/// Memformat epoch detik ke representasi ISO 8601 standar: "YYYY-MM-DDTHH:MM:SSZ"
pub fn waktu_format_iso(epoch: u64) -> String {
    let (yr, m, d, hr, mn, sc) = epoch_ke_komponen(epoch);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", yr, m, d, hr, mn, sc)
}

/// Memformat waktu berdasarkan pola format kustom:
/// - %Y: Tahun 4 digit (contoh: 2026)
/// - %m: Bulan 2 digit (01-12)
/// - %d: Hari 2 digit (01-31)
/// - %H: Jam 24 format 2 digit (00-23)
/// - %M: Menit 2 digit (00-59)
/// - %S: Detik 2 digit (00-59)
/// - %A: Nama hari Indonesia ("Senin", ...)
/// - %B: Nama bulan Indonesia ("Januari", ...)
/// - %%: Karakter persen literal
pub fn waktu_format(epoch: u64, pola: &str) -> String {
    let (yr, m, d, hr, mn, sc) = epoch_ke_komponen(epoch);
    let mut out = String::with_capacity(pola.len() + 16);
    let chars: Vec<char> = pola.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            match chars[i + 1] {
                'Y' => out.push_str(&format!("{:04}", yr)),
                'm' => out.push_str(&format!("{:02}", m)),
                'd' => out.push_str(&format!("{:02}", d)),
                'H' => out.push_str(&format!("{:02}", hr)),
                'M' => out.push_str(&format!("{:02}", mn)),
                'S' => out.push_str(&format!("{:02}", sc)),
                'A' => out.push_str(waktu_hari_dalam_minggu(epoch)),
                'B' => {
                    let b_idx = if m >= 1 && m <= 12 { (m - 1) as usize } else { 0 };
                    out.push_str(NAMA_BULAN[b_idx]);
                }
                '%' => out.push('%'),
                other => {
                    out.push('%');
                    out.push(other);
                }
            }
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Mengambil waktu lokal sistem saat ini (tahun, bulan, hari, jam, menit, detik)
pub fn waktu_sekarang_lokal() -> (u64, u64, u64, u64, u64, u64) {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SYSTEMTIME {
            w_year: u16,
            w_month: u16,
            w_day_of_week: u16,
            w_day: u16,
            w_hour: u16,
            w_minute: u16,
            w_second: u16,
            w_milliseconds: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(lp_system_time: *mut SYSTEMTIME);
        }
        let mut st = SYSTEMTIME {
            w_year: 0,
            w_month: 0,
            w_day_of_week: 0,
            w_day: 0,
            w_hour: 0,
            w_minute: 0,
            w_second: 0,
            w_milliseconds: 0,
        };
        unsafe {
            GetLocalTime(&mut st);
        }
        (
            st.w_year as u64,
            st.w_month as u64,
            st.w_day as u64,
            st.w_hour as u64,
            st.w_minute as u64,
            st.w_second as u64,
        )
    }

    #[cfg(not(windows))]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32,
            tm_min: i32,
            tm_hour: i32,
            tm_mday: i32,
            tm_mon: i32,
            tm_year: i32,
            tm_wday: i32,
            tm_yday: i32,
            tm_isdst: i32,
        }
        unsafe extern "C" {
            fn time(tloc: *mut i64) -> i64;
            fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
        }
        let mut t: i64 = 0;
        let mut res = std::mem::MaybeUninit::<Tm>::zeroed();
        unsafe {
            time(&mut t);
            localtime_r(&t, res.as_mut_ptr());
            let res = res.assume_init();
            (
                (res.tm_year + 1900) as u64,
                (res.tm_mon + 1) as u64,
                res.tm_mday as u64,
                res.tm_hour as u64,
                res.tm_min as u64,
                res.tm_sec as u64,
            )
        }
    }
}

/// Menghitung selisih offset zona waktu lokal terhadap UTC dalam detik
pub fn zona_waktu_offset_detik() -> i64 {
    let lokal = waktu_sekarang_lokal();
    let epoch_lokal = komponen_ke_epoch(lokal.0, lokal.1, lokal.2, lokal.3, lokal.4, lokal.5);
    let epoch_utc = waktu_sekarang_detik();
    (epoch_lokal as i64) - (epoch_utc as i64)
}

/// Mengonversi detik epoch ke komponen waktu lokal sistem
pub fn epoch_ke_lokal_komponen(epoch: u64) -> (u64, u64, u64, u64, u64, u64) {
    let offset = zona_waktu_offset_detik();
    let local_epoch = ((epoch as i64) + offset).max(0) as u64;
    epoch_ke_komponen(local_epoch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kabisat() {
        assert!(waktu_adalah_kabisat(2000));
        assert!(waktu_adalah_kabisat(2024));
        assert!(!waktu_adalah_kabisat(1900));
        assert!(!waktu_adalah_kabisat(2023));
        assert!(!waktu_adalah_kabisat(2025));
        assert!(!waktu_adalah_kabisat(2026));
    }

    #[test]
    fn test_epoch_roundtrip() {
        // 1970-01-01 00:00:00 UTC
        assert_eq!(epoch_ke_komponen(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(komponen_ke_epoch(1970, 1, 1, 0, 0, 0), 0);

        // 2026-10-16 09:46:00
        let ep = komponen_ke_epoch(2026, 10, 16, 9, 46, 0);
        let comp = epoch_ke_komponen(ep);
        assert_eq!(comp, (2026, 10, 16, 9, 46, 0));
    }

    #[test]
    fn test_hari_dalam_minggu() {
        // 1970-01-01 was Thursday (Kamis)
        assert_eq!(waktu_hari_dalam_minggu(0), "Kamis");
        // 1970-01-02 was Friday (Jumat)
        assert_eq!(waktu_hari_dalam_minggu(86400), "Jumat");
        // 1970-01-04 was Sunday (Minggu)
        assert_eq!(waktu_hari_dalam_minggu(86400 * 3), "Minggu");
        // 1970-01-05 was Monday (Senin)
        assert_eq!(waktu_hari_dalam_minggu(86400 * 4), "Senin");
    }

    #[test]
    fn test_format_iso() {
        assert_eq!(waktu_format_iso(0), "1970-01-01T00:00:00Z");
        let ep = komponen_ke_epoch(2026, 5, 4, 14, 30, 15);
        assert_eq!(waktu_format_iso(ep), "2026-05-04T14:30:15Z");
    }

    #[test]
    fn test_format_custom() {
        let ep = komponen_ke_epoch(2026, 8, 17, 10, 0, 0);
        let s = waktu_format(ep, "%d/%m/%Y %H:%M:%S");
        assert_eq!(s, "17/08/2026 10:00:00");

        let s2 = waktu_format(ep, "%A, %d %B %Y");
        // 2026-08-17 is Monday (Senin)
        assert_eq!(s2, "Senin, 17 Agustus 2026");
    }
}

