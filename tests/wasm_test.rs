use dapcodec::ast::*;
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::wasm::WasmEmitter;
use std::fs;
use std::process::Command;

fn parse_code(code: &str) -> Program {
    let mut lexer = Lexer::new(code, 0);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, 0);
    parser.parse_program().expect("Parsing gagal")
}

#[test]
fn test_wasm_arithmetic_and_function_call() {
    let code = r#"
fungsi tambah(a: u64, b: u64) -> u64 {
    kembali a + b
}

fungsi hitung_rumus(x: u64, y: u64) -> u64 {
    hasil := tambah(x, y) * 2
    kembali hasil - 5
}
"#;
    let program = parse_code(code);
    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM gagal");

    // Header WASM harus valid: \0asm versi 1
    assert_eq!(&wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    assert_eq!(&wasm_bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);

    // Jalankan dan verifikasi di Node.js
    let temp_wasm = std::env::temp_dir().join("test_dap_wasm_calc.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis temp wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
const imports = {{
    env: {{
        cetak_u64: () => {{}},
        cetak_f64: () => {{}},
        cetak_teks: () => {{}}
    }}
}};
WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    const tambahRes = instance.exports.tambah(15n, 25n);
    if (tambahRes !== 40n) process.exit(1);

    const rumusRes = instance.exports.hitung_rumus(10n, 20n);
    // (10 + 20) * 2 - 5 = 60 - 5 = 55
    if (rumusRes !== 55n) process.exit(2);

    console.log("WASM_CALC_OK");
}}).catch(err => {{
    console.error(err);
    process.exit(3);
}});
"#,
        temp_wasm.to_string_lossy().replace('\\', "/")
    );

    let output = Command::new("node")
        .arg("-e")
        .arg(&js_test)
        .output()
        .expect("Gagal mengeksekusi node");

    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Node WASM gagal: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_str.contains("WASM_CALC_OK"));

    let _ = fs::remove_file(temp_wasm);
}

#[test]
fn test_wasm_recursion_and_branching() {
    let code = r#"
fungsi faktorial(n: u64) -> u64 {
    jika n <= 1 {
        kembali 1
    }
    kembali n * faktorial(n - 1)
}

fungsi fibonacci(n: u64) -> u64 {
    jika n <= 0 {
        kembali 0
    }
    jika n == 1 {
        kembali 1
    }
    kembali fibonacci(n - 1) + fibonacci(n - 2)
}
"#;
    let program = parse_code(code);
    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM gagal");

    let temp_wasm = std::env::temp_dir().join("test_dap_wasm_rec.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis temp wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
const imports = {{
    env: {{
        cetak_u64: () => {{}},
        cetak_f64: () => {{}},
        cetak_teks: () => {{}}
    }}
}};
WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    const f5 = instance.exports.faktorial(5n);
    if (f5 !== 120n) process.exit(1);

    const fib7 = instance.exports.fibonacci(7n); // 0, 1, 1, 2, 3, 5, 8, 13
    if (fib7 !== 13n) process.exit(2);

    console.log("WASM_REC_OK");
}}).catch(err => {{
    console.error(err);
    process.exit(3);
}});
"#,
        temp_wasm.to_string_lossy().replace('\\', "/")
    );

    let output = Command::new("node")
        .arg("-e")
        .arg(&js_test)
        .output()
        .expect("Gagal mengeksekusi node");

    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Node WASM gagal: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_str.contains("WASM_REC_OK"));

    let _ = fs::remove_file(temp_wasm);
}

#[test]
fn test_wasm_loops_and_floating_point() {
    let code = r#"
fungsi hitung_total_loop(batas: u64) -> u64 {
    total := 0
    i := 1
    selama i <= batas {
        total = total + i
        i = i + 1
    }
    kembali total
}

fungsi luas_lingkaran(r: f64) -> f64 {
    pi := 3.141592653589793
    kembali pi * (r * r)
}
"#;
    let program = parse_code(code);
    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM gagal");

    let temp_wasm = std::env::temp_dir().join("test_dap_wasm_loop.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis temp wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
const imports = {{
    env: {{
        cetak_u64: () => {{}},
        cetak_f64: () => {{}},
        cetak_teks: () => {{}}
    }}
}};
WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    const total = instance.exports.hitung_total_loop(10n); // 1 + 2 + ... + 10 = 55
    if (total !== 55n) process.exit(1);

    const luas = instance.exports.luas_lingkaran(10.0);
    if (Math.abs(luas - 314.1592653589793) > 0.001) process.exit(2);

    console.log("WASM_LOOP_OK");
}}).catch(err => {{
    console.error(err);
    process.exit(3);
}});
"#,
        temp_wasm.to_string_lossy().replace('\\', "/")
    );

    let output = Command::new("node")
        .arg("-e")
        .arg(&js_test)
        .output()
        .expect("Gagal mengeksekusi node");

    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Node WASM gagal: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_str.contains("WASM_LOOP_OK"));

    let _ = fs::remove_file(temp_wasm);
}

#[test]
fn test_wasm_strings_and_host_printing() {
    let code = r#"
fungsi utama() {
    cetak("Halo dari DapCode WebAssembly!")
    cetak(42)
}
"#;
    let program = parse_code(code);
    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM gagal");

    let temp_wasm = std::env::temp_dir().join("test_dap_wasm_str.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis temp wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
let memory;
let logs = [];

const imports = {{
    env: {{
        cetak_u64: (val) => logs.push("ANGKA:" + val.toString()),
        cetak_f64: (val) => logs.push("FLOAT:" + val.toString()),
        cetak_teks: (ptr, len) => {{
            const arr = new Uint8Array(memory.buffer, ptr, len);
            logs.push("TEKS:" + new TextDecoder().decode(arr));
        }}
    }}
}};

WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    memory = instance.exports.memory;
    instance.exports.utama();

    if (!logs.includes("TEKS:Halo dari DapCode WebAssembly!")) process.exit(1);
    if (!logs.includes("ANGKA:42")) process.exit(2);

    console.log("WASM_STR_OK");
}}).catch(err => {{
    console.error(err);
    process.exit(3);
}});
"#,
        temp_wasm.to_string_lossy().replace('\\', "/")
    );

    let output = Command::new("node")
        .arg("-e")
        .arg(&js_test)
        .output()
        .expect("Gagal mengeksekusi node");

    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Node WASM gagal: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_str.contains("WASM_STR_OK"));

    let _ = fs::remove_file(temp_wasm);
}
