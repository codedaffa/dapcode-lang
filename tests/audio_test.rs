use dapcodec::ast::*;
use dapcodec::codegen::CodeGenerator;
use dapcodec::eval::{note_to_frequency, Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::ownership::OwnershipAnalyzer;
use dapcodec::parser::Parser;
use dapcodec::types::TypeChecker;
use dapcodec::wasm::WasmEmitter;
use std::fs;

fn parse_code(code: &str) -> Program {
    let mut lexer = Lexer::new(code, 0);
    let tokens = lexer.tokenize().expect("Tokenisasi gagal");
    let mut parser = Parser::new(tokens, 0);
    parser.parse_program().expect("Parsing gagal")
}

#[test]
fn test_audio_note_to_frequency_precision() {
    // 1. Standar A4 = 440 Hz
    let a4 = note_to_frequency("A4");
    assert!((a4 - 440.0).abs() < 0.01, "A4 harus 440 Hz, dapat: {}", a4);

    // 2. Middle C (C4) ≈ 261.63 Hz
    let c4 = note_to_frequency("C4");
    assert!((c4 - 261.63).abs() < 0.05, "C4 harus ~261.63 Hz, dapat: {}", c4);

    // 3. Oktaf bawah & atas
    let a3 = note_to_frequency("A3");
    assert!((a3 - 220.0).abs() < 0.01, "A3 harus 220 Hz");

    let a5 = note_to_frequency("A5");
    assert!((a5 - 880.0).abs() < 0.01, "A5 harus 880 Hz");

    let c5 = note_to_frequency("C5");
    assert!((c5 - 523.25).abs() < 0.05, "C5 harus ~523.25 Hz");

    // 4. Aksidental kres (#) dan mol (b)
    let c_sharp = note_to_frequency("C#4");
    let d_flat = note_to_frequency("Db4");
    assert!((c_sharp - d_flat).abs() < 0.001, "C#4 dan Db4 harus bernilai sama persis (enarmonis)");
    assert!((c_sharp - 277.18).abs() < 0.05, "C#4 harus ~277.18 Hz");

    // 5. Huruf kecil dan whitespace
    let a4_lower = note_to_frequency("  a4  ");
    assert!((a4_lower - 440.0).abs() < 0.01);
}

#[test]
fn test_audio_typechecker_and_interpreter() {
    let code = r#"
fungsi uji_sintesis_audio() -> Unit {
    audio_set_volume(0.75)

    freq_c4 := audio_frekuensi_nada("C4")
    freq_a4 := audio_frekuensi_nada("A4")

    audio_mainkan_nada(freq_a4, 0.5, 0.8)
    audio_mainkan_gelombang(1, freq_c4, 0.25, 0.6)
    audio_efek_suara(2)
    audio_hentikan_semua()
}

fungsi utama() -> Unit {
    uji_sintesis_audio()
}
"#;
    let program = parse_code(code);

    // 1. TypeChecker
    let mut tc = TypeChecker::new(0);
    tc.check_program(&program).expect("Pemeriksaan tipe Web Audio gagal");

    // 2. Evaluator
    let mut eval = Evaluator::new();
    let val = eval.run_program(&program).expect("Evaluasi interpreter Web Audio gagal");
    assert!(matches!(val, Value::Unit));

    // Verifikasi output log audio di interpreter
    let output = eval.get_output();
    let joined = output.join("\n");
    assert!(joined.contains("[Audio] Set master volume=0.75"));
    assert!(joined.contains("[Audio] Mainkan nada freq=440.00Hz"));
    assert!(joined.contains("[Audio] Gelombang square freq=261.63Hz"));
    assert!(joined.contains("[Audio SFX] Efek suara: koin"));
    assert!(joined.contains("[Audio] Hentikan semua audio"));
}

#[test]
fn test_audio_c_codegen() {
    let code = r#"
fungsi sintesis_c() -> Unit {
    f := audio_frekuensi_nada("A4")
    audio_mainkan_nada(f, 0.3, 0.5)
    audio_mainkan_gelombang(2, 220.0, 0.4, 0.6)
    audio_efek_suara(1)
    audio_set_volume(0.9)
    audio_hentikan_semua()
}
"#;
    let program = parse_code(code);
    let ownership = OwnershipAnalyzer::new().analyze_program(&program);
    let mut codegen = CodeGenerator::new(&ownership);
    let c_code = codegen.generate_c(&program);

    assert!(c_code.contains("dap_audio_frekuensi_nada"));
    assert!(c_code.contains("dap_audio_mainkan_nada"));
    assert!(c_code.contains("dap_audio_mainkan_gelombang"));
    assert!(c_code.contains("dap_audio_efek_suara"));
    assert!(c_code.contains("dap_audio_set_volume"));
    assert!(c_code.contains("dap_audio_hentikan_semua"));
}

#[test]
fn test_audio_wasm_compilation_and_node_execution() {
    let code = r#"
fungsi sintesis_wasm() -> Unit {
    audio_set_volume(0.8)
    f := audio_frekuensi_nada("C4")
    audio_mainkan_nada(f, 0.4, 0.7)
    audio_mainkan_gelombang(3, 440.0, 0.3, 0.5)
    audio_efek_suara(5)
    audio_hentikan_semua()
}

fungsi utama() -> u64 {
    sintesis_wasm()
    kembali 42
}
"#;
    let program = parse_code(code);
    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM audio gagal");

    // Header WASM harus valid: \0asm versi 1
    assert_eq!(&wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    assert_eq!(&wasm_bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);

    // Jalankan dan verifikasi panggilan host di Node.js
    let temp_wasm = std::env::temp_dir().join("test_dap_audio_wasm.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis temp wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
const calls = [];
let wasmInstance = null;
const imports = {{
    env: {{
        cetak_u64: () => {{}},
        cetak_f64: () => {{}},
        cetak_teks: () => {{}},
        audio_set_volume: (vol) => calls.push(['set_volume', vol]),
        audio_mainkan_nada: (freq, dur, vol) => calls.push(['mainkan_nada', freq, dur, vol]),
        audio_mainkan_gelombang: (tipe, freq, dur, vol) => calls.push(['mainkan_gelombang', Number(tipe), freq, dur, vol]),
        audio_efek_suara: (tipe) => calls.push(['efek_suara', Number(tipe)]),
        audio_hentikan_semua: () => calls.push(['hentikan_semua']),
        audio_frekuensi_nada: (ptr, len) => {{
            const arr = new Uint8Array(wasmInstance.exports.memory.buffer, ptr, len);
            const str = new TextDecoder().decode(arr).trim().toUpperCase();
            calls.push(['frekuensi_nada', str]);
            if (str === 'C4') return 261.63;
            if (str === 'A4') return 440.0;
            return 440.0;
        }},
        waktu_sekarang_ms: () => 1000n,
    }}
}};
WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    wasmInstance = instance;
    const res = instance.exports.utama();
    if (res !== 42n) {{
        console.error("Expected 42n, got:", res);
        process.exit(1);
    }}

    const hasSetVol = calls.some(c => c[0] === 'set_volume' && c[1] === 0.8);
    const hasFreq = calls.some(c => c[0] === 'frekuensi_nada' && c[1] === 'C4');
    const hasNada = calls.some(c => c[0] === 'mainkan_nada' && Math.abs(c[1] - 261.63) < 0.1);
    const hasGelombang = calls.some(c => c[0] === 'mainkan_gelombang' && c[1] === 3 && c[2] === 440.0);
    const hasSfx = calls.some(c => c[0] === 'efek_suara' && c[1] === 5);
    const hasStop = calls.some(c => c[0] === 'hentikan_semua');

    if (!hasSetVol || !hasFreq || !hasNada || !hasGelombang || !hasSfx || !hasStop) {{
        console.error("Panggilan audio tidak lengkap di log:", calls);
        process.exit(2);
    }}

    process.exit(0);
}}).catch(err => {{
    console.error(err);
    process.exit(3);
}});
"#,
        temp_wasm.to_string_lossy().replace('\\', "/")
    );

    let temp_js = std::env::temp_dir().join("test_dap_audio_wasm.js");
    fs::write(&temp_js, &js_test).expect("Gagal menulis temp js");

    let node_res = std::process::Command::new("node").arg(&temp_js).output();
    if let Ok(output) = node_res {
        if !output.status.success() {
            panic!(
                "Node.js WebAssembly Web Audio execution failed with status: {:?}\nstderr: {}\nstdout: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }

    let _ = fs::remove_file(temp_wasm);
    let _ = fs::remove_file(temp_js);
}
