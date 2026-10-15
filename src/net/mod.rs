use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

/// Melakukan HTTP GET request ke URL (mendukung http:// dan https://).
pub fn ambil_http(url: &str) -> Result<String, String> {
    if url.starts_with("https://") {
        http_curl_request("GET", url, None)
    } else if url.starts_with("http://") {
        http_tcp_request("GET", url, None, None)
    } else {
        Err(format!("Protokol URL tidak didukung (harus http:// atau https://): {}", url))
    }
}

/// Melakukan HTTP POST request dengan payload JSON (Content-Type: application/json).
pub fn kirim_http_post(url: &str, body: &str) -> Result<String, String> {
    if url.starts_with("https://") {
        http_curl_request("POST", url, Some(body))
    } else if url.starts_with("http://") {
        http_tcp_request("POST", url, Some(body), Some("application/json"))
    } else {
        Err(format!("Protokol URL tidak didukung (harus http:// atau https://): {}", url))
    }
}

/// Mengambil status code numerik dari HTTP request (misal 200, 404, 500).
pub fn ambil_status_http(url: &str) -> Result<u64, String> {
    if url.starts_with("https://") {
        let mut cmd = Command::new("curl.exe");
        cmd.args(&["-s", "-S", "-o", "nul", "-w", "%{http_code}", "-X", "GET", url]);
        let output = match cmd.output() {
            Ok(out) => out,
            Err(_) => Command::new("curl")
                .args(&["-s", "-S", "-o", "/dev/null", "-w", "%{http_code}", "-X", "GET", url])
                .output()
                .map_err(|e| format!("Gagal menjalankan curl: {}", e))?,
        };
        let code_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        code_str.parse::<u64>().map_err(|_| format!("Gagal menguraikan status code HTTP: {}", code_str))
    } else if url.starts_with("http://") {
        let (host, port, path) = parse_url(url)?;
        let mut stream = TcpStream::connect((host.as_str(), port))
            .map_err(|e| format!("Gagal terhubung ke {}:{}: {}", host, port, e))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;

        let req = format!(
            "HEAD {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: DapCode/0.1\r\nConnection: close\r\n\r\n",
            path, host
        );
        stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let resp = String::from_utf8_lossy(&buf);
        let first_line = resp.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            parts[1].parse::<u64>().map_err(|_| format!("Format status line tidak valid: {}", first_line))
        } else {
            Err(format!("Respons HTTP tidak valid: {}", first_line))
        }
    } else {
        Err(format!("Protokol URL tidak didukung: {}", url))
    }
}

// --- Implementasi Internal TCP Socket HTTP/1.1 ---

fn http_tcp_request(
    method: &str,
    url: &str,
    body: Option<&str>,
    content_type: Option<&str>,
) -> Result<String, String> {
    let (host, port, path) = parse_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))
        .map_err(|e| format!("Koneksi jaringan gagal ke {}:{}: {}", host, port, e))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;

    let mut req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: DapCode/0.1\r\nConnection: close\r\nAccept: */*\r\n",
        method, path, host
    );

    if let Some(ct) = content_type {
        req.push_str(&format!("Content-Type: {}\r\n", ct));
    }

    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n\r\n{}", b.len(), b));
    } else {
        req.push_str("\r\n");
    }

    stream.write_all(req.as_bytes()).map_err(|e| format!("Gagal mengirim request: {}", e))?;

    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(|e| format!("Gagal membaca respons HTTP: {}", e))?;

    let response_str = String::from_utf8_lossy(&response_bytes).to_string();

    // Pisahkan header dan body (separator \r\n\r\n atau \n\n)
    let (header_str, body_str) = if let Some(idx) = response_str.find("\r\n\r\n") {
        (&response_str[..idx], &response_str[idx + 4..])
    } else if let Some(idx) = response_str.find("\n\n") {
        (&response_str[..idx], &response_str[idx + 2..])
    } else {
        ("", response_str.as_str())
    };

    // Parse status line
    let first_line = header_str.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let status_code: u64 = if parts.len() >= 2 {
        parts[1].parse().unwrap_or(200)
    } else {
        200
    };

    // Decode chunked transfer encoding jika ada
    let final_body = if header_str.to_lowercase().contains("transfer-encoding: chunked") {
        decode_chunked_body(body_str)
    } else {
        body_str.to_string()
    };

    if status_code >= 400 {
        Err(format!("HTTP Error {}: {}", status_code, final_body.trim()))
    } else {
        Ok(final_body)
    }
}

fn decode_chunked_body(chunked: &str) -> String {
    let mut result = String::new();
    let mut lines = chunked.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(size) = usize::from_str_radix(line, 16) {
            if size == 0 {
                break;
            }
            if let Some(data) = lines.next() {
                result.push_str(data);
            }
        }
    }
    result
}

fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let without_proto = if let Some(stripped) = url.strip_prefix("http://") {
        stripped
    } else if let Some(stripped) = url.strip_prefix("https://") {
        stripped
    } else {
        return Err("URL harus diawali dengan http:// atau https://".to_string());
    };

    let (host_port, path) = match without_proto.find('/') {
        Some(idx) => (&without_proto[..idx], &without_proto[idx..]),
        None => (without_proto, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(idx) => {
            let h = &host_port[..idx];
            let p = host_port[idx + 1..]
                .parse::<u16>()
                .map_err(|_| format!("Port tidak valid dalam URL: {}", host_port))?;
            (h.to_string(), p)
        }
        None => {
            let default_port = if url.starts_with("https://") { 443 } else { 80 };
            (host_port.to_string(), default_port)
        }
    };

    Ok((host, port, path.to_string()))
}

// --- Implementasi HTTPS via Native System Curl ---

fn http_curl_request(method: &str, url: &str, body: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("curl.exe");
    let mut args = vec!["-s", "-S", "-L", "-w", "\n%{http_code}", "-X", method];

    if let Some(b) = body {
        args.push("-H");
        args.push("Content-Type: application/json");
        args.push("-d");
        args.push(b);
    }
    args.push(url);

    cmd.args(&args);

    let output = match cmd.output() {
        Ok(out) => out,
        Err(_) => {
            // Fallback to "curl"
            let mut fallback_cmd = Command::new("curl");
            fallback_cmd.args(&args);
            fallback_cmd
                .output()
                .map_err(|e| format!("Gagal memanggil curl: {}", e))?
        }
    };

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(if err.is_empty() {
            "Gagal melakukan permintaan jaringan HTTP".to_string()
        } else {
            err
        });
    }

    let full_out = String::from_utf8_lossy(&output.stdout).to_string();
    let (resp_body, status_code) = parse_curl_output(&full_out);

    if status_code >= 400 || status_code == 0 {
        Err(format!("HTTP Error {}: {}", status_code, resp_body.trim()))
    } else {
        Ok(resp_body)
    }
}

fn parse_curl_output(output: &str) -> (String, u64) {
    if let Some(idx) = output.rfind('\n') {
        let body = &output[..idx];
        let code_str = output[idx + 1..].trim();
        let status = code_str.parse::<u64>().unwrap_or(200);
        (body.to_string(), status)
    } else {
        (output.to_string(), 200)
    }
}

// ==========================================
// --- HTTP Web Server Core Engine ---
// ==========================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServerRequest {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServerResponse {
    pub status: u64,
    pub content_type: String,
    pub body: String,
    pub raw_body: Option<Vec<u8>>,
    pub headers: std::collections::HashMap<String, String>,
}

impl Default for HttpServerResponse {
    fn default() -> Self {
        HttpServerResponse {
            status: 200,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: String::new(),
            raw_body: None,
            headers: std::collections::HashMap::new(),
        }
    }
}

pub fn parse_http_request(raw: &[u8]) -> Result<HttpServerRequest, String> {
    let text = String::from_utf8_lossy(raw);
    let mut lines = text.split("\r\n");
    let req_line = lines.next().ok_or("Permintaan HTTP kosong")?;
    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(format!("Baris permintaan HTTP tidak valid: {}", req_line));
    }

    let method = parts[0].to_uppercase();
    let full_uri = parts[1];
    let (path, query) = if let Some((p, q)) = full_uri.split_once('?') {
        (p.to_string(), q.to_string())
    } else {
        (full_uri.to_string(), String::new())
    };

    let mut headers = std::collections::HashMap::new();
    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }

    let body = if let Some(sep) = text.find("\r\n\r\n") {
        text[sep + 4..].to_string()
    } else {
        String::new()
    };

    Ok(HttpServerRequest {
        method,
        path,
        query,
        headers,
        body,
    })
}

pub fn format_http_response(resp: &HttpServerResponse) -> Vec<u8> {
    if resp.status == 101 {
        let mut header = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nServer: DapCode-HTTP/1.0\r\n"
        );
        for (k, v) in &resp.headers {
            if !k.starts_with('_') {
                header.push_str(&format!("{}: {}\r\n", k, v));
            }
        }
        header.push_str("\r\n");
        return header.into_bytes();
    }

    let status_text = match resp.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    };

    let body_bytes: &[u8] = match &resp.raw_body {
        Some(bytes) => bytes.as_slice(),
        None => resp.body.as_bytes(),
    };
    let mut header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nServer: DapCode-HTTP/1.0\r\n",
        resp.status, status_text, resp.content_type, body_bytes.len()
    );
    for (k, v) in &resp.headers {
        if !k.starts_with('_') {
            header.push_str(&format!("{}: {}\r\n", k, v));
        }
    }
    header.push_str("Connection: close\r\n\r\n");

    let mut out = header.into_bytes();
    out.extend_from_slice(body_bytes);
    out
}

// ==========================================
// --- WebSocket Subsystem (RFC 6455) ---
// ==========================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketFrame {
    pub fin: bool,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

impl WebSocketFrame {
    pub fn text(payload: &str) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: 0x1,
            payload: payload.as_bytes().to_vec(),
        }
    }

    pub fn binary(payload: Vec<u8>) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: 0x2,
            payload,
        }
    }

    pub fn close() -> Self {
        WebSocketFrame {
            fin: true,
            opcode: 0x8,
            payload: Vec::new(),
        }
    }

    pub fn ping() -> Self {
        WebSocketFrame {
            fin: true,
            opcode: 0x9,
            payload: Vec::new(),
        }
    }

    pub fn pong(payload: Vec<u8>) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: 0xA,
            payload,
        }
    }
}

/// Membaca satu frame WebSocket (RFC 6455) dari stream
pub fn baca_frame_websocket<R: Read>(reader: &mut R) -> Result<Option<WebSocketFrame>, String> {
    let mut header = [0u8; 2];
    match reader.read_exact(&mut header) {
        Ok(_) => {},
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof || e.kind() == std::io::ErrorKind::ConnectionReset => return Ok(None),
        Err(e) => return Err(format!("Gagal membaca header WebSocket: {}", e)),
    }

    let b0 = header[0];
    let b1 = header[1];

    let fin = (b0 & 0x80) != 0;
    let opcode = b0 & 0x0F;
    let masked = (b1 & 0x80) != 0;
    let len_byte = b1 & 0x7F;

    let payload_len: usize = if len_byte <= 125 {
        len_byte as usize
    } else if len_byte == 126 {
        let mut ext = [0u8; 2];
        reader.read_exact(&mut ext).map_err(|e| format!("Gagal membaca extended len 16: {}", e))?;
        u16::from_be_bytes(ext) as usize
    } else {
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext).map_err(|e| format!("Gagal membaca extended len 64: {}", e))?;
        u64::from_be_bytes(ext) as usize
    };

    let mask_key: Option<[u8; 4]> = if masked {
        let mut key = [0u8; 4];
        reader.read_exact(&mut key).map_err(|e| format!("Gagal membaca masking key: {}", e))?;
        Some(key)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload).map_err(|e| format!("Gagal membaca payload WebSocket: {}", e))?;
        if let Some(mask) = mask_key {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
    }

    Ok(Some(WebSocketFrame {
        fin,
        opcode,
        payload,
    }))
}

/// Menulis frame WebSocket ke writer (server-to-client tanpa masking sesuai RFC 6455)
pub fn tulis_frame_websocket<W: Write>(writer: &mut W, opcode: u8, payload: &[u8]) -> Result<(), String> {
    let mut header = Vec::with_capacity(10);
    // FIN = 1
    let b0: u8 = 0x80 | (opcode & 0x0F);
    header.push(b0);

    let len = payload.len();
    if len <= 125 {
        header.push(len as u8);
    } else if len <= 65535 {
        header.push(126);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(127);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }

    writer.write_all(&header).map_err(|e| format!("Gagal menulis header frame WS: {}", e))?;
    if !payload.is_empty() {
        writer.write_all(payload).map_err(|e| format!("Gagal menulis payload frame WS: {}", e))?;
    }
    writer.flush().map_err(|e| format!("Gagal flush frame WS: {}", e))?;
    Ok(())
}

/// Memeriksa apakah permintaan HTTP merupakan handshake WebSocket upgrade
pub fn adalah_permintaan_websocket(req: &HttpServerRequest) -> bool {
    let is_upgrade = req.headers.get("upgrade")
        .map(|v| v.to_lowercase().contains("websocket"))
        .unwrap_or(false);
    let has_key = req.headers.contains_key("sec-websocket-key");
    is_upgrade && has_key
}

/// Mengambil nilai Sec-WebSocket-Key dari request HTTP
pub fn ambil_kunci_websocket(req: &HttpServerRequest) -> Option<String> {
    req.headers.get("sec-websocket-key").cloned()
}

/// Membuat respons HTTP 101 Switching Protocols untuk upgrade ke WebSocket
pub fn buat_respons_upgrade_websocket(kunci_client: &str) -> Result<HttpServerResponse, String> {
    if kunci_client.trim().is_empty() {
        return Err("Kunci Sec-WebSocket-Key client tidak boleh kosong".to_string());
    }
    let accept_key = crate::crypto::hitung_kunci_websocket(kunci_client);

    let mut headers = std::collections::HashMap::new();
    headers.insert("Sec-WebSocket-Accept".to_string(), accept_key);

    Ok(HttpServerResponse {
        status: 101,
        content_type: "".to_string(),
        body: "".to_string(),
        raw_body: None,
        headers,
    })
}

// ==========================================
// --- Router Pattern Matching & Query Parser ---
// ==========================================

/// Memeriksa apakah jalur HTTP cocok dengan pola rute (mendukung parameter segmen :nama_param dan *)
pub fn cocok_rute(pola: &str, jalur: &str) -> bool {
    let p_segments: Vec<&str> = pola.split('/').filter(|s| !s.is_empty()).collect();
    let j_segments: Vec<&str> = jalur.split('/').filter(|s| !s.is_empty()).collect();

    if p_segments.len() != j_segments.len() {
        return false;
    }

    for (p, j) in p_segments.iter().zip(j_segments.iter()) {
        if p.starts_with(':') || *p == "*" {
            continue;
        }
        if p != j {
            return false;
        }
    }

    true
}

/// Mengambil nilai parameter rute dari pola dan jalur tertentu (misal :id -> "42")
pub fn ambil_param_rute(pola: &str, jalur: &str, nama_param: &str) -> Result<String, String> {
    let p_segments: Vec<&str> = pola.split('/').filter(|s| !s.is_empty()).collect();
    let j_segments: Vec<&str> = jalur.split('/').filter(|s| !s.is_empty()).collect();

    if p_segments.len() != j_segments.len() {
        return Err(format!("Jalur '{}' tidak cocok dengan pola rute '{}'", jalur, pola));
    }

    let clean_param = nama_param.strip_prefix(':').unwrap_or(nama_param);

    for (p, j) in p_segments.iter().zip(j_segments.iter()) {
        if let Some(param_key) = p.strip_prefix(':') {
            if param_key == clean_param {
                return Ok(j.to_string());
            }
        } else if *p != *j && *p != "*" {
            return Err(format!("Jalur '{}' tidak cocok dengan pola rute '{}'", jalur, pola));
        }
    }

    Err(format!("Parameter rute ':{}' tidak ditemukan dalam pola '{}'", clean_param, pola))
}

/// Mengambil nilai parameter dari query string HTTP (misal: "halaman=2&kategori=kopi", "kategori" -> "kopi")
pub fn ambil_kueri_param(kueri: &str, kunci: &str) -> Result<String, String> {
    let clean_query = kueri.strip_prefix('?').unwrap_or(kueri);
    for pair in clean_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((k, v)) = pair.split_once('=') {
            if k == kunci {
                return Ok(url_decode(v));
            }
        } else if pair == kunci {
            return Ok(String::new());
        }
    }
    Err(format!("Parameter kueri '{}' tidak ditemukan", kunci))
}

/// Memeriksa apakah parameter tertentu ada dalam query string
pub fn ada_kueri_param(kueri: &str, kunci: &str) -> bool {
    let clean_query = kueri.strip_prefix('?').unwrap_or(kueri);
    for pair in clean_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((k, _)) = pair.split_once('=') {
            if k == kunci {
                return true;
            }
        } else if pair == kunci {
            return true;
        }
    }
    false
}

/// URL decoder sederhana untuk menangani '+' sebagai spasi dan %XX hex encoding
pub fn url_decode(s: &str) -> String {
    let mut res = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            res.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte_val) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                res.push(byte_val);
                i += 3;
            } else {
                res.push(bytes[i]);
                i += 1;
            }
        } else {
            res.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&res).to_string()
}

/// Menentukan MIME Content-Type berdasarkan ekstensi berkas
pub fn deteksi_tipe_mime(jalur: &str) -> &'static str {
    let ext = std::path::Path::new(jalur)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "wasm" => "application/wasm",
        "xml" => "application/xml; charset=utf-8",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// Membaca berkas dari disk dan menghasilkan ResponsHttp dengan Content-Type yang sesuai
pub fn respons_berkas(jalur_berkas: &str) -> Result<HttpServerResponse, String> {
    let p = std::path::Path::new(jalur_berkas);
    if !p.exists() {
        return Err(format!("Berkas tidak ditemukan: {}", jalur_berkas));
    }
    if !p.is_file() {
        return Err(format!("Jalur bukan merupakan berkas: {}", jalur_berkas));
    }

    let bytes = std::fs::read(p)
        .map_err(|e| format!("Gagal membaca berkas '{}': {}", jalur_berkas, e))?;
    let content_type = deteksi_tipe_mime(jalur_berkas).to_string();

    let is_text = content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("javascript")
        || content_type.contains("xml")
        || content_type.contains("svg");

    let body_str = if is_text {
        String::from_utf8_lossy(&bytes).to_string()
    } else {
        String::new()
    };

    Ok(HttpServerResponse {
        status: 200,
        content_type,
        body: body_str,
        raw_body: Some(bytes),
        headers: std::collections::HashMap::new(),
    })
}

/// Menyajikan berkas statis dari folder publik dengan proteksi path traversal
pub fn sajikan_berkas_statis(folder_dasar: &str, jalur_permintaan: &str) -> Result<HttpServerResponse, String> {
    let clean_req = url_decode(jalur_permintaan.split('?').next().unwrap_or(""));
    let req_path = clean_req.trim_start_matches('/');

    let target_rel = if req_path.is_empty() || req_path == "/" {
        "index.html"
    } else {
        req_path
    };

    if target_rel.contains("..") {
        return Err("Akses jalur ditolak (upaya directory traversal terdeteksi)".to_string());
    }

    let base = std::path::Path::new(folder_dasar);
    let full_path = base.join(target_rel);

    let final_path = if full_path.is_dir() {
        full_path.join("index.html")
    } else {
        full_path
    };

    let path_str = final_path.to_string_lossy().to_string();
    respons_berkas(&path_str)
}

pub fn run_http_server<F, W>(
    port: u16,
    max_requests: u64,
    mut handler: F,
    mut ws_handler: W,
) -> Result<u64, String>
where
    F: FnMut(HttpServerRequest) -> Result<HttpServerResponse, String>,
    W: FnMut(&str, &str) -> Option<String>,
{
    use std::net::TcpListener;

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)
        .map_err(|e| format!("Gagal mengikat server HTTP ke {}: {}", addr, e))?;

    let mut served_count: u64 = 0;

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Peringatan koneksi HTTP: {}", e);
                continue;
            }
        };

        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

        let mut buf = [0u8; 8192];
        let n = match stream.read(&mut buf) {
            Ok(n) if n > 0 => n,
            _ => continue,
        };

        let req = match parse_http_request(&buf[..n]) {
            Ok(r) => r,
            Err(err) => {
                let err_resp = HttpServerResponse {
                    status: 400,
                    content_type: "text/plain; charset=utf-8".to_string(),
                    body: format!("400 Bad Request: {}", err),
                    raw_body: None,
                    headers: std::collections::HashMap::new(),
                };
                let _ = stream.write_all(&format_http_response(&err_resp));
                continue;
            }
        };

        let resp = match handler(req) {
            Ok(r) => r,
            Err(err_msg) => HttpServerResponse {
                status: 500,
                content_type: "text/plain; charset=utf-8".to_string(),
                body: format!("500 Internal Server Error: {}", err_msg),
                raw_body: None,
                headers: std::collections::HashMap::new(),
            },
        };

        let resp_bytes = format_http_response(&resp);
        let _ = stream.write_all(&resp_bytes);
        let _ = stream.flush();

        // Jika respons 101 Switching Protocols, jalankan loop frame WebSocket RFC 6455
        if resp.status == 101 {
            let _ = stream.set_read_timeout(None);
            let _ = stream.set_write_timeout(None);

            if let Some(ws_fn) = resp.headers.get("_ws_handler").cloned() {
                loop {
                    match baca_frame_websocket(&mut stream) {
                        Ok(Some(frame)) => {
                            match frame.opcode {
                                0x1 => {
                                    // Frame Teks
                                    let incoming_str = String::from_utf8_lossy(&frame.payload).to_string();
                                    if let Some(reply) = ws_handler(&ws_fn, &incoming_str) {
                                        let _ = tulis_frame_websocket(&mut stream, 0x1, reply.as_bytes());
                                    }
                                }
                                0x8 => {
                                    // Frame Tutup (Close)
                                    let _ = tulis_frame_websocket(&mut stream, 0x8, &[]);
                                    break;
                                }
                                0x9 => {
                                    // Frame Ping -> Balas Pong (0xA)
                                    let _ = tulis_frame_websocket(&mut stream, 0xA, &frame.payload);
                                }
                                _ => {}
                            }
                        }
                        _ => break,
                    }
                }
            }
        }

        served_count += 1;
        if max_requests > 0 && served_count >= max_requests {
            break;
        }
    }

    Ok(served_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url() {
        let (host, port, path) = parse_url("http://example.com/api/test").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        assert_eq!(path, "/api/test");

        let (host2, port2, path2) = parse_url("http://localhost:8080/data").unwrap();
        assert_eq!(host2, "localhost");
        assert_eq!(port2, 8080);
        assert_eq!(path2, "/data");
    }

    #[test]
    fn test_parse_and_format_http_server() {
        let raw_req = b"GET /api/pengguna?aktif=benar&limit=10 HTTP/1.1\r\nHost: localhost:8080\r\nContent-Type: application/json\r\n\r\n{\"auth\": true}";
        let req = parse_http_request(raw_req).expect("Failed to parse request");

        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/pengguna");
        assert_eq!(req.query, "aktif=benar&limit=10");
        assert_eq!(req.headers.get("content-type").unwrap(), "application/json");
        assert_eq!(req.body, "{\"auth\": true}");

        let mut headers = std::collections::HashMap::new();
        headers.insert("Access-Control-Allow-Origin".to_string(), "*".to_string());
        let resp = HttpServerResponse {
            status: 200,
            content_type: "application/json".to_string(),
            body: "{\"status\": \"sukses\"}".to_string(),
            raw_body: None,
            headers,
        };
        let formatted = format_http_response(&resp);
        let resp_str = String::from_utf8_lossy(&formatted);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp_str.contains("Content-Type: application/json\r\n"));
        assert!(resp_str.contains("Access-Control-Allow-Origin: *\r\n"));
        assert!(resp_str.ends_with("{\"status\": \"sukses\"}"));
    }

    #[test]
    fn test_route_pattern_matching_and_params() {
        assert!(cocok_rute("/api/item/:id", "/api/item/42"));
        assert!(cocok_rute("/api/users/:user_id/posts/:post_id", "/api/users/10/posts/99"));
        assert!(!cocok_rute("/api/item/:id", "/api/item/42/detail"));
        assert!(!cocok_rute("/api/item/:id", "/api/user/42"));

        let p1 = ambil_param_rute("/api/item/:id", "/api/item/42", "id").unwrap();
        assert_eq!(p1, "42");

        let p2 = ambil_param_rute("/api/users/:uid/orders/:oid", "/api/users/alice/orders/order-123", ":oid").unwrap();
        assert_eq!(p2, "order-123");

        let err = ambil_param_rute("/api/item/:id", "/api/item/42", "name");
        assert!(err.is_err());
    }

    #[test]
    fn test_query_param_parsing() {
        let q = "kategori=kopi%20tubruk&halaman=2&urut=desc&aktif";
        assert_eq!(ambil_kueri_param(q, "kategori").unwrap(), "kopi tubruk");
        assert_eq!(ambil_kueri_param(q, "halaman").unwrap(), "2");
        assert_eq!(ambil_kueri_param(q, "urut").unwrap(), "desc");
        assert_eq!(ambil_kueri_param(q, "aktif").unwrap(), "");
        assert!(ambil_kueri_param(q, "hilang").is_err());

        assert!(ada_kueri_param(q, "halaman"));
        assert!(ada_kueri_param(q, "aktif"));
        assert!(!ada_kueri_param(q, "tidak_ada"));
    }

    #[test]
    fn test_mime_detection_and_static_files() {
        assert_eq!(deteksi_tipe_mime("index.html"), "text/html; charset=utf-8");
        assert_eq!(deteksi_tipe_mime("style.css"), "text/css; charset=utf-8");
        assert_eq!(deteksi_tipe_mime("app.js"), "application/javascript; charset=utf-8");
        assert_eq!(deteksi_tipe_mime("data.json"), "application/json; charset=utf-8");
        assert_eq!(deteksi_tipe_mime("logo.png"), "image/png");
        assert_eq!(deteksi_tipe_mime("icon.svg"), "image/svg+xml");

        let temp_dir = std::env::temp_dir().join("dapcode_mime_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let html_file = temp_dir.join("index.html");
        std::fs::write(&html_file, "<h1>Halo DapCode</h1>").unwrap();

        let resp = sajikan_berkas_statis(temp_dir.to_str().unwrap(), "/").unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, "text/html; charset=utf-8");
        assert_eq!(resp.body, "<h1>Halo DapCode</h1>");

        let traversal = sajikan_berkas_statis(temp_dir.to_str().unwrap(), "/../rahasia.txt");
        assert!(traversal.is_err());
    }

    #[test]
    fn test_websocket_framing_and_handshake() {
        // 1. Uji Handshake
        let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
        let resp = buat_respons_upgrade_websocket(client_key).unwrap();
        assert_eq!(resp.status, 101);
        assert_eq!(resp.headers.get("Sec-WebSocket-Accept").unwrap(), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");

        let raw_bytes = format_http_response(&resp);
        let resp_str = String::from_utf8_lossy(&raw_bytes);
        assert!(resp_str.contains("HTTP/1.1 101 Switching Protocols"));
        assert!(resp_str.contains("Upgrade: websocket"));
        assert!(resp_str.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

        // 2. Uji Tulis & Baca Frame Teks
        let mut buffer = Vec::new();
        tulis_frame_websocket(&mut buffer, 0x1, b"Halo WebSocket DapCode!").unwrap();

        let mut cursor = std::io::Cursor::new(buffer);
        let frame = baca_frame_websocket(&mut cursor).unwrap().expect("Frame harus valid");
        assert!(frame.fin);
        assert_eq!(frame.opcode, 0x1);
        assert_eq!(String::from_utf8(frame.payload).unwrap(), "Halo WebSocket DapCode!");

        // 3. Uji Ping & Pong
        let mut ping_buf = Vec::new();
        tulis_frame_websocket(&mut ping_buf, 0x9, b"ping-data").unwrap();
        let mut ping_cur = std::io::Cursor::new(ping_buf);
        let ping_frame = baca_frame_websocket(&mut ping_cur).unwrap().unwrap();
        assert_eq!(ping_frame.opcode, 0x9);
        assert_eq!(ping_frame.payload, b"ping-data");
    }
}

