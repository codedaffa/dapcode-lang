use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn run_dapcode(src: &str) -> Result<Value, String> {
    let mut lexer = Lexer::new(src, 0);
    let tokens = lexer.tokenize().map_err(|d| d.message)?;
    let mut parser = Parser::new(tokens, 0);
    let program = parser.parse_program().map_err(|d| d.message)?;
    let mut checker = TypeChecker::new(0);
    checker.check_program(&program).map_err(|d| d.message)?;
    let mut eval = Evaluator::new();
    eval.run_program(&program).map_err(|d| d.message)
}

#[test]
fn test_http_get_mock_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 26\r\nConnection: close\r\n\r\n{\"status\":\"koneksi_sukses\"}";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    url := "http://127.0.0.1:{}/status"
    respons := ambil_http(url) ? tangkap err {{
        kembali "Gagal: " + err
    }}
    kembali ambil_kunci_json(respons, "status") ? tangkap err {{
        kembali "Error JSON"
    }}
}}
"#,
        port
    );

    let res = run_dapcode(&code).expect("Evaluasi DapCode harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "koneksi_sukses"),
        other => panic!("Diharapkan Value::String, didapat {:?}", other),
    }

    server_handle.join().unwrap();
}

#[test]
fn test_http_post_mock_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);
            assert!(req_str.starts_with("POST /data HTTP/1.1"));
            assert!(req_str.contains("Content-Type: application/json"));
            assert!(req_str.contains("DapCode-Bahasa"));

            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 21\r\nConnection: close\r\n\r\n{\"hasil\":\"diterima\"}";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    url := "http://127.0.0.1:{}/data"
    payload := "\{{\"proyek\":\"DapCode-Bahasa\"\}}"
    respons := kirim_http_post(url, payload) ? tangkap err {{
        kembali "Gagal: " + err
    }}
    kembali ambil_kunci_json(respons, "hasil") ? tangkap err {{
        kembali "Error"
    }}
}}
"#,
        port
    );

    let res = run_dapcode(&code).expect("Evaluasi DapCode harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "diterima"),
        other => panic!("Diharapkan Value::String, didapat {:?}", other),
    }

    server_handle.join().unwrap();
}

#[test]
fn test_http_error_union_catch_404() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 14\r\nConnection: close\r\n\r\nHalaman Hilang";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let code = format!(
        r#"
fungsi utama() -> Teks {{
    url := "http://127.0.0.1:{}/tidak_ada"
    respons := ambil_http(url) ? tangkap err {{
        kembali "Tertangkap: 404"
    }}
    kembali respons
}}
"#,
        port
    );

    let res = run_dapcode(&code).expect("Evaluasi DapCode harus berhasil");
    match res {
        Value::String(s) => assert_eq!(s, "Tertangkap: 404"),
        other => panic!("Diharapkan Value::String, didapat {:?}", other),
    }

    server_handle.join().unwrap();
}

#[test]
fn test_http_status_code_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let code = format!(
        r#"
fungsi utama() -> u64 {{
    url := "http://127.0.0.1:{}/cek"
    status := ambil_status_http(url) ? tangkap err {{
        kembali 500
    }}
    kembali status
}}
"#,
        port
    );

    let res = run_dapcode(&code).expect("Evaluasi DapCode harus berhasil");
    match res {
        Value::Int(code) => assert_eq!(code, 200),
        other => panic!("Diharapkan Value::Int(200), didapat {:?}", other),
    }

    server_handle.join().unwrap();
}
