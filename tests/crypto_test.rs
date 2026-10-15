use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use std::fs;
use std::path::Path;

#[test]
fn test_evaluator_crypto_and_base64() {
    let src = r#"
fungsi utama() -> Teks {
    hash_sha := sha256("hello")
    hash_md5 := md5("hello")
    b64 := ke_base64("DapCode")
    asli := dari_base64(b64) ? tangkap err {
        kembali "Gagal decode: {err}"
    }

    kembali "{hash_sha}|{hash_md5}|{b64}|{asli}"
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("crypto_eval.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    let mut tc = TypeChecker::new(file_id);
    tc.check_program(&program).expect("Type check failed");

    let mut evaluator = Evaluator::new();
    let res = evaluator.run_program(&program).expect("Eval failed");

    match res {
        Value::String(s) => {
            let parts: Vec<&str> = s.split('|').collect();
            assert_eq!(parts.len(), 4);
            // SHA-256("hello")
            assert_eq!(parts[0], "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
            // MD5("hello")
            assert_eq!(parts[1], "5d41402abc4b2a76b9719d911017c592");
            // Base64("DapCode")
            assert_eq!(parts[2], "RGFwQ29kZQ==");
            // Original decoded
            assert_eq!(parts[3], "DapCode");
        }
        other => panic!("Expected string result, got {:?}", other),
    }
}

#[test]
fn test_test_runner_with_crypto_assertions() {
    let src = r#"
uji "validasi hash kriptografi" {
    h_sha := sha256("DapCode 2026")
    pastikan_sama(h_sha, "30e9dfca33878fe64d3c0fb603e58200e1c4890d27f452506ec989cbf5f5820b")

    h_md5 := md5("")
    pastikan_sama(h_md5, "d41d8cd98f00b204e9800998ecf8427e")
}

uji "validasi base64 encode dan decode error handling" {
    encoded := ke_base64("Pemrograman Indonesia")
    decoded := dari_base64(encoded) ? tangkap err {
        gagal err
    }
    pastikan_sama(decoded, "Pemrograman Indonesia")

    // Test error union catching on invalid base64 input
    gagal_decode := dari_base64("bukan_base64_valid!") ? tangkap err {
        "berhasil_tangkap"
    }
    pastikan_sama(gagal_decode, "berhasil_tangkap")
}
"#;

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("uji_kripto.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Tokenize failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parse failed");

    let mut tc = TypeChecker::new(file_id);
    tc.check_program(&program).expect("Type check failed");

    let mut evaluator = Evaluator::new();
    let results = evaluator.run_tests(&program);
    assert_eq!(results.len(), 2);
    assert!(results[0].passed, "Uji 0 gagal: {:?}", results[0].error);
    assert!(results[1].passed, "Uji 1 gagal: {:?}", results[1].error);
}

#[test]
fn test_native_c_build_with_crypto() {
    let out_dir = Path::new("target/uji_c_crypto");
    let _ = fs::create_dir_all(out_dir);
    let src_path = out_dir.join("main.dap");
    let bin_path = out_dir.join("app_crypto.exe");

    let src = r#"
fungsi utama() {
    h_sha := sha256("hello")
    h_md5 := md5("hello")
    b64 := ke_base64("dapcode")
    dec := dari_base64(b64) ? tangkap err {
        "gagal"
    }
    cetak("SHA:{h_sha}")
    cetak("MD5:{h_md5}")
    cetak("B64:{b64}")
    cetak("DEC:{dec}")
}
"#;
    fs::write(&src_path, src).unwrap();

    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file(src_path.to_str().unwrap(), src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, file_id);
    let prog = parser.parse_program().unwrap();

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&prog).unwrap();

    let ownership = OwnershipAnalyzer::new().analyze_program(&prog);
    let mut codegen = dapcodec::codegen::CodeGenerator::new(&ownership);
    let c_code = codegen.generate_c(&prog);

    let c_file = out_dir.join("main.c");
    fs::write(&c_file, &c_code).unwrap();

    // Compile with gcc if available
    let gcc_status = std::process::Command::new("gcc")
        .args(&[
            "-std=c11",
            c_file.to_str().unwrap(),
            "-o",
            bin_path.to_str().unwrap(),
            "-lm",
        ])
        .status();

    if let Ok(status) = gcc_status {
        if status.success() {
            let output = std::process::Command::new(&bin_path)
                .output()
                .expect("Failed to execute native binary");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("SHA:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"));
            assert!(stdout.contains("MD5:5d41402abc4b2a76b9719d911017c592"));
            assert!(stdout.contains("B64:ZGFwY29kZQ=="));
            assert!(stdout.contains("DEC:dapcode"));
        }
    }
}
