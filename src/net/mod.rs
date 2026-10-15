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
}

impl Default for HttpServerResponse {
    fn default() -> Self {
        HttpServerResponse {
            status: 200,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: String::new(),
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

    let body_bytes = resp.body.as_bytes();
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nServer: DapCode-HTTP/1.0\r\nConnection: close\r\n\r\n",
        resp.status, status_text, resp.content_type, body_bytes.len()
    );

    let mut out = header.into_bytes();
    out.extend_from_slice(body_bytes);
    out
}

pub fn run_http_server<F>(
    port: u16,
    max_requests: u64,
    mut handler: F,
) -> Result<u64, String>
where
    F: FnMut(HttpServerRequest) -> Result<HttpServerResponse, String>,
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
            },
        };

        let resp_bytes = format_http_response(&resp);
        let _ = stream.write_all(&resp_bytes);
        let _ = stream.flush();

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

        let resp = HttpServerResponse {
            status: 200,
            content_type: "application/json".to_string(),
            body: "{\"status\": \"sukses\"}".to_string(),
        };
        let formatted = format_http_response(&resp);
        let resp_str = String::from_utf8_lossy(&formatted);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp_str.contains("Content-Type: application/json\r\n"));
        assert!(resp_str.ends_with("{\"status\": \"sukses\"}"));
    }
}

