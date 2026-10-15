use crate::ast::*;
use crate::fmt::Formatter;
use crate::json::{parse_json, JsonValue};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::span::SourceMap;
use crate::types::TypeChecker;
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

pub struct LspServer {
    documents: HashMap<String, String>,
}

impl LspServer {
    pub fn new() -> Self {
        LspServer {
            documents: HashMap::new(),
        }
    }

    pub fn start(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();

        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }

            let line = line.trim();
            if line.starts_with("Content-Length:") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() == 2 {
                    let len: usize = parts[1].trim().parse().unwrap_or(0);
                    // Consume separator line (empty or \r)
                    let mut empty = String::new();
                    reader.read_line(&mut empty)?;

                    let mut body_buf = vec![0u8; len];
                    reader.read_exact(&mut body_buf)?;
                    let body_str = String::from_utf8_lossy(&body_buf);

                    self.handle_message(&body_str);
                }
            }
        }

        Ok(())
    }

    pub fn process_message(&mut self, body: &str) -> Option<String> {
        let Ok(msg) = parse_json(body) else {
            return None;
        };

        let id = match msg.get("id") {
            Some(JsonValue::Number(n)) => Some(*n as usize),
            _ => None,
        };

        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "initialize" => {
                let id = id.unwrap_or(1);
                Some(format!(
                    r#"{{"jsonrpc":"2.0","id":{},"result":{{"capabilities":{{"textDocumentSync":1,"hoverProvider":true,"completionProvider":{{"triggerCharacters":[".",":","\""]}},"documentFormattingProvider":true}}}}}}"#,
                    id
                ))
            }
            "initialized" => None,
            "textDocument/didOpen" => {
                if let Some(params) = msg.get("params") {
                    if let Some(td) = params.get("textDocument") {
                        if let (Some(uri), Some(text)) = (
                            td.get("uri").and_then(|u| u.as_str()),
                            td.get("text").and_then(|t| t.as_str()),
                        ) {
                            self.documents.insert(uri.to_string(), text.to_string());
                            return Some(self.build_diagnostics_notification(uri, text));
                        }
                    }
                }
                None
            }
            "textDocument/didChange" => {
                if let Some(params) = msg.get("params") {
                    if let Some(td) = params.get("textDocument") {
                        if let Some(uri) = td.get("uri").and_then(|u| u.as_str()) {
                            if let Some(JsonValue::Array(changes)) = params.get("contentChanges") {
                                if let Some(last_change) = changes.last() {
                                    if let Some(text) =
                                        last_change.get("text").and_then(|t| t.as_str())
                                    {
                                        self.documents.insert(uri.to_string(), text.to_string());
                                        return Some(self.build_diagnostics_notification(uri, text));
                                    }
                                }
                            }
                        }
                    }
                }
                None
            }
            "textDocument/hover" => {
                let id = id.unwrap_or(1);
                let mut hover_info = "### DapCode Symbol\\n\\nBahasa Pemrograman DapCode".to_string();
                if let Some(params) = msg.get("params") {
                    if let Some(td) = params.get("textDocument") {
                        if let Some(uri) = td.get("uri").and_then(|u| u.as_str()) {
                            let pos = params.get("position");
                            let line = pos
                                .and_then(|p| p.get("line"))
                                .and_then(|l| l.as_f64())
                                .map(|n| n as usize)
                                .unwrap_or(0);
                            let col = pos
                                .and_then(|p| p.get("character"))
                                .and_then(|c| c.as_f64())
                                .map(|n| n as usize)
                                .unwrap_or(0);
                            hover_info = self.compute_hover_at(uri, line, col);
                        }
                    }
                }
                Some(format!(
                    r#"{{"jsonrpc":"2.0","id":{},"result":{{"contents":{{"kind":"markdown","value":"{}"}}}}}}"#,
                    id, hover_info
                ))
            }
            "textDocument/completion" => {
                let id = id.unwrap_or(1);
                let uri = msg
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|t| t.get("uri"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                let completions = self.compute_completions(uri);
                Some(format!(
                    r#"{{"jsonrpc":"2.0","id":{},"result":[{}]}}"#,
                    id, completions
                ))
            }
            "textDocument/formatting" => {
                let id = id.unwrap_or(1);
                let uri = msg
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|t| t.get("uri"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                let edit_json = self.compute_formatting(uri);
                if edit_json.is_empty() {
                    Some(format!(r#"{{"jsonrpc":"2.0","id":{},"result":[]}}"#, id))
                } else {
                    Some(format!(
                        r#"{{"jsonrpc":"2.0","id":{},"result":[{}]}}"#,
                        id, edit_json
                    ))
                }
            }
            "shutdown" => {
                let id = id.unwrap_or(1);
                Some(format!(r#"{{"jsonrpc":"2.0","id":{},"result":null}}"#, id))
            }
            "exit" => {
                std::process::exit(0);
            }
            _ => None,
        }
    }

    fn handle_message(&mut self, body: &str) {
        if let Some(response) = self.process_message(body) {
            self.send_response(&response);
        }
    }

    fn send_response(&self, content: &str) {
        let msg = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(msg.as_bytes());
        let _ = stdout.flush();
    }

    fn build_diagnostics_notification(&self, uri: &str, text: &str) -> String {
        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file(uri, text.to_string());
        let mut lexer = Lexer::new(text, file_id);

        let diag_list = match lexer.tokenize() {
            Ok(tokens) => {
                let mut parser = Parser::new(tokens, file_id);
                match parser.parse_program() {
                    Ok(prog) => {
                        let mut checker = TypeChecker::new(file_id);
                        if let Err(diag) = checker.check_program(&prog) {
                            vec![self.diag_to_lsp_json(&diag, &sm)]
                        } else {
                            vec![]
                        }
                    }
                    Err(diag) => vec![self.diag_to_lsp_json(&diag, &sm)],
                }
            }
            Err(diag) => vec![self.diag_to_lsp_json(&diag, &sm)],
        };

        format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":"{}","diagnostics":[{}]}}}}"#,
            uri,
            diag_list.join(",")
        )
    }

    fn diag_to_lsp_json(&self, diag: &crate::diag::Diagnostic, sm: &SourceMap) -> String {
        let (line, col) = if let Some(span) = diag.span {
            if let Some(file) = sm.get_file(span.file_id) {
                file.line_col(span.start)
            } else {
                (1, 1)
            }
        } else {
            (1, 1)
        };

        let l0 = line.saturating_sub(1);
        let c0 = col.saturating_sub(1);
        let clean_msg = diag.message.replace('"', "\\\"").replace('\n', " ");

        format!(
            r#"{{"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"severity":1,"message":"{}"}}"#,
            l0,
            c0,
            l0,
            c0 + 5,
            clean_msg
        )
    }

    fn compute_hover_at(&self, uri: &str, line: usize, col: usize) -> String {
        if let Some(text) = self.documents.get(uri) {
            if let Some(word) = Self::get_word_at(text, line, col) {
                // Check built-in docs
                if let Some(doc) = Self::get_builtin_hover(&word) {
                    return doc;
                }

                // Check AST symbols in document
                let mut sm = SourceMap::new();
                let (file_id, _) = sm.add_file(uri, text.clone());
                let mut lexer = Lexer::new(text, file_id);
                if let Ok(tokens) = lexer.tokenize() {
                    let mut parser = Parser::new(tokens, file_id);
                    if let Ok(prog) = parser.parse_program() {
                        for decl in &prog.decls {
                            match decl {
                                Decl::Fn(f) if f.name == word => {
                                    let params = f
                                        .params
                                        .iter()
                                        .map(|p| format!("{}: {:?}", p.name, p.ty))
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    let ret = f
                                        .ret_ty
                                        .as_ref()
                                        .map(|t| format!(" -> {:?}", t))
                                        .unwrap_or_default();
                                    return format!(
                                        "### `fungsi {}({}){}`\\n\\nFungsi didefinisikan dalam modul ini.",
                                        f.name, params, ret
                                    );
                                }
                                Decl::Struct(s) if s.name == word => {
                                    let fields = s
                                        .fields
                                        .iter()
                                        .map(|f| format!("- `{}: {:?}`", f.name, f.ty))
                                        .collect::<Vec<_>>()
                                        .join("\\n");
                                    return format!(
                                        "### `struktur {}`\\n\\n**Bidang Struktur:**\\n{}",
                                        s.name, fields
                                    );
                                }
                                Decl::Trait(t) if t.name == word => {
                                    return format!(
                                        "### `sifat {}`\\n\\nSifat/Trait didefinisikan dalam modul ini.",
                                        t.name
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }

                return format!("### `{}`\\n\\nSimbol DapCode.", word);
            }
        }
        "### DapCode\\n\\nBahasa Pemrograman DapCode".to_string()
    }

    fn get_word_at(text: &str, target_line: usize, target_col: usize) -> Option<String> {
        let line_str = text.lines().nth(target_line)?;
        let chars: Vec<char> = line_str.chars().collect();
        if chars.is_empty() {
            return None;
        }
        let col = if target_col >= chars.len() {
            chars.len() - 1
        } else {
            target_col
        };

        if !chars[col].is_alphanumeric() && chars[col] != '_' {
            return None;
        }

        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        let mut end = col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        Some(chars[start..end].iter().collect())
    }

    fn get_builtin_hover(word: &str) -> Option<String> {
        match word {
            "ke_json" => Some("### `fungsi ke_json(nilai: T) -> Teks`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nSerialisasi nilai primitif, `Daftar`, atau struktur DapCode ke dalam representasi teks JSON.".to_string()),
            "uraikan_json" => Some("### `fungsi uraikan_json(teks: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nValidasi dan normalisasi teks JSON. Mengembalikan error union jika JSON tidak valid.".to_string()),
            "ambil_kunci_json" => Some("### `fungsi ambil_kunci_json(teks: Teks, kunci: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nMengekstrak nilai string dari suatu kunci objek JSON.".to_string()),
            "ambil_angka_json" => Some("### `fungsi ambil_angka_json(teks: Teks, kunci: Teks) -> f64 ! Teks`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nMengekstrak nilai numerik (f64) dari suatu kunci objek JSON.".to_string()),
            "ambil_bool_json" => Some("### `fungsi ambil_bool_json(teks: Teks, kunci: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nMengekstrak nilai boolean dari suatu kunci objek JSON.".to_string()),
            "ada_kunci_json" => Some("### `fungsi ada_kunci_json(teks: Teks, kunci: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nMemeriksa apakah kunci tertentu ada di dalam objek JSON.".to_string()),
            "valid_json" => Some("### `fungsi valid_json(teks: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - JSON**\\n\\nMemeriksa apakah string merupakan JSON yang valid secara sintaksis.".to_string()),
            "ambil_http" => Some("### `fungsi ambil_http(url: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Jaringan**\\n\\nMelakukan HTTP GET request ke URL (mendukung http:// dan https://). Mengembalikan isi respons teks atau error union jika gagal.".to_string()),
            "kirim_http_post" => Some("### `fungsi kirim_http_post(url: Teks, data_json: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Jaringan**\\n\\nMelakukan HTTP POST request dengan payload JSON (Content-Type: application/json). Mengembalikan isi respons teks.".to_string()),
            "ambil_status_http" => Some("### `fungsi ambil_status_http(url: Teks) -> u64 ! Teks`\\n\\n**Pustaka Standar DapCode - Jaringan**\\n\\nMengambil status code HTTP dari URL (misal 200, 404, 500).".to_string()),
            "cetak" => Some("### `fungsi cetak(...) -> Unit`\\n\\n**Pustaka Standar DapCode - Konsol**\\n\\nMencetak nilai atau ekspresi ke output standar (stdout).".to_string()),
            "baca_berkas" => Some("### `fungsi baca_berkas(jalur: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Berkas**\\n\\nMembaca seluruh isi berkas teks dari disk ke dalam variabel `Teks`.".to_string()),
            "tulis_berkas" => Some("### `fungsi tulis_berkas(jalur: Teks, isi: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Berkas**\\n\\nMenulis isi teks ke berkas di disk.".to_string()),
            "ada_berkas" => Some("### `fungsi ada_berkas(jalur: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - Berkas**\\n\\nMemeriksa keberadaan berkas pada jalur sistem berkas.".to_string()),
            "buat_direktori" => Some("### `fungsi buat_direktori(jalur: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Sistem Berkas**\\n\\nMembuat direktori baru pada disk (termasuk direktori induk secara rekursif).".to_string()),
            "daftar_direktori" => Some("### `fungsi daftar_direktori(jalur: Teks) -> Daftar<Teks> ! Teks`\\n\\n**Pustaka Standar DapCode - Sistem Berkas**\\n\\nMembaca dan mengembalikan daftar nama entri (berkas & folder) di dalam direktori.".to_string()),
            "hapus_berkas" => Some("### `fungsi hapus_berkas(jalur: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Sistem Berkas**\\n\\nMenghapus berkas dari disk.".to_string()),
            "hapus_direktori" => Some("### `fungsi hapus_direktori(jalur: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Sistem Berkas**\\n\\nMenghapus direktori beserta seluruh isinya dari disk.".to_string()),
            "gabung_jalur" => Some("### `fungsi gabung_jalur(induk: Teks, anak: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - Sistem Berkas**\\n\\nMenggabungkan dua segmen jalur direktori secara konsisten lintas sistem operasi.".to_string()),
            "tidur" => Some("### `fungsi tidur(milidetik: u64) -> Unit`\\n\\n**Pustaka Standar DapCode - Waktu & Konkurensi**\\n\\nMenunda jalannya eksekusi utas/coroutine saat ini selama durasi milidetik yang ditentukan.".to_string()),
            "ambil_argumen_cli" => Some("### `fungsi ambil_argumen_cli() -> Daftar<Teks>`\\n\\n**Pustaka Standar DapCode - Lingkungan & CLI**\\n\\nMengambil daftar argumen baris perintah (*command-line arguments*) yang diteruskan saat program dijalankan.".to_string()),
            "ambil_lingkungan" => Some("### `fungsi ambil_lingkungan(nama: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Lingkungan**\\n\\nMembaca nilai variabel lingkungan (*environment variable*) dari sistem operasi. Mengembalikan error union jika variabel tidak disetel.".to_string()),
            "atur_lingkungan" => Some("### `fungsi atur_lingkungan(nama: Teks, nilai: Teks) -> Unit`\\n\\n**Pustaka Standar DapCode - Lingkungan**\\n\\nMengatur atau memperbarui nilai variabel lingkungan (*environment variable*) pada proses saat ini.".to_string()),
            "akar_kuadrat" => Some("### `fungsi akar_kuadrat(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai akar kuadrat non-negatif.".to_string()),
            "pangkat" => Some("### `fungsi pangkat(dasar: f64, eksponen: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung hasil pemangkatan eksponensial.".to_string()),
            "nilai_mutlak" => Some("### `fungsi nilai_mutlak(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai mutlak (absolut).".to_string()),
            "panjang_teks" => Some("### `fungsi panjang_teks(s: Teks) -> u64`\\n\\n**Pustaka Standar DapCode - String**\\n\\nMengembalikan jumlah karakter dalam teks.".to_string()),
            "ke_huruf_besar" => Some("### `fungsi ke_huruf_besar(s: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - String**\\n\\nMengonversi teks menjadi huruf kapital semua.".to_string()),
            "ke_huruf_kecil" => Some("### `fungsi ke_huruf_kecil(s: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - String**\\n\\nMengonversi teks menjadi huruf kecil semua.".to_string()),
            "waktu_sekarang_ms" => Some("### `fungsi waktu_sekarang_ms() -> u64`\\n\\n**Pustaka Standar DapCode - Waktu**\\n\\nMengembalikan timestamp saat ini dalam milidetik (UNIX epoch).".to_string()),
            "uji" => Some("### Kata Kunci `uji \"nama\" { ... }`\\n\\n**Test Runner Bawaan DapCode**\\n\\nMendefinisikan skenario unit test terisolasi yang dapat dieksekusi dengan perintah `dapcodec test`.".to_string()),
            "pastikan" => Some("### `fungsi pastikan(kondisi: Bool) -> Unit`\\n\\n**Asersi Pengujian DapCode**\\n\\nMemastikan kondisi bernilai `benar`. Melempar kegagalan pengujian jika kondisi bernilai `salah`.".to_string()),
            "pastikan_sama" => Some("### `fungsi pastikan_sama(sebenarnya: T, diharapkan: T) -> Unit`\\n\\n**Asersi Pengujian DapCode**\\n\\nMemastikan kedua nilai bernilai sama persis. Menampilkan rincian perbedaan jika asersi gagal.".to_string()),
            "pastikan_tidak_sama" => Some("### `fungsi pastikan_tidak_sama(a: T, b: T) -> Unit`\\n\\n**Asersi Pengujian DapCode**\\n\\nMemastikan kedua nilai tidak bernilai sama.".to_string()),
            "sha256" => Some("### `fungsi sha256(teks: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - Kriptografi**\\n\\nMenghitung nilai cryptographic hash SHA-256 (FIPS 180-4) dalam format heksadesimal 64 karakter.".to_string()),
            "md5" => Some("### `fungsi md5(teks: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - Kriptografi**\\n\\nMenghitung nilai hash MD5 (RFC 1321) dalam format heksadesimal 32 karakter.".to_string()),
            "ke_base64" => Some("### `fungsi ke_base64(teks: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - Encoding**\\n\\nMengonversi teks menjadi format representasi Base64 (RFC 4648).".to_string()),
            "dari_base64" => Some("### `fungsi dari_base64(teks: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Encoding**\\n\\nMendekode teks Base64 kembali ke teks asli. Mengembalikan error union jika format Base64 tidak valid.".to_string()),
            "buat_server_http" => Some("### `fungsi buat_server_http(port: u64, penangan: Teks, [maks_permintaan: u64]) -> u64 ! Teks`\\n\\n**Pustaka Standar DapCode - Web Server HTTP**\\n\\nMenjalankan server web HTTP bawaan DapCode pada port tertentu. Setiap permintaan yang masuk diproses oleh fungsi penangan (*handler*) DapCode yang ditentukan.".to_string()),
            "kirim_respons_http" => Some("### `fungsi kirim_respons_http(status: u64, tipe_konten: Teks, konten: Teks) -> ResponsHttp`\\n\\n**Pustaka Standar DapCode - Web Server HTTP**\\n\\nMembuat objek `ResponsHttp` lengkap dengan status kustom, Content-Type, dan konten respons teks.".to_string()),
            "respons_json" => Some("### `fungsi respons_json(data_json: Teks) -> ResponsHttp`\\n\\n**Pustaka Standar DapCode - Web Server HTTP**\\n\\nMembuat objek `ResponsHttp` status 200 OK dengan header `Content-Type: application/json`.".to_string()),
            "respons_teks" => Some("### `fungsi respons_teks(teks: Teks) -> ResponsHttp`\\n\\n**Pustaka Standar DapCode - Web Server HTTP**\\n\\nMembuat objek `ResponsHttp` status 200 OK dengan header `Content-Type: text/plain; charset=utf-8`.".to_string()),
            "PermintaanHttp" => Some("### `struktur PermintaanHttp`\\n\\n**Model Data Permintaan HTTP Bawaan DapCode**\\n\\n- `metode: Teks` (GET, POST, PUT, DELETE, dll)\\n- `jalur: Teks` (contoh: /api/halo)\\n- `kueri: Teks` (query string mentah)\\n- `isi: Teks` (body permintaan)".to_string()),
            "ResponsHttp" => Some("### `struktur ResponsHttp`\\n\\n**Model Data Respons HTTP Bawaan DapCode**\\n\\n- `status: u64` (contoh: 200, 404, 500)\\n- `tipe_konten: Teks` (application/json, text/plain, dll)\\n- `konten: Teks` (body respons)".to_string()),
            "struktur" => Some("### Kata Kunci `struktur`\\n\\nMendeklarasikan tipe data komposit baru dengan bidang-bidang bertipe.".to_string()),
            "sifat" => Some("### Kata Kunci `sifat`\\n\\nMendeklarasikan kontrak antarmuka (trait) perilaku bersama.".to_string()),
            "terapkan" => Some("### Kata Kunci `terapkan`\\n\\nMengimplementasikan sifat (trait) atau kumpulan metode untuk suatu struktur.".to_string()),
            "fungsi" => Some("### Kata Kunci `fungsi`\\n\\nMendeklarasikan fungsi baru.".to_string()),
            "jalan_paralel" => Some("### Kata Kunci `jalan_paralel`\\n\\nMeluncurkan coroutine asinkron/paralel (*colorless coroutine*).".to_string()),
            "tunggu" => Some("### Kata Kunci `tunggu`\\n\\nMensinkronisasi dan mengekstrak hasil tugas dari `jalan_paralel`.".to_string()),
            "comptime" => Some("### Kata Kunci `comptime`\\n\\nEvaluasi ekspresi langsung saat fase kompilasi.".to_string()),
            "impor" => Some("### Kata Kunci `impor`\\n\\nMengimpor modul berkas `.dap` lain secara relatif.".to_string()),
            "jika" => Some("### Kata Kunci `jika`\\n\\nPercabangan kondisi logika.".to_string()),
            "selain_itu" => Some("### Kata Kunci `selain_itu`\\n\\nBlok alternatif percabangan jika.".to_string()),
            "selama" => Some("### Kata Kunci `selama`\\n\\nPerulangan berulang selama kondisi bernilai `benar`.".to_string()),
            "untuk" => Some("### Kata Kunci `untuk`\\n\\nPerulangan `untuk item di koleksi` atau pengikat `terapkan ... untuk ...`.".to_string()),
            "gagal" => Some("### Kata Kunci `gagal`\\n\\nMelemparkan nilai kesalahan pada Error Union (`T ! E`).".to_string()),
            "tangkap" => Some("### Kata Kunci `tangkap`\\n\\nMenangkap kesalahan inline dari Error Union (`? tangkap err { ... }`).".to_string()),
            "kembali" => Some("### Kata Kunci `kembali`\\n\\nMengembalikan nilai dari fungsi.".to_string()),
            _ => None,
        }
    }

    fn compute_completions(&self, uri: &str) -> String {
        let mut items = Vec::new();

        let builtins = [
            ("cetak", "cetak(\"$1\")", 3, "Fungsi cetak output konsol"),
            ("ke_json", "ke_json($1)", 3, "Serialisasi nilai ke format JSON"),
            ("uraikan_json", "uraikan_json($1)", 3, "Validasi/penguraian teks JSON"),
            ("ambil_kunci_json", "ambil_kunci_json($1, \"$2\")", 3, "Ekstrak string dari kunci JSON"),
            ("ambil_angka_json", "ambil_angka_json($1, \"$2\")", 3, "Ekstrak float dari kunci JSON"),
            ("ambil_bool_json", "ambil_bool_json($1, \"$2\")", 3, "Ekstrak boolean dari kunci JSON"),
            ("ada_kunci_json", "ada_kunci_json($1, \"$2\")", 3, "Periksa keberadaan kunci JSON"),
            ("valid_json", "valid_json($1)", 3, "Periksa validitas sintaks JSON"),
            ("baca_berkas", "baca_berkas(\"$1\")", 3, "Baca seluruh isi berkas dari disk"),
            ("tulis_berkas", "tulis_berkas(\"$1\", \"$2\")", 3, "Tulis teks ke berkas di disk"),
            ("ada_berkas", "ada_berkas(\"$1\")", 3, "Periksa keberadaan berkas"),
            ("panjang_teks", "panjang_teks($1)", 3, "Hitung panjang string"),
            ("ke_huruf_besar", "ke_huruf_besar($1)", 3, "Ubah teks ke huruf kapital"),
            ("ke_huruf_kecil", "ke_huruf_kecil($1)", 3, "Ubah teks ke huruf kecil"),
            ("akar_kuadrat", "akar_kuadrat($1)", 3, "Akar kuadrat matematika"),
            ("pangkat", "pangkat($1, $2)", 3, "Pemangkatan matematika"),
            ("nilai_mutlak", "nilai_mutlak($1)", 3, "Nilai absolut matematika"),
            ("waktu_sekarang_ms", "waktu_sekarang_ms()", 3, "Timestamp milidetik"),
            ("ambil_http", "ambil_http(\"$1\")", 3, "HTTP GET request ke URL"),
            ("kirim_http_post", "kirim_http_post(\"$1\", $2)", 3, "HTTP POST JSON request ke URL"),
            ("ambil_status_http", "ambil_status_http(\"$1\")", 3, "Ambil status code HTTP dari URL"),
            ("buat_server_http", "buat_server_http(${1:8080}, \"${2:tangani_permintaan}\")", 3, "Jalankan HTTP Web Server DapCode"),
            ("kirim_respons_http", "kirim_respons_http(${1:200}, \"${2:application/json}\", ${3:data})", 3, "Buat objek ResponsHttp lengkap"),
            ("respons_json", "respons_json(${1:data_json})", 3, "Buat respons HTTP 200 OK JSON"),
            ("respons_teks", "respons_teks(\"${1:konten}\")", 3, "Buat respons HTTP 200 OK Text"),
            ("sha256", "sha256(\"$1\")", 3, "Kalkulasi cryptographic hash SHA-256 teks"),
            ("md5", "md5(\"$1\")", 3, "Kalkulasi hash MD5 teks"),
            ("ke_base64", "ke_base64(\"$1\")", 3, "Encode teks ke Base64"),
            ("dari_base64", "dari_base64(\"$1\")", 3, "Decode teks dari Base64"),
        ];

        for (label, snippet, kind, doc) in builtins {
            items.push(format!(
                r#"{{"label":"{}","kind":{},"detail":"{}","insertText":"{}","insertTextFormat":2}}"#,
                label, kind, doc, snippet.replace('"', "\\\"")
            ));
        }

        let keywords = [
            ("fungsi", "fungsi ${1:nama}() -> ${2:Unit} {\n\t$0\n}", 14, "Deklarasi fungsi baru"),
            ("struktur", "struktur ${1:Nama} {\n\t${2:bidang}: ${3:Teks},\n}", 14, "Deklarasi tipe struktur data"),
            ("sifat", "sifat ${1:NamaSifat} {\n\tfungsi ${2:metode}(ini) -> ${3:Teks}\n}", 14, "Deklarasi antarmuka/trait"),
            ("terapkan", "terapkan ${1:Sifat} untuk ${2:Struktur} {\n\t$0\n}", 14, "Implementasi trait"),
            ("jika", "jika ${1:kondisi} {\n\t$0\n}", 14, "Percabangan kondisi"),
            ("selain_itu", "selain_itu {\n\t$0\n}", 14, "Blok alternatif percabangan"),
            ("selama", "selama ${1:kondisi} {\n\t$0\n}", 14, "Perulangan while"),
            ("untuk", "untuk ${1:item} di ${2:koleksi} {\n\t$0\n}", 14, "Perulangan for-in"),
            ("kembali", "kembali $0", 14, "Kembalikan nilai fungsi"),
            ("gagal", "gagal \"$0\"", 14, "Lemparkan kesalahan pada Error Union"),
            ("tangkap", "? tangkap ${1:err} {\n\t$0\n}", 14, "Blok penanganan error inline"),
            ("jalan_paralel", "jalan_paralel {\n\t$0\n}", 14, "Luncurkan coroutine asinkron"),
            ("tunggu", "tunggu($1)", 14, "Sinkronisasi hasil tugas paralel"),
            ("comptime", "comptime {\n\t$0\n}", 14, "Evaluasi saat kompilasi"),
            ("impor", "impor \"$1\"", 14, "Impor berkas modul"),
            ("ini", "ini", 14, "Referensi penerima metode (self)"),
            ("benar", "benar", 14, "Konstanta boolean true"),
            ("salah", "salah", 14, "Konstanta boolean false"),
            ("u64", "u64", 7, "Tipe integer 64-bit unsigned"),
            ("f64", "f64", 7, "Tipe float 64-bit"),
            ("Teks", "Teks", 7, "Tipe teks / string"),
            ("Bool", "Bool", 7, "Tipe boolean"),
            ("Unit", "Unit", 7, "Tipe kosong ()"),
            ("Daftar", "Daftar<$1>", 7, "Tipe koleksi dinamis"),
            ("uji", "uji \"${1:nama_pengujian}\" {\n\t$0\n}", 14, "Blok pengujian unit terintegrasi"),
            ("pastikan", "pastikan(${1:kondisi})", 3, "Asersi boolean pengujian"),
            ("pastikan_sama", "pastikan_sama(${1:sebenarnya}, ${2:diharapkan})", 3, "Asersi kesamaan nilai pengujian"),
            ("pastikan_tidak_sama", "pastikan_tidak_sama(${1:a}, ${2:b})", 3, "Asersi ketidaksamaan nilai pengujian"),
            ("buat_direktori", "buat_direktori(\"${1:jalur}\")", 3, "Buat direktori baru rekursif"),
            ("daftar_direktori", "daftar_direktori(\"${1:jalur}\")", 3, "Daftar isi direktori"),
            ("hapus_berkas", "hapus_berkas(\"${1:jalur}\")", 3, "Hapus berkas dari disk"),
            ("hapus_direktori", "hapus_direktori(\"${1:jalur}\")", 3, "Hapus direktori dari disk"),
            ("gabung_jalur", "gabung_jalur(\"${1:induk}\", \"${2:anak}\")", 3, "Gabungkan dua segmen jalur"),
            ("tidur", "tidur(${1:100})", 3, "Tunda eksekusi (milidetik)"),
            ("ambil_argumen_cli", "ambil_argumen_cli()", 3, "Ambil daftar argumen baris perintah"),
            ("ambil_lingkungan", "ambil_lingkungan(\"${1:NAMA_VARIABEL}\")", 3, "Baca variabel lingkungan OS"),
            ("atur_lingkungan", "atur_lingkungan(\"${1:NAMA_VARIABEL}\", \"${2:nilai}\")", 3, "Setel variabel lingkungan OS"),
        ];

        for (label, snippet, kind, doc) in keywords {
            items.push(format!(
                r#"{{"label":"{}","kind":{},"detail":"{}","insertText":"{}","insertTextFormat":2}}"#,
                label, kind, doc, snippet.replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t")
            ));
        }

        // Add user symbols from document
        if let Some(text) = self.documents.get(uri) {
            let mut sm = SourceMap::new();
            let (file_id, _) = sm.add_file(uri, text.clone());
            let mut lexer = Lexer::new(text, file_id);
            if let Ok(tokens) = lexer.tokenize() {
                let mut parser = Parser::new(tokens, file_id);
                if let Ok(prog) = parser.parse_program() {
                    for decl in &prog.decls {
                        match decl {
                            Decl::Fn(f) => {
                                items.push(format!(
                                    r#"{{"label":"{}","kind":3,"detail":"Fungsi lokal","insertText":"{}()"}}"#,
                                    f.name, f.name
                                ));
                            }
                            Decl::Struct(s) => {
                                items.push(format!(
                                    r#"{{"label":"{}","kind":7,"detail":"Struktur lokal","insertText":"{}"}}"#,
                                    s.name, s.name
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        items.join(",")
    }

    fn compute_formatting(&self, uri: &str) -> String {
        if let Some(text) = self.documents.get(uri) {
            let mut sm = SourceMap::new();
            let (file_id, _) = sm.add_file(uri, text.clone());
            let mut lexer = Lexer::new(text, file_id);
            if let Ok(tokens) = lexer.tokenize() {
                let mut parser = Parser::new(tokens, file_id);
                if let Ok(prog) = parser.parse_program() {
                    let mut fmt = Formatter::new();
                    let formatted = fmt.format_program(&prog);
                    let clean = formatted
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n");
                    let lines_count = text.lines().count();
                    return format!(
                        r#"{{"range":{{"start":{{"line":0,"character":0}},"end":{{"line":{},"character":0}}}},"newText":"{}"}}"#,
                        lines_count + 1,
                        clean
                    );
                }
            }
        }
        "".to_string()
    }
}
