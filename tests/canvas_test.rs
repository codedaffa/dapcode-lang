use dapcodec::ast::*;
use dapcodec::codegen::CodeGenerator;
use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
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
fn test_canvas_typechecker_and_interpreter() {
    let code = r#"
fungsi uji_kanvas_lengkap() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    pastikan_sama(w, 800.0)
    pastikan_sama(h, 600.0)

    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()
    tombol := kanvas_tombol_ditekan(32)
    dt := kanvas_delta_waktu()
    t := waktu_sekarang_ms()
    pastikan_sama(mx, 400.0)
    pastikan_sama(my, 300.0)
    pastikan_sama(klik, 0)
    pastikan_sama(tombol, 0)
    pastikan_sama(dt, 0.0166)

    kanvas_bersihkan(0.1, 0.2, 0.3, 1.0)
    kanvas_warna_isi(1.0, 0.0, 0.0, 0.8)
    kanvas_warna_garis(0.0, 1.0, 0.0, 1.0)
    kanvas_tebal_garis(3.0)

    kanvas_persegi(10.0, 20.0, 100.0, 50.0)
    kanvas_bingkai_persegi(15.0, 25.0, 90.0, 40.0)
    kanvas_lingkaran(200.0, 200.0, 50.0)
    kanvas_bingkai_lingkaran(200.0, 200.0, 55.0)
    kanvas_garis(0.0, 0.0, 800.0, 600.0)
    kanvas_titik(400.0, 300.0, 5.0)
    kanvas_teks("Halo Kanvas DapCode", 50.0, 500.0, 16.0)
}

fungsi utama() -> Unit {
    uji_kanvas_lengkap()
}
"#;
    let program = parse_code(code);

    // 1. TypeChecker
    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("TypeChecker kanvas gagal");

    // 2. Evaluator
    let mut eval = Evaluator::new();
    let val = eval.run_program(&program).expect("Evaluasi interpreter gagal");
    assert!(matches!(val, Value::Unit));
}

#[test]
fn test_canvas_c_codegen() {
    let code = r#"
fungsi gambar_statis() -> Unit {
    kanvas_bersihkan(0.0, 0.0, 0.0, 1.0)
    kanvas_persegi(10.0, 10.0, 50.0, 50.0)
    kanvas_lingkaran(100.0, 100.0, 25.0)
    kanvas_teks("Test", 20.0, 20.0, 12.0)
    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()
    tombol := kanvas_tombol_ditekan(65)
    dt := kanvas_delta_waktu()
    t := waktu_sekarang_ms()
}
"#;
    let program = parse_code(code);
    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);
    let c_code = codegen.generate_c(&program);

    assert!(c_code.contains("dap_kanvas_bersihkan"));
    assert!(c_code.contains("dap_kanvas_persegi"));
    assert!(c_code.contains("dap_kanvas_lingkaran"));
    assert!(c_code.contains("dap_kanvas_teks"));
    assert!(c_code.contains("dap_kanvas_mouse_x"));
    assert!(c_code.contains("dap_kanvas_mouse_y"));
    assert!(c_code.contains("dap_kanvas_mouse_klik"));
    assert!(c_code.contains("dap_kanvas_tombol_ditekan"));
    assert!(c_code.contains("dap_kanvas_delta_waktu"));
    assert!(c_code.contains("dap_waktu_sekarang_ms"));
}

#[test]
fn test_canvas_wasm_compilation_and_node_execution() {
    let code = r#"
fungsi render_grafis() -> Unit {
    kanvas_bersihkan(0.05, 0.05, 0.1, 1.0)
    kanvas_warna_isi(0.2, 0.6, 1.0, 1.0)
    kanvas_warna_garis(1.0, 1.0, 1.0, 0.5)
    kanvas_tebal_garis(2.0)

    kanvas_persegi(50.0, 60.0, 120.0, 80.0)
    kanvas_bingkai_persegi(40.0, 50.0, 140.0, 100.0)
    kanvas_lingkaran(300.0, 200.0, 45.0)
    kanvas_bingkai_lingkaran(300.0, 200.0, 50.0)
    kanvas_garis(10.0, 10.0, 500.0, 400.0)
    kanvas_titik(250.0, 150.0, 4.0)
    kanvas_teks("WebAssembly Canvas 2D", 60.0, 100.0, 14.0)

    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()
    tombol := kanvas_tombol_ditekan(32)
    dt := kanvas_delta_waktu()
    t := waktu_sekarang_ms()
    kanvas_titik(mx, my, 8.0)
}

fungsi utama() -> u64 {
    render_grafis()
    kembali 1
}
"#;
    let program = parse_code(code);
    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM kanvas gagal");

    // Header WASM harus valid: \0asm versi 1
    assert_eq!(&wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    assert_eq!(&wasm_bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);

    // Jalankan dan verifikasi di Node.js
    let temp_wasm = std::env::temp_dir().join("test_dap_canvas_wasm.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis temp wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
const log = [];
let wasmInstance = null;
const imports = {{
    env: {{
        cetak_u64: () => {{}},
        cetak_f64: () => {{}},
        cetak_teks: () => {{}},
        kanvas_bersihkan: (r, g, b, a) => log.push(['bersihkan', r, g, b, a]),
        kanvas_warna_isi: (r, g, b, a) => log.push(['warna_isi', r, g, b, a]),
        kanvas_warna_garis: (r, g, b, a) => log.push(['warna_garis', r, g, b, a]),
        kanvas_tebal_garis: (t) => log.push(['tebal_garis', t]),
        kanvas_persegi: (x, y, w, h) => log.push(['persegi', x, y, w, h]),
        kanvas_bingkai_persegi: (x, y, w, h) => log.push(['bingkai_persegi', x, y, w, h]),
        kanvas_persegi_bulat: (x, y, w, h, r) => log.push(['persegi_bulat', x, y, w, h, r]),
        kanvas_bingkai_persegi_bulat: (x, y, w, h, r) => log.push(['bingkai_persegi_bulat', x, y, w, h, r]),
        kanvas_lingkaran: (x, y, r) => log.push(['lingkaran', x, y, r]),
        kanvas_bingkai_lingkaran: (x, y, r) => log.push(['bingkai_lingkaran', x, y, r]),
        kanvas_garis: (x1, y1, x2, y2) => log.push(['garis', x1, y1, x2, y2]),
        kanvas_titik: (x, y, u) => log.push(['titik', x, y, u]),
        kanvas_teks: (ptr, len, x, y, s) => {{
            const arr = new Uint8Array(wasmInstance.exports.memory.buffer, ptr, len);
            const str = new TextDecoder().decode(arr);
            log.push(['teks', str, x, y, s]);
        }},
        kanvas_teks_tengah: (ptr, len, cx, cy, s) => {{
            const arr = new Uint8Array(wasmInstance.exports.memory.buffer, ptr, len);
            const str = new TextDecoder().decode(arr);
            log.push(['teks_tengah', str, cx, cy, s]);
        }},
        kanvas_lebar: () => 800.0,
        kanvas_tinggi: () => 600.0,
        kanvas_mouse_x: () => 150.0,
        kanvas_mouse_y: () => 250.0,
        kanvas_mouse_klik: () => 1n,
        kanvas_tombol_ditekan: (code) => (code === 32n ? 1n : 0n),
        kanvas_delta_waktu: () => 0.0166,
        waktu_sekarang_ms: () => 1000n,
    }}
}};
WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    wasmInstance = instance;
    const res = instance.exports.utama();
    if (res !== 1n) process.exit(1);

    const hasBersihkan = log.some(c => c[0] === 'bersihkan');
    const hasPersegi = log.some(c => c[0] === 'persegi');
    const hasLingkaran = log.some(c => c[0] === 'lingkaran');
    const hasGaris = log.some(c => c[0] === 'garis');
    const hasTitik = log.some(c => c[0] === 'titik');
    const teksCall = log.find(c => c[0] === 'teks');

    if (!hasBersihkan || !hasPersegi || !hasLingkaran || !hasGaris || !hasTitik || !teksCall) {{
        console.error("Missing canvas call in log:", log);
        process.exit(2);
    }}

    if (teksCall[1] !== "WebAssembly Canvas 2D") {{
        console.error("Incorrect text interned:", teksCall);
        process.exit(3);
    }}

    // Verifikasi titik pada koordinat mouse (150.0, 250.0)
    const mousePoint = log.find(c => c[0] === 'titik' && c[1] === 150.0 && c[2] === 250.0);
    if (!mousePoint) {{
        console.error("Titik koordinat mouse tidak ditemukan:", log);
        process.exit(4);
    }}

    console.log("CANVAS_WASM_NODE_OK");
}}).catch(err => {{
    console.error(err);
    process.exit(5);
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
    assert!(output.status.success(), "Node Canvas WASM gagal: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_str.contains("CANVAS_WASM_NODE_OK"));

    let _ = fs::remove_file(temp_wasm);
}

#[test]
fn test_kompilasi_ke_wasm_builtin() {
    let code = r#"
fungsi uji_compiler() -> Teks {
    kode_sumber := "fungsi kuadrat(x: u64) -> u64 \{ kembali x * x \}\nfungsi utama() -> u64 \{ kembali kuadrat(7) \}"
    b64 := kompilasi_ke_wasm(kode_sumber) ? tangkap err {
        kembali "GALAT: " + err
    }
    kembali b64
}

fungsi utama() -> Teks {
    kembali uji_compiler()
}
"#;
    let program = parse_code(code);
    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("TypeChecker kompilasi_ke_wasm gagal");

    let mut eval = Evaluator::new();
    let val = eval.run_program(&program).expect("Evaluasi kompilasi_ke_wasm gagal");
    if let Value::String(b64) = val {
        assert!(!b64.is_empty(), "Base64 WASM hasil kompilasi tidak boleh kosong");
        assert!(!b64.starts_with("GALAT:"), "Kompilasi menghasilkan galat: {}", b64);
        assert!(b64.starts_with("AGFzbQE"), "Header WASM Base64 harus diawali 'AGFzbQE'");
    } else {
        panic!("Hasil bukan string: {:?}", val);
    }
}
