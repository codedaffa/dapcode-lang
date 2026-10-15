use crate::ast::*;
use crate::fmt::Formatter;
use crate::json::{parse_json, JsonValue};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::span::{SourceMap, Span};
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
                    r#"{{"jsonrpc":"2.0","id":{},"result":{{"capabilities":{{"textDocumentSync":1,"hoverProvider":true,"completionProvider":{{"triggerCharacters":[".",":","\""]}},"documentFormattingProvider":true,"definitionProvider":true,"documentSymbolProvider":true,"codeActionProvider":true}}}}}}"#,
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
            "textDocument/definition" => {
                let id = id.unwrap_or(1);
                let mut def_json = "null".to_string();
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
                            if let Some(loc) = self.compute_definition_at(uri, line, col) {
                                def_json = loc;
                            }
                        }
                    }
                }
                Some(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#, id, def_json))
            }
            "textDocument/documentSymbol" => {
                let id = id.unwrap_or(1);
                let uri = msg
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|t| t.get("uri"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                let symbols = self.compute_document_symbols(uri);
                Some(format!(r#"{{"jsonrpc":"2.0","id":{},"result":[{}]}}"#, id, symbols))
            }
            "textDocument/codeAction" => {
                let id = id.unwrap_or(1);
                let actions = self.compute_code_actions(msg.get("params"));
                if actions.is_empty() {
                    Some(format!(r#"{{"jsonrpc":"2.0","id":{},"result":[]}}"#, id))
                } else {
                    Some(format!(
                        r#"{{"jsonrpc":"2.0","id":{},"result":[{}]}}"#,
                        id, actions
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
        if let Some(text) = self.get_document_text(uri) {
            if let Some(word) = Self::get_word_at(&text, line, col) {
                // Check built-in docs
                if let Some(doc) = Self::get_builtin_hover(&word) {
                    return doc;
                }

                // Check AST symbols in document
                let mut sm = SourceMap::new();
                let (file_id, _) = sm.add_file(uri, text.clone());
                let mut lexer = Lexer::new(&text, file_id);
                if let Ok(tokens) = lexer.tokenize() {
                    let mut parser = Parser::new(tokens, file_id);
                    if let Ok(prog) = parser.parse_program() {
                        for decl in &prog.decls {
                            match decl {
                                Decl::Fn(f) if f.name == word => {
                                    let params = f
                                        .params
                                        .iter()
                                        .map(|p| {
                                            if p.is_self {
                                                "ini".to_string()
                                            } else if let Some(ref ty) = p.ty {
                                                format!("{}: {}", p.name, crate::doc::format_type_expr(ty))
                                            } else {
                                                p.name.clone()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    let ret = f
                                        .ret_ty
                                        .as_ref()
                                        .map(|t| format!(" -> {}", crate::doc::format_type_expr(t)))
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
                                        .map(|f| format!("- `{}: {}`", f.name, crate::doc::format_type_expr(&f.ty)))
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
                                Decl::Impl(imp) => {
                                    for m in &imp.methods {
                                        if m.name == word {
                                            let params = m
                                                .params
                                                .iter()
                                                .map(|p| {
                                                    if p.is_self {
                                                        "ini".to_string()
                                                    } else if let Some(ref ty) = p.ty {
                                                        format!("{}: {}", p.name, crate::doc::format_type_expr(ty))
                                                    } else {
                                                        p.name.clone()
                                                    }
                                                })
                                                .collect::<Vec<_>>()
                                                .join(", ");
                                            let ret = m
                                                .ret_ty
                                                .as_ref()
                                                .map(|t| format!(" -> {}", crate::doc::format_type_expr(t)))
                                                .unwrap_or_default();
                                            return format!(
                                                "### `metode {}({}){}`\\n\\nMetode untuk struktur `{}`.",
                                                m.name, params, ret, imp.target_struct
                                            );
                                        }
                                    }
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
            "PI" => Some("### `konstanta PI: f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nKonstanta matematika Archimedes (π = 3.141592653589793...). Rasio keliling lingkaran terhadap diameternya.".to_string()),
            "TAU" => Some("### `konstanta TAU: f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nKonstanta lingkaran penuh (τ = 2π = 6.283185307179586...). Satu putaran penuh dalam radian.".to_string()),
            "E" => Some("### `konstanta E: f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nKonstanta bilangan Euler (e = 2.718281828459045...). Basis dari logaritma natural.".to_string()),
            "sin" => Some("### `fungsi sin(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai sinus dari sudut `x` (dalam radian).".to_string()),
            "cos" => Some("### `fungsi cos(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai kosinus dari sudut `x` (dalam radian).".to_string()),
            "tan" => Some("### `fungsi tan(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai tangen dari sudut `x` (dalam radian).".to_string()),
            "asin" => Some("### `fungsi asin(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai busur sinus (arcsinus) dalam rentang [-π/2, π/2] radian.".to_string()),
            "acos" => Some("### `fungsi acos(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai busur kosinus (arckosinus) dalam rentang [0, π] radian.".to_string()),
            "atan" => Some("### `fungsi atan(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai busur tangen (arctangen) dalam rentang [-π/2, π/2] radian.".to_string()),
            "atan2" => Some("### `fungsi atan2(y: f64, x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMenghitung nilai arctangen dari koordinat kuadran kartesius (y, x) dalam rentang (-π, π] radian.".to_string()),
            "akar_kuadrat" => Some("### `fungsi akar_kuadrat(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai akar kuadrat non-negatif (√x).".to_string()),
            "akar_kubik" => Some("### `fungsi akar_kubik(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai akar kubik (∛x).".to_string()),
            "pangkat" => Some("### `fungsi pangkat(dasar: f64, eksponen: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung hasil pemangkatan eksponensial (dasar^eksponen).".to_string()),
            "nilai_mutlak" => Some("### `fungsi nilai_mutlak(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai mutlak absolut (|x|).".to_string()),
            "bulat" => Some("### `fungsi bulat(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nPembulatan angka desimal ke bilangan bulat terdekat.".to_string()),
            "lantai" => Some("### `fungsi lantai(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nPembulatan ke bawah (floor) ke bilangan bulat terdekat terkecil.".to_string()),
            "langit_langit" => Some("### `fungsi langit_langit(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nPembulatan ke atas (ceil) ke bilangan bulat terdekat terbesar.".to_string()),
            "tanda" => Some("### `fungsi tanda(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMengembalikan tanda bilangan: 1.0 jika positif, -1.0 jika negatif, dan 0.0 jika nol.".to_string()),
            "log" => Some("### `fungsi log(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai logaritma natural (basis e).".to_string()),
            "log10" => Some("### `fungsi log10(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai logaritma basis 10.".to_string()),
            "log2" => Some("### `fungsi log2(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai logaritma basis 2.".to_string()),
            "exp" => Some("### `fungsi exp(x: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMenghitung nilai eksponensial e^x.".to_string()),
            "min" => Some("### `fungsi min(a: f64, b: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMengembalikan nilai minimum antara dua angka desimal.".to_string()),
            "max" => Some("### `fungsi max(a: f64, b: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Matematika**\\n\\nMengembalikan nilai maksimum antara dua angka desimal.".to_string()),
            "ke_radian" => Some("### `fungsi ke_radian(derajat: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMengonversi besar sudut derajat ke satuan radian.".to_string()),
            "ke_derajat" => Some("### `fungsi ke_derajat(radian: f64) -> f64`\\n\\n**Pustaka Standar DapCode - Trigonometri**\\n\\nMengonversi besar sudut radian ke satuan derajat.".to_string()),
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
            "tambah_header" => Some("### `fungsi tambah_header(resp: ResponsHttp, kunci: Teks, nilai: Teks) -> ResponsHttp`\\n\\n**Pustaka Standar DapCode - Web Server HTTP**\\n\\nMenambahkan header HTTP kustom ke objek respons (contoh: Authorization, Cache-Control, atau CORS).".to_string()),
            "respons_cors" => Some("### `fungsi respons_cors(resp: ResponsHttp) -> ResponsHttp`\\n\\n**Pustaka Standar DapCode - Web Framework**\\n\\nMenambahkan header CORS standar (`Access-Control-Allow-Origin: *`, Methods, dan Headers) ke objek respons.".to_string()),
            "cocok_rute" => Some("### `fungsi cocok_rute(pola: Teks, jalur: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - Web Router**\\n\\nMemeriksa apakah jalur URL cocok dengan pola rute dinamis (mendukung wildcard `:nama_param` dan `*`).\\n\\n*Contoh:* `cocok_rute(\"/api/produk/:id\", \"/api/produk/42\")` -> `benar`".to_string()),
            "ambil_param_rute" => Some("### `fungsi ambil_param_rute(pola: Teks, jalur: Teks, nama_param: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Web Router**\\n\\nMengekstrak nilai parameter dari segmen rute dinamis.\\n\\n*Contoh:* `ambil_param_rute(\"/api/produk/:id\", \"/api/produk/42\", \"id\")` -> `\"42\"`".to_string()),
            "ambil_kueri_param" => Some("### `fungsi ambil_kueri_param(kueri: Teks, kunci: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Web Router**\\n\\nMengekstrak nilai parameter dari query string URL dengan URL decoding otomatis.\\n\\n*Contoh:* `ambil_kueri_param(\"kategori=buku&hal=2\", \"kategori\")` -> `\"buku\"`".to_string()),
            "ada_kueri_param" => Some("### `fungsi ada_kueri_param(kueri: Teks, kunci: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - Web Router**\\n\\nMemeriksa apakah kunci parameter kueri tertentu terdapat dalam query string URL.".to_string()),
            "respons_berkas" => Some("### `fungsi respons_berkas(jalur_berkas: Teks) -> ResponsHttp ! Teks`\\n\\n**Pustaka Standar DapCode - Web Server HTTP**\\n\\nMembaca berkas dari disk dan menghasilkan `ResponsHttp` 200 OK dengan `Content-Type` terdeteksi otomatis sesuai ekstensi MIME berkas.".to_string()),
            "sajikan_berkas_statis" => Some("### `fungsi sajikan_berkas_statis(folder_dasar: Teks, jalur_permintaan: Teks) -> ResponsHttp ! Teks`\\n\\n**Pustaka Standar DapCode - Web Framework**\\n\\nMenyajikan aset statis (HTML, CSS, JS, Gambar, Font, dll) secara aman dari direktori dasar dengan deteksi MIME otomatis dan proteksi directory traversal (`..`). Mengarahkan `/` ke `index.html` secara otomatis.".to_string()),
            "PermintaanHttp" => Some("### `struktur PermintaanHttp`\\n\\n**Model Data Permintaan HTTP Bawaan DapCode**\\n\\n- `metode: Teks` (GET, POST, PUT, DELETE, dll)\\n- `jalur: Teks` (contoh: /api/halo)\\n- `kueri: Teks` (query string mentah)\\n- `isi: Teks` (body permintaan)\\n\\n**Metode:**\\n- `.cocok(pola: Teks) -> Bool`\\n- `.param(pola: Teks, nama: Teks) -> Teks ! Teks`\\n- `.kueri_param(kunci: Teks) -> Teks ! Teks`\\n- `.ada_kueri(kunci: Teks) -> Bool`\\n- `.sajikan_statis(folder: Teks) -> ResponsHttp ! Teks`\\n- `.adalah_websocket() -> Bool`\\n- `.kunci_websocket() -> Teks ! Teks`\\n- `.tingkatkan_websocket(nama_penangan: Teks) -> ResponsHttp ! Teks`".to_string()),
            "sha1" => Some("### `fungsi sha1(teks: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - Kriptografi**\\n\\nMenghitung nilai cryptographic hash SHA-1 (FIPS 180-1 / RFC 3174) dari string input dalam bentuk 40 karakter heksadesimal.".to_string()),
            "kunci_websocket" => Some("### `fungsi kunci_websocket(kunci_klien: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - WebSocket RFC 6455**\\n\\nMenghitung nilai header `Sec-WebSocket-Accept` berdasarkan `Sec-WebSocket-Key` klien menggunakan hashing SHA-1 dan pengkodean Base64.".to_string()),
            "respons_upgrade_websocket" => Some("### `fungsi respons_upgrade_websocket(kunci_klien: Teks) -> ResponsHttp ! Teks`\\n\\n**Pustaka Standar DapCode - WebSocket RFC 6455**\\n\\nMembuat objek `ResponsHttp` status 101 Switching Protocols lengkap dengan header `Upgrade: websocket`, `Connection: Upgrade`, dan `Sec-WebSocket-Accept`.".to_string()),
            "buat_server_websocket" => Some("### `fungsi buat_server_websocket(port: u64, nama_penangan: Teks, maks_pesan: u64 = 0) -> u64 ! Teks`\\n\\n**Pustaka Standar DapCode - WebSocket Server**\\n\\nMenjalankan dedicated standalone WebSocket Server RFC 6455 pada port TCP tertentu. Pesan teks yang masuk diteruskan ke fungsi penangan DapCode yang ditentukan.".to_string()),
            "adalah_websocket" => Some("### `metode req.adalah_websocket() -> Bool`\\n\\n**Pustaka Standar DapCode - WebSocket RFC 6455**\\n\\nMemeriksa apakah permintaan HTTP masuk memiliki header upgrade WebSocket (`Upgrade: websocket`).".to_string()),
            "tingkatkan_websocket" => Some("### `metode req.tingkatkan_websocket(nama_penangan: Teks) -> ResponsHttp ! Teks`\\n\\n**Pustaka Standar DapCode - WebSocket RFC 6455**\\n\\nMenyetujui permintaan upgrade WebSocket klien dan mendaftarkan fungsi penangan DapCode untuk memproses frame real-time.".to_string()),
            "ResponsHttp" => Some("### `struktur ResponsHttp`\\n\\n**Model Data Respons HTTP Bawaan DapCode**\\n\\n- `status: u64` (contoh: 200, 404, 500)\\n- `tipe_konten: Teks` (application/json, text/plain, dll)\\n- `konten: Teks` (body respons)\\n- `header: Teks` (header kustom terformat)\\n\\n**Metode:**\\n- `.tambah_header(kunci: Teks, nilai: Teks) -> ResponsHttp`\\n- `.cors() -> ResponsHttp`".to_string()),
            "BasisData" => Some("### `struktur BasisData`\\n\\n**Pustaka Standar DapCode - Basis Data Mandiri**\\n\\nObjek pegangan (*handle*) koneksi basis data persisten lokal DapCode.\\n\\n**Metode:**\\n- `.simpan(kunci: Teks, nilai: Teks) -> Bool ! Teks`\\n- `.ambil(kunci: Teks) -> Teks ! Teks`\\n- `.hapus(kunci: Teks) -> Bool ! Teks`\\n- `.ada(kunci: Teks) -> Bool`\\n- `.daftar_kunci() -> Daftar<Teks> ! Teks`\\n- `.padatkan() -> Bool ! Teks`".to_string()),
            "buka_basis_data" => Some("### `fungsi buka_basis_data(jalur: Teks) -> BasisData ! Teks`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMembuka atau membuat berkas basis data persisten mandiri DapCode pada jalur disk yang ditentukan.".to_string()),
            "db_simpan" => Some("### `fungsi db_simpan(jalur: Teks, kunci: Teks, nilai: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMenyimpan nilai teks/JSON dengan kunci tertentu ke basis data disk.".to_string()),
            "db_ambil" => Some("### `fungsi db_ambil(jalur: Teks, kunci: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMengambil nilai teks/JSON yang tersimpan berdasarkan kunci.".to_string()),
            "db_hapus" => Some("### `fungsi db_hapus(jalur: Teks, kunci: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMenghapus entri data berdasarkan kunci dari basis data.".to_string()),
            "db_ada" => Some("### `fungsi db_ada(jalur: Teks, kunci: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMemeriksa apakah suatu kunci ada di dalam basis data.".to_string()),
            "db_daftar_kunci" => Some("### `fungsi db_daftar_kunci(jalur: Teks) -> Daftar<Teks> ! Teks`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMengambil seluruh daftar kunci aktif yang tersimpan dalam basis data.".to_string()),
            "db_padatkan" => Some("### `fungsi db_padatkan(jalur: Teks) -> Bool ! Teks`\\n\\n**Pustaka Standar DapCode - Basis Data**\\n\\nMemadatkan berkas log basis data append-only untuk menghemat ruang disk.".to_string()),
            "cocok_regex" => Some("### `fungsi cocok_regex(pola: Teks, teks: Teks) -> Bool`\\n\\n**Pustaka Standar DapCode - Reguler Ekspresi (Regex)**\\n\\nMemeriksa apakah string `teks` cocok dengan pola regex `pola`. Mendukung kelas karakter, kuantifier greedy (*, +, ?, {n}), jangkar (^, $), dan alternasi (a|b).\\n\\n*Contoh:* `cocok_regex(\"\\\\d+\", \"Pesanan 101\")` -> `benar`".to_string()),
            "cari_regex" => Some("### `fungsi cari_regex(pola: Teks, teks: Teks) -> Daftar<Teks>`\\n\\n**Pustaka Standar DapCode - Reguler Ekspresi (Regex)**\\n\\nMenemukan seluruh kecocokan pola regex yang tidak tumpang tindih dan mengembalikannya sebagai daftar teks (*list of strings*).\\n\\n*Contoh:* `cari_regex(\"\\\\d+\", \"Kopi 15000, Teh 8000\")` -> `[\"15000\", \"8000\"]`".to_string()),
            "ganti_regex" => Some("### `fungsi ganti_regex(pola: Teks, teks: Teks, pengganti: Teks) -> Teks`\\n\\n**Pustaka Standar DapCode - Reguler Ekspresi (Regex)**\\n\\nMengganti setiap kemunculan pola regex dalam teks dengan string pengganti.\\n\\n*Contoh:* `ganti_regex(\"\\\\d+\", \"Item 123\", \"XXX\")` -> `\"Item XXX\"`".to_string()),
            "ekstrak_regex" => Some("### `fungsi ekstrak_regex(pola: Teks, teks: Teks) -> Teks ! Teks`\\n\\n**Pustaka Standar DapCode - Reguler Ekspresi (Regex)**\\n\\nMengekstrak substring kecocokan pertama dari pola regex atau mengembalikan galat jika tidak ditemukan.\\n\\n*Contoh:* `ekstrak_regex(\"\\\\w+@\\\\w+\\\\.\\\\w+\", \"Email: info@dapcode.com\")` -> `\"info@dapcode.com\"`".to_string()),
            "kanvas_bersihkan" => Some("### `fungsi kanvas_bersihkan(r: f64, g: f64, b: f64, a: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMembersihkan seluruh kanvas dan mengisi latar belakang dengan warna RGBA (nilai 0.0 - 1.0).".to_string()),
            "kanvas_warna_isi" => Some("### `fungsi kanvas_warna_isi(r: f64, g: f64, b: f64, a: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMengatur warna isian (fill style) geometri dengan warna RGBA (nilai 0.0 - 1.0).".to_string()),
            "kanvas_warna_garis" => Some("### `fungsi kanvas_warna_garis(r: f64, g: f64, b: f64, a: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMengatur warna garis/bingkai (stroke style) dengan warna RGBA (nilai 0.0 - 1.0).".to_string()),
            "kanvas_tebal_garis" => Some("### `fungsi kanvas_tebal_garis(tebal: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMengatur ketebalan garis (line width) dalam satuan piksel.".to_string()),
            "kanvas_persegi" => Some("### `fungsi kanvas_persegi(x: f64, y: f64, lebar: f64, tinggi: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenggambar persegi padat terisi warna pada koordinat (x, y) dengan ukuran lebar dan tinggi.".to_string()),
            "kanvas_bingkai_persegi" => Some("### `fungsi kanvas_bingkai_persegi(x: f64, y: f64, lebar: f64, tinggi: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenggambar garis bingkai outline persegi pada koordinat (x, y) dengan ukuran lebar dan tinggi.".to_string()),
            "kanvas_lingkaran" => Some("### `fungsi kanvas_lingkaran(x: f64, y: f64, radius: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenggambar lingkaran padat terisi warna dengan titik pusat (x, y) dan jari-jari radius.".to_string()),
            "kanvas_bingkai_lingkaran" => Some("### `fungsi kanvas_bingkai_lingkaran(x: f64, y: f64, radius: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenggambar garis tepi lingkaran dengan titik pusat (x, y) dan jari-jari radius.".to_string()),
            "kanvas_garis" => Some("### `fungsi kanvas_garis(x1: f64, y1: f64, x2: f64, y2: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenggambar garis lurus dari titik koordinat (x1, y1) menuju (x2, y2).".to_string()),
            "kanvas_titik" => Some("### `fungsi kanvas_titik(x: f64, y: f64, ukuran: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenggambar sebuah titik pada koordinat (x, y) dengan diameter ukuran piksel.".to_string()),
            "kanvas_teks" => Some("### `fungsi kanvas_teks(teks: Teks, x: f64, y: f64, ukuran: f64) -> Unit`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMenuliskan teks string pada koordinat kanvas (x, y) dengan ukuran font dalam piksel.".to_string()),
            "kanvas_lebar" => Some("### `fungsi kanvas_lebar() -> f64`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMengembalikan lebar elemen kanvas aktif dalam satuan piksel.".to_string()),
            "kanvas_tinggi" => Some("### `fungsi kanvas_tinggi() -> f64`\\n\\n**Pustaka Grafis DapCode - Canvas 2D WebAssembly**\\n\\nMengembalikan tinggi elemen kanvas aktif dalam satuan piksel.".to_string()),
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
            ("PI", "PI", 21, "Konstanta matematika Archimedes (π = 3.14159...)"),
            ("TAU", "TAU", 21, "Konstanta lingkaran penuh (τ = 2π = 6.28318...)"),
            ("E", "E", 21, "Konstanta bilangan Euler (e = 2.71828...)"),
            ("sin", "sin($1)", 3, "Sinus sudut dalam radian"),
            ("cos", "cos($1)", 3, "Kosinus sudut dalam radian"),
            ("tan", "tan($1)", 3, "Tangen sudut dalam radian"),
            ("asin", "asin($1)", 3, "Arcsinus invers sudut"),
            ("acos", "acos($1)", 3, "Arckosinus invers sudut"),
            ("atan", "atan($1)", 3, "Arctangen invers sudut"),
            ("atan2", "atan2($1, $2)", 3, "Arctangen koordinat cartesian (y, x)"),
            ("akar_kuadrat", "akar_kuadrat($1)", 3, "Akar kuadrat matematika (√x)"),
            ("akar_kubik", "akar_kubik($1)", 3, "Akar kubik matematika (∛x)"),
            ("pangkat", "pangkat($1, $2)", 3, "Pemangkatan matematika (a^b)"),
            ("nilai_mutlak", "nilai_mutlak($1)", 3, "Nilai absolut matematika (|x|)"),
            ("bulat", "bulat($1)", 3, "Pembulatan ke bilangan bulat terdekat"),
            ("lantai", "lantai($1)", 3, "Pembulatan ke bawah (floor)"),
            ("langit_langit", "langit_langit($1)", 3, "Pembulatan ke atas (ceil)"),
            ("tanda", "tanda($1)", 3, "Tanda bilangan (-1.0, 0.0, 1.0)"),
            ("log", "log($1)", 3, "Logaritma natural basis e"),
            ("log10", "log10($1)", 3, "Logaritma basis 10"),
            ("log2", "log2($1)", 3, "Logaritma basis 2"),
            ("exp", "exp($1)", 3, "Eksponensial e^x"),
            ("min", "min($1, $2)", 3, "Nilai minimum antara dua angka"),
            ("max", "max($1, $2)", 3, "Nilai maksimum antara dua angka"),
            ("ke_radian", "ke_radian($1)", 3, "Konversi sudut derajat ke radian"),
            ("ke_derajat", "ke_derajat($1)", 3, "Konversi sudut radian ke derajat"),
            ("waktu_sekarang_ms", "waktu_sekarang_ms()", 3, "Timestamp milidetik"),
            ("ambil_http", "ambil_http(\"$1\")", 3, "HTTP GET request ke URL"),
            ("kirim_http_post", "kirim_http_post(\"$1\", $2)", 3, "HTTP POST JSON request ke URL"),
            ("ambil_status_http", "ambil_status_http(\"$1\")", 3, "Ambil status code HTTP dari URL"),
            ("buat_server_http", "buat_server_http(${1:8080}, \"${2:tangani_permintaan}\")", 3, "Jalankan HTTP Web Server DapCode"),
            ("kirim_respons_http", "kirim_respons_http(${1:200}, \"${2:application/json}\", ${3:data})", 3, "Buat objek ResponsHttp lengkap"),
            ("respons_json", "respons_json(${1:data_json})", 3, "Buat respons HTTP 200 OK JSON"),
            ("respons_teks", "respons_teks(\"${1:konten}\")", 3, "Buat respons HTTP 200 OK Text"),
            ("tambah_header", "tambah_header(${1:resp}, \"${2:kunci}\", \"${3:nilai}\")", 3, "Tambah header kustom pada ResponsHttp"),
            ("respons_cors", "respons_cors(${1:resp})", 3, "Inject header CORS ke ResponsHttp"),
            ("cocok_rute", "cocok_rute(\"${1:/api/item/:id}\", ${2:req.jalur})", 3, "Periksa kecocokan rute URL dinamis"),
            ("ambil_param_rute", "ambil_param_rute(\"${1:/api/item/:id}\", ${2:req.jalur}, \"${3:id}\")", 3, "Ekstrak parameter rute URL dinamis"),
            ("ambil_kueri_param", "ambil_kueri_param(${1:req.kueri}, \"${2:kunci}\")", 3, "Ekstrak parameter dari query string"),
            ("ada_kueri_param", "ada_kueri_param(${1:req.kueri}, \"${2:kunci}\")", 3, "Periksa keberadaan parameter query string"),
            ("respons_berkas", "respons_berkas(\"${1:jalur_berkas}\")", 3, "Sajikan berkas dengan deteksi MIME otomatis"),
            ("sajikan_berkas_statis", "sajikan_berkas_statis(\"${1:folder_dasar}\", ${2:req.jalur})", 3, "Sajikan aset statis dari folder secara aman"),
            ("buka_basis_data", "buka_basis_data(\"${1:data.db}\")", 3, "Buka atau buat basis data mandiri"),
            ("db_simpan", "db_simpan(\"${1:data.db}\", \"${2:kunci}\", ${3:nilai})", 3, "Simpan data ke basis data"),
            ("db_ambil", "db_ambil(\"${1:data.db}\", \"${2:kunci}\")", 3, "Ambil data dari basis data"),
            ("db_hapus", "db_hapus(\"${1:data.db}\", \"${2:kunci}\")", 3, "Hapus data dari basis data"),
            ("db_ada", "db_ada(\"${1:data.db}\", \"${2:kunci}\")", 3, "Periksa keberadaan kunci basis data"),
            ("db_daftar_kunci", "db_daftar_kunci(\"${1:data.db}\")", 3, "Ambil daftar semua kunci basis data"),
            ("db_padatkan", "db_padatkan(\"${1:data.db}\")", 3, "Padatkan berkas basis data"),
            ("sha256", "sha256(\"$1\")", 3, "Kalkulasi cryptographic hash SHA-256 teks"),
            ("sha1", "sha1(\"$1\")", 3, "Kalkulasi cryptographic hash SHA-1 teks (FIPS 180-1 / RFC 3174)"),
            ("md5", "md5(\"$1\")", 3, "Kalkulasi hash MD5 teks"),
            ("ke_base64", "ke_base64(\"$1\")", 3, "Encode teks ke Base64"),
            ("dari_base64", "dari_base64(\"$1\")", 3, "Decode teks dari Base64"),
            ("kunci_websocket", "kunci_websocket(\"$1\")", 3, "Hitung Sec-WebSocket-Accept dari Sec-WebSocket-Key"),
            ("respons_upgrade_websocket", "respons_upgrade_websocket(\"$1\")", 3, "Buat ResponsHttp 101 Switching Protocols untuk WebSocket"),
            ("buat_server_websocket", "buat_server_websocket(${1:8080}, \"${2:tangani_pesan}\")", 3, "Jalankan dedicated WebSocket Server DapCode"),
            ("adalah_websocket", "adalah_websocket()", 2, "Periksa apakah permintaan adalah Upgrade WebSocket"),
            ("tingkatkan_websocket", "tingkatkan_websocket(\"${1:tangani_pesan}\")", 2, "Tingkatkan permintaan HTTP menjadi koneksi WebSocket dua arah"),
            ("cocok_regex", "cocok_regex(\"${1:pola}\", ${2:teks})", 3, "Periksa kecocokan pola regex pada teks"),
            ("cari_regex", "cari_regex(\"${1:pola}\", ${2:teks})", 3, "Temukan seluruh kemunculan pola regex sebagai Daftar"),
            ("ganti_regex", "ganti_regex(\"${1:pola}\", ${2:teks}, \"${3:pengganti}\")", 3, "Ganti kemunculan pola regex dengan teks baru"),
            ("ekstrak_regex", "ekstrak_regex(\"${1:pola}\", ${2:teks})", 3, "Ekstrak kecocokan pertama pola regex (Error Union)"),
            ("kanvas_bersihkan", "kanvas_bersihkan(${1:0.0}, ${2:0.0}, ${3:0.0}, ${4:1.0})", 3, "Bersihkan kanvas dengan warna latar RGBA"),
            ("kanvas_warna_isi", "kanvas_warna_isi(${1:1.0}, ${2:1.0}, ${3:1.0}, ${4:1.0})", 3, "Atur warna isian bentuk (fill style RGBA)"),
            ("kanvas_warna_garis", "kanvas_warna_garis(${1:1.0}, ${2:1.0}, ${3:1.0}, ${4:1.0})", 3, "Atur warna garis/outline (stroke style RGBA)"),
            ("kanvas_tebal_garis", "kanvas_tebal_garis(${1:2.0})", 3, "Atur ketebalan garis gambar (piksel)"),
            ("kanvas_persegi", "kanvas_persegi(${1:x}, ${2:y}, ${3:lebar}, ${4:tinggi})", 3, "Gambar persegi padat terisi warna"),
            ("kanvas_bingkai_persegi", "kanvas_bingkai_persegi(${1:x}, ${2:y}, ${3:lebar}, ${4:tinggi})", 3, "Gambar garis tepi outline persegi"),
            ("kanvas_lingkaran", "kanvas_lingkaran(${1:x}, ${2:y}, ${3:radius})", 3, "Gambar lingkaran padat terisi warna"),
            ("kanvas_bingkai_lingkaran", "kanvas_bingkai_lingkaran(${1:x}, ${2:y}, ${3:radius})", 3, "Gambar garis tepi lingkaran"),
            ("kanvas_garis", "kanvas_garis(${1:x1}, ${2:y1}, ${3:x2}, ${4:y2})", 3, "Gambar garis lurus dari (x1, y1) ke (x2, y2)"),
            ("kanvas_titik", "kanvas_titik(${1:x}, ${2:y}, ${3:ukuran})", 3, "Gambar sebuah titik dengan diameter piksel"),
            ("kanvas_teks", "kanvas_teks(\"${1:teks}\", ${2:x}, ${3:y}, ${4:16.0})", 3, "Tulis teks pada kanvas dengan ukuran piksel"),
            ("kanvas_lebar", "kanvas_lebar()", 3, "Ambil dimensi lebar kanvas piksel"),
            ("kanvas_tinggi", "kanvas_tinggi()", 3, "Ambil dimensi tinggi kanvas piksel"),
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
        if let Some(text) = self.get_document_text(uri) {
            let mut sm = SourceMap::new();
            let (file_id, _) = sm.add_file(uri, text.clone());
            let mut lexer = Lexer::new(&text, file_id);
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

    pub fn uri_to_path(uri: &str) -> String {
        let mut path = uri.trim();
        if let Some(stripped) = path.strip_prefix("file://") {
            path = stripped;
        }
        if path.starts_with('/') && path.len() >= 3 {
            let bytes = path.as_bytes();
            if bytes[1].is_ascii_alphabetic()
                && (bytes[2] == b':'
                    || path[2..].starts_with("%3A")
                    || path[2..].starts_with("%3a"))
            {
                path = &path[1..];
            }
        }
        path.replace("%3A", ":")
            .replace("%3a", ":")
            .replace("%20", " ")
            .replace('\\', "/")
    }

    pub fn path_to_uri(path: &str) -> String {
        let normalized = path.replace('\\', "/");
        if normalized.starts_with('/') {
            format!("file://{}", normalized)
        } else {
            format!("file:///{}", normalized)
        }
    }

    pub fn get_document_text(&self, uri: &str) -> Option<String> {
        if let Some(text) = self.documents.get(uri) {
            return Some(text.clone());
        }
        let req_path = Self::uri_to_path(uri);
        for (doc_uri, text) in &self.documents {
            if Self::uri_to_path(doc_uri) == req_path {
                return Some(text.clone());
            }
        }
        if let Ok(content) = std::fs::read_to_string(&req_path) {
            return Some(content);
        }
        None
    }

    fn span_to_lsp_range(span: Span, sm: &SourceMap) -> (usize, usize, usize, usize) {
        if let Some(file) = sm.get_file(span.file_id) {
            let (l1, c1) = file.line_col(span.start);
            let (l2, c2) = file.line_col(span.end);
            (
                l1.saturating_sub(1),
                c1.saturating_sub(1),
                l2.saturating_sub(1),
                c2.saturating_sub(1),
            )
        } else {
            (0, 0, 0, 0)
        }
    }

    fn pos_in_lsp_range(
        line: usize,
        col: usize,
        sl: usize,
        sc: usize,
        el: usize,
        ec: usize,
    ) -> bool {
        if line < sl || line > el {
            return false;
        }
        if line == sl && col < sc {
            return false;
        }
        if line == el && col > ec {
            return false;
        }
        true
    }

    fn format_range(sl: usize, sc: usize, el: usize, ec: usize) -> String {
        format!(
            r#"{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}}"#,
            sl, sc, el, ec
        )
    }

    fn format_location(uri: &str, sl: usize, sc: usize, el: usize, ec: usize) -> String {
        let clean_uri = uri.replace('\\', "/");
        format!(
            r#"{{"uri":"{}","range":{}}}"#,
            clean_uri,
            Self::format_range(sl, sc, el, ec)
        )
    }

    fn format_document_symbol(
        name: &str,
        detail: &str,
        kind: usize,
        sl: usize,
        sc: usize,
        el: usize,
        ec: usize,
        children: &[String],
    ) -> String {
        let clean_name = name.replace('\\', "\\\\").replace('"', "\\\"");
        let clean_detail = detail
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ");
        let range = Self::format_range(sl, sc, el, ec);
        let children_part = if children.is_empty() {
            "".to_string()
        } else {
            format!(r#","children":[{}]"#, children.join(","))
        };

        format!(
            r#"{{"name":"{}","detail":"{}","kind":{},"range":{},"selectionRange":{}{}}}"#,
            clean_name, clean_detail, kind, range, range, children_part
        )
    }

    fn find_var_at_pos(
        block: &Block,
        word: &str,
        line: usize,
        col: usize,
        sm: &SourceMap,
    ) -> Option<Span> {
        // 1. Check nested blocks if cursor is inside them
        for stmt in &block.stmts {
            match stmt {
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(then_branch.span, sm);
                    if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                        if let Some(s) = Self::find_var_at_pos(then_branch, word, line, col, sm) {
                            return Some(s);
                        }
                    }
                    if let Some(eb) = else_branch {
                        let (sl, sc, el, ec) = Self::span_to_lsp_range(eb.span, sm);
                        if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                            if let Some(s) = Self::find_var_at_pos(eb, word, line, col, sm) {
                                return Some(s);
                            }
                        }
                    }
                }
                Stmt::ForIn {
                    var_name,
                    span,
                    body,
                    ..
                } => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(body.span, sm);
                    if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                        if let Some(s) = Self::find_var_at_pos(body, word, line, col, sm) {
                            return Some(s);
                        }
                        if var_name == word {
                            return Some(*span);
                        }
                    }
                }
                Stmt::While { body, .. } => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(body.span, sm);
                    if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                        if let Some(s) = Self::find_var_at_pos(body, word, line, col, sm) {
                            return Some(s);
                        }
                    }
                }
                Stmt::Expr(expr) => {
                    if let Some(s) = Self::find_var_in_expr_at_pos(expr, word, line, col, sm) {
                        return Some(s);
                    }
                }
                _ => {}
            }
        }

        // 2. Check local variable declarations in current block
        for stmt in &block.stmts {
            match stmt {
                Stmt::VarDecl { name, span, .. } if name == word => {
                    return Some(*span);
                }
                Stmt::ForIn { var_name, span, .. } if var_name == word => {
                    return Some(*span);
                }
                _ => {}
            }
        }

        None
    }

    fn find_var_in_expr_at_pos(
        expr: &Expr,
        word: &str,
        line: usize,
        col: usize,
        sm: &SourceMap,
    ) -> Option<Span> {
        match expr {
            Expr::Catch {
                err_var,
                handler,
                span,
                ..
            } => {
                let (sl, sc, el, ec) = Self::span_to_lsp_range(handler.span, sm);
                if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                    if let Some(s) = Self::find_var_at_pos(handler, word, line, col, sm) {
                        return Some(s);
                    }
                    if err_var == word {
                        return Some(*span);
                    }
                }
                None
            }
            Expr::Parallel { body, .. } | Expr::Comptime { body, .. } | Expr::Block(body) => {
                let (sl, sc, el, ec) = Self::span_to_lsp_range(body.span, sm);
                if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                    Self::find_var_at_pos(body, word, line, col, sm)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn compute_definition_at(&self, uri: &str, line: usize, col: usize) -> Option<String> {
        let text = self.get_document_text(uri)?;
        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file(uri, text.clone());
        let mut lexer = Lexer::new(&text, file_id);
        let tokens = lexer.tokenize().ok()?;
        let mut parser = Parser::new(tokens, file_id);
        let prog = parser.parse_program().ok()?;

        // 1. Check if cursor is directly on an import declaration
        for decl in &prog.decls {
            if let Decl::Import(imp) = decl {
                let (sl, sc, el, ec) = Self::span_to_lsp_range(imp.span, &sm);
                if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                    let cur_file_path = Self::uri_to_path(uri);
                    let cur_p = std::path::Path::new(&cur_file_path);
                    let target_path = if let Some(parent) = cur_p.parent() {
                        if parent.as_os_str().is_empty() {
                            std::path::PathBuf::from(&imp.path)
                        } else {
                            parent.join(&imp.path)
                        }
                    } else {
                        std::path::PathBuf::from(&imp.path)
                    };
                    let target_uri = Self::path_to_uri(&target_path.to_string_lossy());
                    return Some(Self::format_location(&target_uri, 0, 0, 0, 0));
                }
            }
        }

        // 2. Identify word at cursor
        let word = Self::get_word_at(&text, line, col)?;

        // 3. Search local scopes (function params & local variables in active block)
        for decl in &prog.decls {
            match decl {
                Decl::Fn(f) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(f.span, &sm);
                    if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                        for p in &f.params {
                            if p.name == word {
                                let (psl, psc, pel, pec) = Self::span_to_lsp_range(p.span, &sm);
                                return Some(Self::format_location(uri, psl, psc, pel, pec));
                            }
                        }
                        if let Some(var_span) =
                            Self::find_var_at_pos(&f.body, &word, line, col, &sm)
                        {
                            let (vsl, vsc, vel, vec) = Self::span_to_lsp_range(var_span, &sm);
                            return Some(Self::format_location(uri, vsl, vsc, vel, vec));
                        }
                    }
                }
                Decl::Impl(imp) => {
                    for m in &imp.methods {
                        let (sl, sc, el, ec) = Self::span_to_lsp_range(m.span, &sm);
                        if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                            for p in &m.params {
                                if p.name == word {
                                    let (psl, psc, pel, pec) =
                                        Self::span_to_lsp_range(p.span, &sm);
                                    return Some(Self::format_location(uri, psl, psc, pel, pec));
                                }
                            }
                            if let Some(var_span) =
                                Self::find_var_at_pos(&m.body, &word, line, col, &sm)
                            {
                                let (vsl, vsc, vel, vec) =
                                    Self::span_to_lsp_range(var_span, &sm);
                                return Some(Self::format_location(uri, vsl, vsc, vel, vec));
                            }
                        }
                    }
                }
                Decl::Test(t) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(t.span, &sm);
                    if Self::pos_in_lsp_range(line, col, sl, sc, el, ec) {
                        if let Some(var_span) =
                            Self::find_var_at_pos(&t.body, &word, line, col, &sm)
                        {
                            let (vsl, vsc, vel, vec) = Self::span_to_lsp_range(var_span, &sm);
                            return Some(Self::format_location(uri, vsl, vsc, vel, vec));
                        }
                    }
                }
                _ => {}
            }
        }

        // 4. Search top-level declarations in current file
        for decl in &prog.decls {
            match decl {
                Decl::Fn(f) if f.name == word => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(f.span, &sm);
                    return Some(Self::format_location(uri, sl, sc, el, ec));
                }
                Decl::Struct(s) if s.name == word => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(s.span, &sm);
                    return Some(Self::format_location(uri, sl, sc, el, ec));
                }
                Decl::Struct(s) => {
                    for fld in &s.fields {
                        if fld.name == word {
                            let (sl, sc, el, ec) = Self::span_to_lsp_range(fld.span, &sm);
                            return Some(Self::format_location(uri, sl, sc, el, ec));
                        }
                    }
                }
                Decl::Trait(t) if t.name == word => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(t.span, &sm);
                    return Some(Self::format_location(uri, sl, sc, el, ec));
                }
                Decl::Trait(t) => {
                    for m in &t.methods {
                        if m.name == word {
                            let (sl, sc, el, ec) = Self::span_to_lsp_range(m.span, &sm);
                            return Some(Self::format_location(uri, sl, sc, el, ec));
                        }
                    }
                }
                Decl::Impl(imp) => {
                    for m in &imp.methods {
                        if m.name == word {
                            let (sl, sc, el, ec) = Self::span_to_lsp_range(m.span, &sm);
                            return Some(Self::format_location(uri, sl, sc, el, ec));
                        }
                    }
                }
                _ => {}
            }
        }

        // 5. Cross-file lookup in imported modules
        for decl in &prog.decls {
            if let Decl::Import(imp) = decl {
                let cur_file_path = Self::uri_to_path(uri);
                let cur_p = std::path::Path::new(&cur_file_path);
                let target_path = if let Some(parent) = cur_p.parent() {
                    if parent.as_os_str().is_empty() {
                        std::path::PathBuf::from(&imp.path)
                    } else {
                        parent.join(&imp.path)
                    }
                } else {
                    std::path::PathBuf::from(&imp.path)
                };
                let target_uri = Self::path_to_uri(&target_path.to_string_lossy());
                if let Some(target_text) = self.get_document_text(&target_uri) {
                    let mut imp_sm = SourceMap::new();
                    let (imp_fid, _) = imp_sm.add_file(&target_uri, target_text.clone());
                    let mut imp_lexer = Lexer::new(&target_text, imp_fid);
                    if let Ok(imp_tokens) = imp_lexer.tokenize() {
                        let mut imp_parser = Parser::new(imp_tokens, imp_fid);
                        if let Ok(imp_prog) = imp_parser.parse_program() {
                            for d in &imp_prog.decls {
                                match d {
                                    Decl::Fn(f) if f.name == word => {
                                        let (sl, sc, el, ec) =
                                            Self::span_to_lsp_range(f.span, &imp_sm);
                                        return Some(Self::format_location(
                                            &target_uri,
                                            sl,
                                            sc,
                                            el,
                                            ec,
                                        ));
                                    }
                                    Decl::Struct(s) if s.name == word => {
                                        let (sl, sc, el, ec) =
                                            Self::span_to_lsp_range(s.span, &imp_sm);
                                        return Some(Self::format_location(
                                            &target_uri,
                                            sl,
                                            sc,
                                            el,
                                            ec,
                                        ));
                                    }
                                    Decl::Trait(t) if t.name == word => {
                                        let (sl, sc, el, ec) =
                                            Self::span_to_lsp_range(t.span, &imp_sm);
                                        return Some(Self::format_location(
                                            &target_uri,
                                            sl,
                                            sc,
                                            el,
                                            ec,
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn compute_document_symbols(&self, uri: &str) -> String {
        let text = match self.get_document_text(uri) {
            Some(t) => t,
            None => return "".to_string(),
        };

        let mut sm = SourceMap::new();
        let (file_id, _) = sm.add_file(uri, text.clone());
        let mut lexer = Lexer::new(&text, file_id);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(_) => return "".to_string(),
        };
        let mut parser = Parser::new(tokens, file_id);
        let prog = match parser.parse_program() {
            Ok(p) => p,
            Err(_) => return "".to_string(),
        };

        let mut symbols = Vec::new();

        for decl in &prog.decls {
            match decl {
                Decl::Fn(f) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(f.span, &sm);
                    let params = f
                        .params
                        .iter()
                        .map(|p| {
                            if p.is_self {
                                "ini".to_string()
                            } else if let Some(ref ty) = p.ty {
                                format!("{}: {}", p.name, crate::doc::format_type_expr(ty))
                            } else {
                                p.name.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let ret = f
                        .ret_ty
                        .as_ref()
                        .map(|t| format!(" -> {}", crate::doc::format_type_expr(t)))
                        .unwrap_or_else(|| " -> Unit".to_string());
                    let detail = format!("({}){}", params, ret);
                    symbols.push(Self::format_document_symbol(
                        &f.name, &detail, 12, sl, sc, el, ec, &[],
                    ));
                }
                Decl::Struct(s) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(s.span, &sm);
                    let mut field_symbols = Vec::new();
                    for field in &s.fields {
                        let (fsl, fsc, fel, fec) = Self::span_to_lsp_range(field.span, &sm);
                        let detail = crate::doc::format_type_expr(&field.ty);
                        field_symbols.push(Self::format_document_symbol(
                            &field.name,
                            &detail,
                            8,
                            fsl,
                            fsc,
                            fel,
                            fec,
                            &[],
                        ));
                    }
                    symbols.push(Self::format_document_symbol(
                        &s.name,
                        "struktur",
                        23,
                        sl,
                        sc,
                        el,
                        ec,
                        &field_symbols,
                    ));
                }
                Decl::Trait(t) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(t.span, &sm);
                    let mut method_symbols = Vec::new();
                    for m in &t.methods {
                        let (msl, msc, mel, mec) = Self::span_to_lsp_range(m.span, &sm);
                        let params = m
                            .params
                            .iter()
                            .map(|p| {
                                if p.is_self {
                                    "ini".to_string()
                                } else if let Some(ref ty) = p.ty {
                                    format!("{}: {}", p.name, crate::doc::format_type_expr(ty))
                                } else {
                                    p.name.clone()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let ret = m
                            .ret_ty
                            .as_ref()
                            .map(|t| format!(" -> {}", crate::doc::format_type_expr(t)))
                            .unwrap_or_else(|| " -> Unit".to_string());
                        let detail = format!("({}){}", params, ret);
                        method_symbols.push(Self::format_document_symbol(
                            &m.name, &detail, 6, msl, msc, mel, mec, &[],
                        ));
                    }
                    symbols.push(Self::format_document_symbol(
                        &t.name,
                        "sifat",
                        11,
                        sl,
                        sc,
                        el,
                        ec,
                        &method_symbols,
                    ));
                }
                Decl::Impl(imp) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(imp.span, &sm);
                    let name = if let Some(ref tr) = imp.trait_name {
                        format!("terapkan {} untuk {}", tr, imp.target_struct)
                    } else {
                        format!("terapkan {}", imp.target_struct)
                    };
                    let mut method_symbols = Vec::new();
                    for m in &imp.methods {
                        let (msl, msc, mel, mec) = Self::span_to_lsp_range(m.span, &sm);
                        let params = m
                            .params
                            .iter()
                            .map(|p| {
                                if p.is_self {
                                    "ini".to_string()
                                } else if let Some(ref ty) = p.ty {
                                    format!("{}: {}", p.name, crate::doc::format_type_expr(ty))
                                } else {
                                    p.name.clone()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let ret = m
                            .ret_ty
                            .as_ref()
                            .map(|t| format!(" -> {}", crate::doc::format_type_expr(t)))
                            .unwrap_or_else(|| " -> Unit".to_string());
                        let detail = format!("({}){}", params, ret);
                        method_symbols.push(Self::format_document_symbol(
                            &m.name, &detail, 6, msl, msc, mel, mec, &[],
                        ));
                    }
                    symbols.push(Self::format_document_symbol(
                        &name,
                        "implementasi",
                        5,
                        sl,
                        sc,
                        el,
                        ec,
                        &method_symbols,
                    ));
                }
                Decl::Test(test) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(test.span, &sm);
                    let name = format!("uji \"{}\"", test.name);
                    symbols.push(Self::format_document_symbol(
                        &name,
                        "pengujian unit",
                        12,
                        sl,
                        sc,
                        el,
                        ec,
                        &[],
                    ));
                }
                Decl::Import(imp) => {
                    let (sl, sc, el, ec) = Self::span_to_lsp_range(imp.span, &sm);
                    let name = format!("impor \"{}\"", imp.path);
                    symbols.push(Self::format_document_symbol(
                        &name,
                        "modul eksternal",
                        3,
                        sl,
                        sc,
                        el,
                        ec,
                        &[],
                    ));
                }
            }
        }

        symbols.join(",")
    }

    pub fn compute_code_actions(&self, params: Option<&JsonValue>) -> String {
        let Some(params) = params else { return String::new(); };
        let uri = params
            .get("textDocument")
            .and_then(|t| t.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        if uri.is_empty() {
            return String::new();
        }

        let text = match self.get_document_text(uri) {
            Some(t) => t,
            None => return String::new(),
        };

        struct DiagItem {
            message: String,
            start_line: usize,
            start_col: usize,
        }

        let mut diag_items = Vec::new();
        if let Some(JsonValue::Array(diag_arr)) = params.get("context").and_then(|c| c.get("diagnostics")) {
            for d in diag_arr {
                let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
                let sl = d.get("range").and_then(|r| r.get("start")).and_then(|s| s.get("line")).and_then(|l| l.as_f64()).map(|n| n as usize).unwrap_or(0);
                let sc = d.get("range").and_then(|r| r.get("start")).and_then(|s| s.get("character")).and_then(|c| c.as_f64()).map(|n| n as usize).unwrap_or(0);
                if !msg.is_empty() {
                    diag_items.push(DiagItem {
                        message: msg.to_string(),
                        start_line: sl,
                        start_col: sc,
                    });
                }
            }
        }

        // If no diagnostics were provided in context, compute diagnostics from source
        if diag_items.is_empty() {
            let mut sm = SourceMap::new();
            let (file_id, _) = sm.add_file(uri, text.clone());
            let mut lexer = Lexer::new(&text, file_id);
            if let Ok(tokens) = lexer.tokenize() {
                let mut parser = Parser::new(tokens, file_id);
                match parser.parse_program() {
                    Ok(prog) => {
                        let mut checker = TypeChecker::new(file_id);
                        if let Err(diag) = checker.check_program(&prog) {
                            let (sl, sc, _, _) = diag.span.map(|s| Self::span_to_lsp_range(s, &sm)).unwrap_or((0, 0, 0, 0));
                            diag_items.push(DiagItem {
                                message: diag.message,
                                start_line: sl,
                                start_col: sc,
                            });
                        }
                    }
                    Err(diag) => {
                        let (sl, sc, _, _) = diag.span.map(|s| Self::span_to_lsp_range(s, &sm)).unwrap_or((0, 0, 0, 0));
                        diag_items.push(DiagItem {
                            message: diag.message,
                            start_line: sl,
                            start_col: sc,
                        });
                    }
                }
            } else if let Err(diag) = lexer.tokenize() {
                let (sl, sc, _, _) = diag.span.map(|s| Self::span_to_lsp_range(s, &sm)).unwrap_or((0, 0, 0, 0));
                diag_items.push(DiagItem {
                    message: diag.message,
                    start_line: sl,
                    start_col: sc,
                });
            }
        }

        let mut candidate_symbols: Vec<String> = vec![
            // Math
            "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "log", "log10", "log2", "exp",
            "akar_kuadrat", "akar_kubik", "pangkat", "nilai_mutlak", "bulat", "lantai", "langit_langit",
            "tanda", "min", "max", "ke_radian", "ke_derajat", "PI", "TAU", "E",
            // Stdlib
            "cetak", "panjang", "tambah", "ke_teks", "ke_angka", "ke_json", "uraikan_json",
            "ambil_kunci_json", "ambil_angka_json", "ambil_bool_json", "ada_kunci_json", "valid_json",
            "baca_berkas", "tulis_berkas", "ada_berkas", "hapus_berkas", "buat_direktori",
            "daftar_direktori", "hapus_direktori", "gabung_jalur", "tidur", "ambil_argumen_cli",
            "ambil_lingkungan", "atur_lingkungan", "waktu_sekarang_ms", "ambil_http", "kirim_http_post",
            "ambil_status_http", "buat_server_http", "kirim_respons_http", "respons_json", "respons_teks",
            "tambah_header", "respons_cors", "cocok_rute", "ambil_param_rute", "ambil_kueri_param",
            "ada_kueri_param", "respons_berkas", "sajikan_berkas_statis", "buka_basis_data",
            "db_simpan", "db_ambil", "db_hapus", "db_ada", "db_daftar_kunci", "db_padatkan",
            "sha256", "sha1", "md5", "ke_base64", "dari_base64", "kunci_websocket",
            "respons_upgrade_websocket", "buat_server_websocket", "adalah_websocket",
            "tingkatkan_websocket", "cocok_regex", "cari_regex", "ganti_regex", "ekstrak_regex",
            "pastikan", "pastikan_sama", "pastikan_tidak_sama",
            // Keywords & Types
            "fungsi", "struktur", "sifat", "terapkan", "untuk", "dalam", "jika", "selain_itu",
            "selama", "kembali", "gagal", "tangkap", "jalan_paralel", "tunggu", "comptime",
            "impor", "ini", "benar", "salah", "uji", "u64", "i64", "f64", "Teks", "Bool", "Unit", "Daftar",
        ].into_iter().map(|s| s.to_string()).collect();

        // Also gather identifiers from the document tokens
        let mut sm_tokens = SourceMap::new();
        let (fid, _) = sm_tokens.add_file(uri, text.clone());
        let mut token_lexer = Lexer::new(&text, fid);
        if let Ok(toks) = token_lexer.tokenize() {
            for tok in toks {
                if let crate::lexer::TokenKind::Ident(ref name) = tok.kind {
                    if !candidate_symbols.contains(name) {
                        candidate_symbols.push(name.clone());
                    }
                }
            }
        }

        let mut actions: Vec<String> = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        let add_action = |actions: &mut Vec<String>, title: &str, kind: &str, is_preferred: bool, sl: usize, sc: usize, el: usize, ec: usize, new_text: &str| {
            let act_json = format!(
                r#"{{"title":"{}","kind":"{}","isPreferred":{},"edit":{{"changes":{{"{}":[{{"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"newText":"{}"}}]}}}}}}"#,
                Self::escape_lsp_json(title),
                Self::escape_lsp_json(kind),
                is_preferred,
                Self::escape_lsp_json(uri),
                sl,
                sc,
                el,
                ec,
                Self::escape_lsp_json(new_text)
            );
            if !actions.iter().any(|a| a.contains(&format!("\"title\":\"{}\"", Self::escape_lsp_json(title)))) {
                actions.push(act_json);
            }
        };

        for diag in &diag_items {
            // Check for missing 'utama()'
            if diag.message.contains("utama()") || diag.message.contains("Fungsi 'utama()' tidak ditemukan") {
                let last_line = lines.len();
                let stub = if text.ends_with('\n') {
                    "\nfungsi utama() {\n    cetak(\"Halo dari DapCode!\")\n}\n"
                } else {
                    "\n\nfungsi utama() {\n    cetak(\"Halo dari DapCode!\")\n}\n"
                };
                add_action(
                    &mut actions,
                    "Tambahkan fungsi 'utama()'",
                    "quickfix",
                    true,
                    last_line,
                    0,
                    last_line,
                    0,
                    stub,
                );
            }

            // Extract identifier from quotes e.g. 'XYZ'
            if let Some(typo) = Self::extract_quoted_identifier(&diag.message) {
                let target_line_idx = diag.start_line;
                let line_str = lines.get(target_line_idx).copied().unwrap_or("");

                // Locate typo position on target line
                let typo_range = Self::find_identifier_range_on_line(line_str, &typo, diag.start_col)
                    .unwrap_or((diag.start_col, diag.start_col + typo.len()));

                // 1. Check for assignment `=` instead of `:=`
                // e.g. line has: <typo> = <val>
                if let Some(eq_col) = Self::find_single_equals_on_line(line_str, typo_range.1) {
                    add_action(
                        &mut actions,
                        "Ganti '=' dengan ':=' untuk mendeklarasikan variabel baru",
                        "quickfix",
                        true,
                        target_line_idx,
                        eq_col,
                        target_line_idx,
                        eq_col + 1,
                        ":=",
                    );
                }

                // 2. Levenshtein typo suggestions
                let mut matches: Vec<(usize, &String)> = candidate_symbols
                    .iter()
                    .filter(|c| c.as_str() != typo.as_str())
                    .filter_map(|c| {
                        let dist = Self::levenshtein(&typo, c);
                        let max_allowed = if typo.len() <= 3 { 1 } else if typo.len() <= 6 { 2 } else { 3 };
                        if dist <= max_allowed {
                            Some((dist, c))
                        } else {
                            None
                        }
                    })
                    .collect();

                matches.sort_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then_with(|| (a.1.len() as isize - typo.len() as isize).abs().cmp(&(b.1.len() as isize - typo.len() as isize).abs()))
                });

                for (idx, (dist, cand)) in matches.iter().take(3).enumerate() {
                    let is_pref = idx == 0 && *dist <= 2;
                    let title = format!("Perbaiki ejaan menjadi '{}'", cand);
                    add_action(
                        &mut actions,
                        &title,
                        "quickfix",
                        is_pref,
                        target_line_idx,
                        typo_range.0,
                        target_line_idx,
                        typo_range.1,
                        cand,
                    );
                }

                // 3. Auto-stub function if it looks like a function call `typo(`
                let after_typo = if typo_range.1 < line_str.len() { &line_str[typo_range.1..] } else { "" };
                if after_typo.trim_start().starts_with('(') {
                    let last_line = lines.len();
                    let stub = format!("\n\nfungsi {}() {{\n    // TODO: implementasi fungsi {}\n}}\n", typo, typo);
                    let title = format!("Buat stub fungsi '{}()'", typo);
                    add_action(
                        &mut actions,
                        &title,
                        "quickfix",
                        false,
                        last_line,
                        0,
                        last_line,
                        0,
                        &stub,
                    );
                }
            }
        }

        // Formatting document action
        let fmt_edit = self.compute_formatting(uri);
        if !fmt_edit.is_empty() {
            let act_json = format!(
                r#"{{"title":"Format dokumen DapCode","kind":"source.fixAll","isPreferred":false,"edit":{{"changes":{{"{}":[{}]}}}}}}"#,
                Self::escape_lsp_json(uri),
                fmt_edit
            );
            actions.push(act_json);
        }

        actions.join(",")
    }

    pub fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();

        let mut dp = vec![vec![0; n + 1]; m + 1];

        for i in 0..=m {
            dp[i][0] = i;
        }
        for j in 0..=n {
            dp[0][j] = j;
        }

        for i in 1..=m {
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                dp[i][j] = (dp[i - 1][j] + 1)
                    .min(dp[i][j - 1] + 1)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[m][n]
    }

    pub fn extract_quoted_identifier(msg: &str) -> Option<String> {
        if let Some(start) = msg.find('\'') {
            let rest = &msg[start + 1..];
            if let Some(end) = rest.find('\'') {
                let mut id = &rest[..end];
                if id.ends_with("()") {
                    id = &id[..id.len() - 2];
                }
                if !id.is_empty() {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    pub fn find_identifier_range_on_line(line_str: &str, ident: &str, target_col: usize) -> Option<(usize, usize)> {
        let ident_len = ident.chars().count();
        let chars: Vec<char> = line_str.chars().collect();
        let n = chars.len();
        let ident_chars: Vec<char> = ident.chars().collect();

        if ident_len == 0 || ident_len > n {
            return None;
        }

        let mut matches = Vec::new();
        for i in 0..=n - ident_len {
            if chars[i..i + ident_len] == ident_chars[..] {
                let prev_ok = i == 0 || (!chars[i - 1].is_alphanumeric() && chars[i - 1] != '_');
                let next_ok = (i + ident_len == n) || (!chars[i + ident_len].is_alphanumeric() && chars[i + ident_len] != '_');
                if prev_ok && next_ok {
                    matches.push(i);
                }
            }
        }

        if matches.is_empty() {
            for i in 0..=n - ident_len {
                if chars[i..i + ident_len] == ident_chars[..] {
                    matches.push(i);
                }
            }
        }

        matches.into_iter().min_by_key(|&pos| (pos as isize - target_col as isize).abs())
            .map(|pos| (pos, pos + ident_len))
    }

    pub fn find_single_equals_on_line(line_str: &str, after_col: usize) -> Option<usize> {
        let chars: Vec<char> = line_str.chars().collect();
        let start = after_col.min(chars.len());
        for i in start..chars.len() {
            if chars[i] == '=' {
                let prev = if i > 0 { chars[i - 1] } else { ' ' };
                let next = if i + 1 < chars.len() { chars[i + 1] } else { ' ' };
                if prev != '=' && prev != '!' && prev != '<' && prev != '>' && prev != ':' && next != '=' && next != '>' {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn escape_lsp_json(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }
}
