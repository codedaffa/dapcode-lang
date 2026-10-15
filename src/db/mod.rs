use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Engine Penyimpanan Basis Data Mandiri DapCode (DapDb)
/// Format penyimpanan: Append-Only Log dengan Indeks Memori (Bitcask-inspired)
/// Mendukung persistensi data terstruktur (JSON/Teks), bebas dependensi eksternal.
pub struct DapDb {
    path: PathBuf,
    index: HashMap<String, String>, // kunci -> nilai saat ini
}

// Registri basis data yang sedang dibuka
static OPEN_DBS: std::sync::OnceLock<Mutex<HashMap<String, Arc<Mutex<DapDb>>>>> =
    std::sync::OnceLock::new();

fn get_db_registry() -> &'static Mutex<HashMap<String, Arc<Mutex<DapDb>>>> {
    OPEN_DBS.get_or_init(|| Mutex::new(HashMap::new()))
}

impl DapDb {
    /// Membuka atau membuat berkas basis data baru
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let p = path.as_ref().to_path_buf();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let mut index = HashMap::new();

        if p.exists() {
            let file = File::open(&p).map_err(|e| format!("Gagal membuka basis data: {}", e))?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let l = line.map_err(|e| format!("Gagal membaca baris data: {}", e))?;
                let trimmed = l.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Format baris:
                // S:<panjang_kunci>:<kunci>:<nilai> (Set)
                // H:<kunci> (Hapus)
                if let Some(rest) = trimmed.strip_prefix("S:") {
                    if let Some(first_colon) = rest.find(':') {
                        let k_len: usize = rest[..first_colon]
                            .parse()
                            .map_err(|_| "Format berkas korup: panjang kunci tidak valid".to_string())?;
                        let after_len = &rest[first_colon + 1..];
                        if after_len.len() >= k_len && after_len.as_bytes().get(k_len) == Some(&b':') {
                            let key = &after_len[..k_len];
                            let val = &after_len[k_len + 1..];
                            // Unescape \n
                            let unescaped_val = val.replace("\\n", "\n").replace("\\r", "\r");
                            index.insert(key.to_string(), unescaped_val);
                        }
                    }
                } else if let Some(key) = trimmed.strip_prefix("H:") {
                    index.remove(key);
                }
            }
        } else {
            // Buat berkas kosong
            let _ = File::create(&p).map_err(|e| format!("Gagal membuat basis data: {}", e))?;
        }

        Ok(DapDb { path: p, index })
    }

    /// Menyimpan nilai teks/JSON dengan kunci tertentu
    pub fn set(&mut self, key: &str, value: &str) -> Result<bool, String> {
        if key.is_empty() {
            return Err("Kunci basis data tidak boleh kosong".to_string());
        }

        let escaped_val = value.replace('\n', "\\n").replace('\r', "\\r");
        let line = format!("S:{}:{}:{}\n", key.len(), key, escaped_val);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("Gagal membuka berkas untuk menulis: {}", e))?;

        file.write_all(line.as_bytes())
            .map_err(|e| format!("Gagal menulis data ke disk: {}", e))?;
        file.flush()
            .map_err(|e| format!("Gagal flush ke disk: {}", e))?;

        self.index.insert(key.to_string(), value.to_string());
        Ok(true)
    }

    /// Mengambil nilai berdasarkan kunci
    pub fn get(&self, key: &str) -> Result<String, String> {
        self.index
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Kunci '{}' tidak ditemukan dalam basis data", key))
    }

    /// Memeriksa apakah kunci ada dalam basis data
    pub fn has(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// Menghapus kunci dari basis data
    pub fn delete(&mut self, key: &str) -> Result<bool, String> {
        if !self.index.contains_key(key) {
            return Err(format!("Kunci '{}' tidak ditemukan untuk dihapus", key));
        }

        let line = format!("H:{}\n", key);
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("Gagal membuka berkas untuk menghapus: {}", e))?;

        file.write_all(line.as_bytes())
            .map_err(|e| format!("Gagal mencatat operasi hapus ke disk: {}", e))?;
        file.flush()
            .map_err(|e| format!("Gagal flush ke disk: {}", e))?;

        self.index.remove(key);
        Ok(true)
    }

    /// Mengembalikan semua daftar kunci aktif
    pub fn list_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.index.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Jumlah entri aktif dalam basis data
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Memadatkan (*compact*) log berkas jika ukuran append-only membesar
    pub fn compact(&mut self) -> Result<(), String> {
        let temp_path = self.path.with_extension("dapdb.tmp");
        {
            let mut file = File::create(&temp_path)
                .map_err(|e| format!("Gagal membuat berkas pemadatan sementara: {}", e))?;
            for (k, v) in &self.index {
                let escaped_val = v.replace('\n', "\\n").replace('\r', "\\r");
                let line = format!("S:{}:{}:{}\n", k.len(), k, escaped_val);
                file.write_all(line.as_bytes())
                    .map_err(|e| format!("Gagal menulis berkas pemadatan: {}", e))?;
            }
            file.flush()
                .map_err(|e| format!("Gagal flush berkas pemadatan: {}", e))?;
        }

        fs::rename(&temp_path, &self.path)
            .map_err(|e| format!("Gagal mengganti berkas dengan versi padat: {}", e))?;

        Ok(())
    }
}

// Fungsi helper publik level modul dengan registri tersinkronisasi
pub fn db_buka(path: &str) -> Result<Arc<Mutex<DapDb>>, String> {
    let mut reg = get_db_registry().lock().unwrap();
    if let Some(db) = reg.get(path) {
        return Ok(Arc::clone(db));
    }
    let db = DapDb::open(path)?;
    let arc = Arc::new(Mutex::new(db));
    reg.insert(path.to_string(), Arc::clone(&arc));
    Ok(arc)
}

pub fn db_simpan(path: &str, key: &str, value: &str) -> Result<bool, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.set(key, value)
}

pub fn db_ambil(path: &str, key: &str) -> Result<String, String> {
    let db_arc = db_buka(path)?;
    let db = db_arc.lock().unwrap();
    db.get(key)
}

pub fn db_hapus(path: &str, key: &str) -> Result<bool, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.delete(key)
}

pub fn db_ada(path: &str, key: &str) -> bool {
    if let Ok(db_arc) = db_buka(path) {
        let db = db_arc.lock().unwrap();
        db.has(key)
    } else {
        false
    }
}

pub fn db_daftar_kunci(path: &str) -> Result<Vec<String>, String> {
    let db_arc = db_buka(path)?;
    let db = db_arc.lock().unwrap();
    Ok(db.list_keys())
}

pub fn db_padatkan(path: &str) -> Result<bool, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.compact().map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dapdb_crud_and_persistence() {
        let dir = std::env::temp_dir().join("dapcode_db_test_1");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let db_file = dir.join("test.db");

        // 1. Set & Get
        {
            let mut db = DapDb::open(&db_file).unwrap();
            assert_eq!(db.len(), 0);
            db.set("user:1", "{\"nama\":\"Budi\"}").unwrap();
            db.set("user:2", "{\"nama\":\"Siti\"}").unwrap();
            assert_eq!(db.len(), 2);
            assert!(db.has("user:1"));
            assert_eq!(db.get("user:1").unwrap(), "{\"nama\":\"Budi\"}");
        }

        // 2. Re-open to verify durability
        {
            let mut db = DapDb::open(&db_file).unwrap();
            assert_eq!(db.len(), 2);
            assert_eq!(db.get("user:2").unwrap(), "{\"nama\":\"Siti\"}");

            // Update user:1
            db.set("user:1", "{\"nama\":\"Budi Santoso\"}").unwrap();
            assert_eq!(db.get("user:1").unwrap(), "{\"nama\":\"Budi Santoso\"}");

            // Delete user:2
            db.delete("user:2").unwrap();
            assert!(!db.has("user:2"));
            assert!(db.get("user:2").is_err());
        }

        // 3. Re-open and verify compaction
        {
            let mut db = DapDb::open(&db_file).unwrap();
            assert_eq!(db.len(), 1);
            assert_eq!(db.get("user:1").unwrap(), "{\"nama\":\"Budi Santoso\"}");

            db.compact().unwrap();
            assert_eq!(db.len(), 1);
            assert_eq!(db.get("user:1").unwrap(), "{\"nama\":\"Budi Santoso\"}");
            assert_eq!(db.list_keys(), vec!["user:1".to_string()]);
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
