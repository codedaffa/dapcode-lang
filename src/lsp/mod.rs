use crate::ast::*;
use crate::fmt::Formatter;
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
                    // Consume empty line \r\n
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

    fn send_response(&self, content: &str) {
        let msg = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(msg.as_bytes());
        let _ = stdout.flush();
    }

    fn handle_message(&mut self, body: &str) {
        // Simple JSON parsing without heavy dependencies
        if body.contains("\"method\":\"initialize\"") {
            let id = self.extract_id(body).unwrap_or(1);
            let resp = format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{{"capabilities":{{"textDocumentSync":1,"hoverProvider":true,"completionProvider":{{"triggerCharacters":[".",":"]}},"documentFormattingProvider":true}}}}}}"#,
                id
            );
            self.send_response(&resp);
        } else if body.contains("\"method\":\"textDocument/didOpen\"") {
            if let Some(uri) = self.extract_string_field(body, "uri") {
                if let Some(text) = self.extract_string_field(body, "text") {
                    self.documents.insert(uri.clone(), text.clone());
                    self.publish_diagnostics(&uri, &text);
                }
            }
        } else if body.contains("\"method\":\"textDocument/didChange\"") {
            if let Some(uri) = self.extract_string_field(body, "uri") {
                if let Some(text) = self.extract_string_field(body, "text") {
                    self.documents.insert(uri.clone(), text.clone());
                    self.publish_diagnostics(&uri, &text);
                }
            }
        } else if body.contains("\"method\":\"textDocument/hover\"") {
            let id = self.extract_id(body).unwrap_or(1);
            let uri = self.extract_string_field(body, "uri").unwrap_or_default();
            let hover_info = self.compute_hover(&uri);
            let resp = format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{{"contents":{{"kind":"markdown","value":"{}"}}}}}}"#,
                id, hover_info
            );
            self.send_response(&resp);
        } else if body.contains("\"method\":\"textDocument/completion\"") {
            let id = self.extract_id(body).unwrap_or(1);
            let completions = self.compute_completions();
            let resp = format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":[{}]}}"#,
                id, completions
            );
            self.send_response(&resp);
        } else if body.contains("\"method\":\"textDocument/formatting\"") {
            let id = self.extract_id(body).unwrap_or(1);
            let uri = self.extract_string_field(body, "uri").unwrap_or_default();
            let edit_json = self.compute_formatting(&uri);
            let resp = format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":[{}]}}"#,
                id, edit_json
            );
            self.send_response(&resp);
        } else if body.contains("\"method\":\"shutdown\"") {
            let id = self.extract_id(body).unwrap_or(1);
            let resp = format!(r#"{{"jsonrpc":"2.0","id":{},"result":null}}"#, id);
            self.send_response(&resp);
        }
    }

    fn publish_diagnostics(&self, uri: &str, text: &str) {
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

        let notification = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":"{}","diagnostics":[{}]}}}}"#,
            uri,
            diag_list.join(",")
        );
        self.send_response(&notification);
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

    fn compute_hover(&self, uri: &str) -> String {
        if let Some(text) = self.documents.get(uri) {
            let mut sm = SourceMap::new();
            let (file_id, _) = sm.add_file(uri, text.clone());
            let mut lexer = Lexer::new(text, file_id);
            if let Ok(tokens) = lexer.tokenize() {
                let mut parser = Parser::new(tokens, file_id);
                if let Ok(prog) = parser.parse_program() {
                    let mut summary = String::from("### DapCode Module\\n\\n");
                    for decl in &prog.decls {
                        match decl {
                            Decl::Struct(s) => {
                                summary.push_str(&format!("- **struktur** `{}`\\n", s.name));
                            }
                            Decl::Trait(t) => {
                                summary.push_str(&format!("- **sifat** `{}`\\n", t.name));
                            }
                            Decl::Fn(f) => {
                                summary.push_str(&format!("- **fungsi** `{}`\\n", f.name));
                            }
                            _ => {}
                        }
                    }
                    return summary;
                }
            }
        }
        "DapCode Symbol".to_string()
    }

    fn compute_completions(&self) -> String {
        let keywords = [
            ("struktur", "Kata kunci deklarasi struktur"),
            ("sifat", "Kata kunci deklarasi sifat/trait"),
            ("terapkan", "Kata kunci implementasi struktur/trait"),
            ("untuk", "Kata kunci penunjuk target trait"),
            ("fungsi", "Kata kunci definisi fungsi"),
            ("jika", "Percabangan jika (if)"),
            ("selain_itu", "Percabangan alternatif (else)"),
            ("selama", "Perulangan selama (while)"),
            ("kembali", "Kembalikan nilai (return)"),
            ("gagal", "Lemparkan kesalahan pada Error Union"),
            ("tangkap", "Blok pemulihan kesalahan (catch)"),
            ("jalan_paralel", "Jalankan coroutine asinkron/paralel"),
            ("tunggu", "Sinkronisasi/tunggu tugas paralel"),
            ("comptime", "Evaluasi saat kompilasi"),
            ("ini", "Referensi penerima metode (self)"),
            ("benar", "Konstanta boolean true"),
            ("salah", "Konstanta boolean false"),
            ("cetak", "Fungsi bawaan untuk mencetak ke konsol"),
            ("u64", "Tipe integer 64-bit unsigned"),
            ("f64", "Tipe float 64-bit"),
            ("Teks", "Tipe teks / string DapCode"),
            ("Bool", "Tipe boolean (benar / salah)"),
        ];

        let mut items = Vec::new();
        for (kw, doc) in keywords {
            items.push(format!(
                r#"{{"label":"{}","kind":14,"detail":"{}","insertText":"{}"}}"#,
                kw, doc, kw
            ));
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
                    let clean = formatted.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
                    let lines_count = text.lines().count();
                    return format!(
                        r#"{{"range":{{"start":{{"line":0,"character":0}},"end":{{"line":{},"character":0}}}},"newText":"{}"}}"#,
                        lines_count + 1, clean
                    );
                }
            }
        }
        "".to_string()
    }

    fn extract_id(&self, body: &str) -> Option<usize> {
        if let Some(idx) = body.find("\"id\":") {
            let after = &body[idx + 5..];
            let end_idx = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
            after[..end_idx].trim().parse().ok()
        } else {
            None
        }
    }

    fn extract_string_field(&self, body: &str, field: &str) -> Option<String> {
        let pattern = format!("\"{}\":\"", field);
        if let Some(idx) = body.find(&pattern) {
            let start = idx + pattern.len();
            let after = &body[start..];
            let mut result = String::new();
            let mut chars = after.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(next_c) = chars.next() {
                        result.push(next_c);
                    }
                } else if c == '"' {
                    break;
                } else {
                    result.push(c);
                }
            }
            Some(result)
        } else {
            None
        }
    }
}
