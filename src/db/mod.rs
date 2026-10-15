use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Engine Penyimpanan Basis Data Mandiri DapCode (DapDb)
/// Format penyimpanan: Append-Only Log dengan Indeks Memori (Bitcask-inspired)
/// Mendukung Transaksi Atomik (ACID), Prefix Query, serta Ekspor/Impor JSON.
pub struct DapDb {
    path: PathBuf,
    index: HashMap<String, String>, // kunci -> nilai saat ini
    tx_staging: Option<HashMap<String, Option<String>>>, // Staging transaksi: Some(val) = set, None = delete
}

#[derive(Clone, Debug)]
enum TxOp {
    Set(String, String),
    Delete(String),
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

            let mut in_tx = false;
            let mut tx_ops: Vec<TxOp> = Vec::new();

            for line in reader.lines() {
                let l = line.map_err(|e| format!("Gagal membaca baris data: {}", e))?;
                let trimmed = l.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "TX:START" {
                    in_tx = true;
                    tx_ops.clear();
                    continue;
                } else if trimmed == "TX:COMMIT" {
                    if in_tx {
                        for op in tx_ops.drain(..) {
                            match op {
                                TxOp::Set(k, v) => {
                                    index.insert(k, v);
                                }
                                TxOp::Delete(k) => {
                                    index.remove(&k);
                                }
                            }
                        }
                        in_tx = false;
                    }
                    continue;
                } else if trimmed == "TX:ABORT" {
                    in_tx = false;
                    tx_ops.clear();
                    continue;
                }

                // Format baris:
                // S:<panjang_kunci>:<kunci>:<nilai> (Set)
                // H:<kunci> (Hapus)
                if let Some(rest) = trimmed.strip_prefix("S:") {
                    if let Some(first_colon) = rest.find(':') {
                        if let Ok(k_len) = rest[..first_colon].parse::<usize>() {
                            let after_len = &rest[first_colon + 1..];
                            if after_len.len() >= k_len && after_len.as_bytes().get(k_len) == Some(&b':') {
                                let key = &after_len[..k_len];
                                let val = &after_len[k_len + 1..];
                                // Unescape \n dan \r
                                let unescaped_val = val.replace("\\n", "\n").replace("\\r", "\r");
                                if in_tx {
                                    tx_ops.push(TxOp::Set(key.to_string(), unescaped_val));
                                } else {
                                    index.insert(key.to_string(), unescaped_val);
                                }
                            }
                        }
                    }
                } else if let Some(key) = trimmed.strip_prefix("H:") {
                    if in_tx {
                        tx_ops.push(TxOp::Delete(key.to_string()));
                    } else {
                        index.remove(key);
                    }
                }
            }
            // Transaksi yang belum memiliki TX:COMMIT sebelum EOF otomatis diabaikan (crash recovery)
        } else {
            // Buat berkas kosong
            let _ = File::create(&p).map_err(|e| format!("Gagal membuat basis data: {}", e))?;
        }

        Ok(DapDb {
            path: p,
            index,
            tx_staging: None,
        })
    }

    /// Memulai transaksi atomik baru
    pub fn mulai_transaksi(&mut self) -> Result<bool, String> {
        if self.tx_staging.is_some() {
            return Err("Transaksi sudah aktif dalam sesi basis data ini".to_string());
        }
        self.tx_staging = Some(HashMap::new());
        Ok(true)
    }

    /// Memeriksa apakah saat ini sedang dalam transaksi aktif
    pub fn sedang_transaksi(&self) -> bool {
        self.tx_staging.is_some()
    }

    /// Membatalkan (*rollback*) seluruh mutasi data dalam transaksi aktif
    pub fn batal(&mut self) -> Result<bool, String> {
        if self.tx_staging.is_none() {
            return Err("Tidak ada transaksi aktif untuk dibatalkan".to_string());
        }
        self.tx_staging = None;
        Ok(true)
    }

    /// Mengomit (*commit*) seluruh mutasi data dalam transaksi secara atomik ke disk
    pub fn komit(&mut self) -> Result<bool, String> {
        let staging = self
            .tx_staging
            .take()
            .ok_or_else(|| "Tidak ada transaksi aktif untuk dikomit".to_string())?;

        if staging.is_empty() {
            return Ok(true);
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("Gagal membuka berkas untuk komit transaksi: {}", e))?;

        let mut tx_payload = String::new();
        tx_payload.push_str("TX:START\n");
        for (key, opt_val) in &staging {
            match opt_val {
                Some(value) => {
                    let escaped_val = value.replace('\n', "\\n").replace('\r', "\\r");
                    tx_payload.push_str(&format!("S:{}:{}:{}\n", key.len(), key, escaped_val));
                }
                None => {
                    tx_payload.push_str(&format!("H:{}\n", key));
                }
            }
        }
        tx_payload.push_str("TX:COMMIT\n");

        file.write_all(tx_payload.as_bytes())
            .map_err(|e| format!("Gagal mencatat transaksi ke disk: {}", e))?;
        file.flush()
            .map_err(|e| format!("Gagal sinkronisasi transaksi ke disk: {}", e))?;

        // Terapkan ke indeks memori
        for (key, opt_val) in staging {
            match opt_val {
                Some(value) => {
                    self.index.insert(key, value);
                }
                None => {
                    self.index.remove(&key);
                }
            }
        }

        Ok(true)
    }

    /// Menyimpan nilai teks/JSON dengan kunci tertentu
    pub fn set(&mut self, key: &str, value: &str) -> Result<bool, String> {
        if key.is_empty() {
            return Err("Kunci basis data tidak boleh kosong".to_string());
        }

        if let Some(staging) = &mut self.tx_staging {
            staging.insert(key.to_string(), Some(value.to_string()));
            return Ok(true);
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
        if let Some(staging) = &self.tx_staging {
            if let Some(opt_val) = staging.get(key) {
                return match opt_val {
                    Some(v) => Ok(v.clone()),
                    None => Err(format!("Kunci '{}' tidak ditemukan dalam basis data", key)),
                };
            }
        }

        self.index
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Kunci '{}' tidak ditemukan dalam basis data", key))
    }

    /// Memeriksa apakah kunci ada dalam basis data
    pub fn has(&self, key: &str) -> bool {
        if let Some(staging) = &self.tx_staging {
            if let Some(opt_val) = staging.get(key) {
                return opt_val.is_some();
            }
        }

        self.index.contains_key(key)
    }

    /// Menghapus kunci dari basis data
    pub fn delete(&mut self, key: &str) -> Result<bool, String> {
        if let Some(staging) = &mut self.tx_staging {
            let exists = match staging.get(key) {
                Some(Some(_)) => true,
                Some(None) => false,
                None => self.index.contains_key(key),
            };
            if !exists {
                return Err(format!("Kunci '{}' tidak ditemukan untuk dihapus", key));
            }
            staging.insert(key.to_string(), None);
            return Ok(true);
        }

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

    /// Mengembalikan semua daftar kunci aktif (diurutkan secara alfabetis)
    pub fn list_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = if let Some(staging) = &self.tx_staging {
            let mut map: HashMap<String, ()> = HashMap::new();
            for k in self.index.keys() {
                map.insert(k.clone(), ());
            }
            for (k, opt_v) in staging {
                if opt_v.is_some() {
                    map.insert(k.clone(), ());
                } else {
                    map.remove(k);
                }
            }
            map.into_keys().collect()
        } else {
            self.index.keys().cloned().collect()
        };
        keys.sort();
        keys
    }

    /// Mengambil daftar kunci yang memiliki awalan (*prefix*) tertentu
    pub fn prefix_keys(&self, prefix: &str) -> Vec<String> {
        let all = self.list_keys();
        all.into_iter().filter(|k| k.starts_with(prefix)).collect()
    }

    /// Jumlah entri aktif dalam basis data
    pub fn len(&self) -> usize {
        self.list_keys().len()
    }

    /// Ekspor seluruh isi basis data ke teks JSON
    pub fn export_json(&self) -> String {
        self.export_json_prefix("")
    }

    /// Ekspor entri dengan awalan tertentu ke format objek JSON
    pub fn export_json_prefix(&self, prefix: &str) -> String {
        let keys = self.prefix_keys(prefix);
        let mut pairs = Vec::new();
        for k in keys {
            if let Ok(v) = self.get(&k) {
                let parsed = match crate::json::JsonParser::new(&v).parse() {
                    Ok(jv) => jv.to_json_string(),
                    Err(_) => crate::json::escape_json_string(&v),
                };
                pairs.push(format!("{}:{}", crate::json::escape_json_string(&k), parsed));
            }
        }
        format!("{{{}}}", pairs.join(","))
    }

    /// Impor seluruh pasangan objek JSON ke dalam basis data
    pub fn import_json(&mut self, json_str: &str) -> Result<usize, String> {
        let val = crate::json::JsonParser::new(json_str)
            .parse()
            .map_err(|e| format!("Gagal mengurai JSON untuk diimpor: {}", e))?;
        match val {
            crate::json::JsonValue::Object(pairs) => {
                let count = pairs.len();
                for (k, v) in pairs {
                    let str_val = match v {
                        crate::json::JsonValue::String(s) => s,
                        other => other.to_json_string(),
                    };
                    self.set(&k, &str_val)?;
                }
                Ok(count)
            }
            _ => Err("Format JSON harus berupa objek { ... } untuk diimpor ke basis data".to_string()),
        }
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

pub fn db_ambil_awalan(path: &str, prefix: &str) -> Result<Vec<String>, String> {
    let db_arc = db_buka(path)?;
    let db = db_arc.lock().unwrap();
    Ok(db.prefix_keys(prefix))
}

pub fn db_mulai_transaksi(path: &str) -> Result<bool, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.mulai_transaksi()
}

pub fn db_sedang_transaksi(path: &str) -> bool {
    if let Ok(db_arc) = db_buka(path) {
        let db = db_arc.lock().unwrap();
        db.sedang_transaksi()
    } else {
        false
    }
}

pub fn db_komit(path: &str) -> Result<bool, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.komit()
}

pub fn db_batal(path: &str) -> Result<bool, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.batal()
}

pub fn db_ekspor_json(path: &str) -> Result<String, String> {
    let db_arc = db_buka(path)?;
    let db = db_arc.lock().unwrap();
    Ok(db.export_json())
}

pub fn db_ekspor_json_awalan(path: &str, prefix: &str) -> Result<String, String> {
    let db_arc = db_buka(path)?;
    let db = db_arc.lock().unwrap();
    Ok(db.export_json_prefix(prefix))
}

pub fn db_impor_json(path: &str, json_str: &str) -> Result<usize, String> {
    let db_arc = db_buka(path)?;
    let mut db = db_arc.lock().unwrap();
    db.import_json(json_str)
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
        let dir = std::env::temp_dir().join("dapcode_db_test_crud");
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

    #[test]
    fn test_dapdb_transactions_commit_and_abort() {
        let dir = std::env::temp_dir().join("dapcode_db_test_tx");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let db_file = dir.join("test_tx.db");

        // 1. Rollback test
        {
            let mut db = DapDb::open(&db_file).unwrap();
            db.set("awal", "data_awal").unwrap();

            db.mulai_transaksi().unwrap();
            assert!(db.sedang_transaksi());
            db.set("tx_kunci", "akan_batal").unwrap();
            assert_eq!(db.get("tx_kunci").unwrap(), "akan_batal");

            db.batal().unwrap();
            assert!(!db.sedang_transaksi());
            assert!(!db.has("tx_kunci"));
        }

        // Verify disk did not record uncommitted data
        {
            let mut db = DapDb::open(&db_file).unwrap();
            assert!(!db.has("tx_kunci"));
            assert_eq!(db.get("awal").unwrap(), "data_awal");

            // 2. Commit test
            db.mulai_transaksi().unwrap();
            db.set("produk:1", "Apel").unwrap();
            db.set("produk:2", "Jeruk").unwrap();
            db.delete("awal").unwrap();
            assert_eq!(db.len(), 2);
            assert!(!db.has("awal"));

            db.komit().unwrap();
            assert!(!db.sedang_transaksi());
        }

        // 3. Re-open to ensure commit persisted
        {
            let db = DapDb::open(&db_file).unwrap();
            assert_eq!(db.len(), 2);
            assert_eq!(db.get("produk:1").unwrap(), "Apel");
            assert_eq!(db.get("produk:2").unwrap(), "Jeruk");
            assert!(!db.has("awal"));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_dapdb_prefix_and_json_export_import() {
        let dir = std::env::temp_dir().join("dapcode_db_test_json");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let db_file = dir.join("test_json.db");

        {
            let mut db = DapDb::open(&db_file).unwrap();
            db.set("kategori:makanan", "Roti").unwrap();
            db.set("kategori:minuman", "Kopi").unwrap();
            db.set("pengaturan:tema", "gelap").unwrap();

            // Prefix scan
            let kat = db.prefix_keys("kategori:");
            assert_eq!(kat, vec!["kategori:makanan".to_string(), "kategori:minuman".to_string()]);

            // Export JSON prefix
            let json_kat = db.export_json_prefix("kategori:");
            assert!(json_kat.contains("\"kategori:makanan\":\"Roti\""));
            assert!(json_kat.contains("\"kategori:minuman\":\"Kopi\""));
            assert!(!json_kat.contains("pengaturan:tema"));

            // Import JSON
            let raw_json = "{\"impor:1\":\"Satu\",\"impor:2\":\"Dua\"}";
            let count = db.import_json(raw_json).unwrap();
            assert_eq!(count, 2);
            assert_eq!(db.get("impor:1").unwrap(), "Satu");
            assert_eq!(db.get("impor:2").unwrap(), "Dua");
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
