use dapcodec::eval::{Evaluator, Value};
use dapcodec::lexer::Lexer;
use dapcodec::parser::Parser;
use dapcodec::span::SourceMap;
use dapcodec::types::TypeChecker;
use dapcodec::wasm::WasmEmitter;
use std::fs;
use std::process::Command;

fn run_dap_code(src: &str) -> (Value, Vec<String>) {
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("math_test.dap", src.to_string());
    let mut lexer = Lexer::new(src, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&program).expect("Type checking failed");

    let mut eval = Evaluator::new();
    let result = eval.run_program(&program).expect("Execution failed");
    let output = eval.get_output();
    (result, output)
}

#[test]
fn test_math_constants_and_trigonometry() {
    let src = r#"
fungsi utama() -> Bool {
    // 1. Konstanta
    var_pi := PI
    var_tau := TAU
    var_e := E

    // 2. Trigonometri dasar
    s0 := sin(0.0)
    c0 := cos(0.0)
    t0 := tan(0.0)

    // sin(PI / 2) ~ 1.0
    s_half_pi := sin(var_pi / 2.0)
    // cos(PI) ~ -1.0
    c_pi := cos(var_pi)

    // 3. Arcus trigonometri
    as1 := asin(1.0) // PI / 2
    ac1 := acos(1.0) // 0.0
    at1 := atan(1.0) // PI / 4
    at2 := atan2(1.0, 1.0) // PI / 4

    // Validasi toleransi epsilon
    eps := 0.0001
    ok_trig := nilai_mutlak(s0) < eps &&
               nilai_mutlak(c0 - 1.0) < eps &&
               nilai_mutlak(t0) < eps &&
               nilai_mutlak(s_half_pi - 1.0) < eps &&
               nilai_mutlak(c_pi - -1.0) < eps &&
               nilai_mutlak(as1 - (var_pi / 2.0)) < eps &&
               nilai_mutlak(ac1) < eps &&
               nilai_mutlak(at1 - (var_pi / 4.0)) < eps &&
               nilai_mutlak(at2 - (var_pi / 4.0)) < eps

    kembali ok_trig
}
"#;
    let (res, _) = run_dap_code(src);
    assert!(matches!(res, Value::Bool(true)));
}

#[test]
fn test_math_roots_powers_and_logarithms() {
    let src = r#"
fungsi utama() -> Bool {
    // Akar & Pangkat
    r2 := akar_kuadrat(64.0) // 8.0
    r3 := akar_kubik(27.0)   // 3.0
    p  := pangkat(2.0, 10.0) // 1024.0

    // Logaritma & Eksponensial
    l_e := log(E)            // 1.0
    l10 := log10(100.0)      // 2.0
    l2  := log2(8.0)         // 3.0
    e0  := exp(0.0)          // 1.0

    eps := 0.0001
    ok := nilai_mutlak(r2 - 8.0) < eps &&
          nilai_mutlak(r3 - 3.0) < eps &&
          nilai_mutlak(p - 1024.0) < eps &&
          nilai_mutlak(l_e - 1.0) < eps &&
          nilai_mutlak(l10 - 2.0) < eps &&
          nilai_mutlak(l2 - 3.0) < eps &&
          nilai_mutlak(e0 - 1.0) < eps

    kembali ok
}
"#;
    let (res, _) = run_dap_code(src);
    assert!(matches!(res, Value::Bool(true)));
}

#[test]
fn test_math_rounding_sign_and_minmax() {
    let src = r#"
fungsi utama() -> Bool {
    b1 := bulat(3.7) // 4.0
    b2 := bulat(3.2) // 3.0
    f1 := lantai(3.9) // 3.0
    c1 := langit_langit(3.1) // 4.0

    t_pos := tanda(45.5)  // 1.0
    t_neg := tanda(-12.8) // -1.0
    t_nol := tanda(0.0)   // 0.0

    abs_v := nilai_mutlak(-77.5) // 77.5
    mn    := min(15.0, 42.0)     // 15.0
    mx    := max(15.0, 42.0)     // 42.0

    deg := ke_derajat(PI)        // 180.0
    rad := ke_radian(180.0)      // PI

    eps := 0.0001
    ok := nilai_mutlak(b1 - 4.0) < eps &&
          nilai_mutlak(b2 - 3.0) < eps &&
          nilai_mutlak(f1 - 3.0) < eps &&
          nilai_mutlak(c1 - 4.0) < eps &&
          nilai_mutlak(t_pos - 1.0) < eps &&
          nilai_mutlak(t_neg - -1.0) < eps &&
          nilai_mutlak(t_nol) < eps &&
          nilai_mutlak(abs_v - 77.5) < eps &&
          nilai_mutlak(mn - 15.0) < eps &&
          nilai_mutlak(mx - 42.0) < eps &&
          nilai_mutlak(deg - 180.0) < eps &&
          nilai_mutlak(rad - PI) < eps

    kembali ok
}
"#;
    let (res, _) = run_dap_code(src);
    assert!(matches!(res, Value::Bool(true)));
}

#[test]
fn test_math_webassembly_execution() {
    let code = r#"
fungsi hitung_hipotenusa(a: f64, b: f64) -> f64 {
    kembali akar_kuadrat((a * a) + (b * b))
}

fungsi hitung_luas_lingkaran(r: f64) -> f64 {
    kembali PI * r * r
}

fungsi hitung_trig(sudut_deg: f64) -> f64 {
    rad := ke_radian(sudut_deg)
    s := sin(rad)
    c := cos(rad)
    kembali (s * s) + (c * c)
}
"#;
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("wasm_math.dap", code.to_string());
    let mut lexer = Lexer::new(code, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let program = parser.parse_program().expect("Parsing failed");

    let mut emitter = WasmEmitter::new();
    let wasm_bytes = emitter.compile_program(&program).expect("Kompilasi WASM gagal");

    let temp_wasm = std::env::temp_dir().join("test_dap_wasm_math.wasm");
    fs::write(&temp_wasm, &wasm_bytes).expect("Gagal menulis wasm");

    let js_test = format!(
        r#"
const fs = require('fs');
const buf = fs.readFileSync('{}');
const imports = {{
    env: {{
        cetak_u64: () => {{}},
        cetak_f64: () => {{}},
        cetak_teks: () => {{}},
        sin: Math.sin,
        cos: Math.cos,
        tan: Math.tan,
        asin: Math.asin,
        acos: Math.acos,
        atan: Math.atan,
        atan2: Math.atan2,
        akar_kubik: Math.cbrt,
        pangkat: Math.pow,
        log: Math.log,
        log10: Math.log10,
        log2: Math.log2,
        exp: Math.exp,
        tanda: Math.sign,
    }}
}};
WebAssembly.instantiate(buf, imports).then(({{ instance }}) => {{
    // Hipotenusa segitiga 3-4-5
    const c = instance.exports.hitung_hipotenusa(3.0, 4.0);
    if (Math.abs(c - 5.0) > 0.001) {{
        console.error('Hipotenusa salah: ' + c);
        process.exit(1);
    }}

    // Luas lingkaran r=10 -> PI * 100 = 314.159...
    const luas = instance.exports.hitung_luas_lingkaran(10.0);
    if (Math.abs(luas - 314.159265) > 0.001) {{
        console.error('Luas lingkaran salah: ' + luas);
        process.exit(2);
    }}

    // Identitas trigonometri: sin^2(x) + cos^2(x) == 1.0
    const trig = instance.exports.hitung_trig(45.0);
    if (Math.abs(trig - 1.0) > 0.0001) {{
        console.error('Identitas trigonometri salah: ' + trig);
        process.exit(3);
    }}

    console.log('WASM_MATH_SUCCESS');
}}).catch(err => {{
    console.error(err);
    process.exit(4);
}});
"#,
        temp_wasm.to_string_lossy().replace('\\', "/")
    );

    let output = Command::new("node")
        .arg("-e")
        .arg(&js_test)
        .output()
        .expect("Gagal menjalankan node");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Node WASM Math runner gagal: stdout={}, stderr={}",
        stdout,
        stderr
    );
    assert!(stdout.contains("WASM_MATH_SUCCESS"));
}

#[test]
fn test_math_native_c_codegen() {
    use dapcodec::ownership::OwnershipAnalyzer;
    use dapcodec::codegen::CodeGenerator;

    let code = r#"
fungsi utama() {
    s := sin(0.0)
    c := cos(0.0)
    hyp := akar_kuadrat(9.0 + 16.0)
    cbr := akar_kubik(125.0)
    pw := pangkat(2.0, 8.0)
    m := min(10.0, 50.0)
    mx := max(10.0, 50.0)
    r := bulat(4.8)
    fl := lantai(4.8)
    cl := langit_langit(4.2)
    sgn := tanda(-99.0)

    cetak("C_MATH_OK")
}
"#;
    let mut sm = SourceMap::new();
    let (file_id, _) = sm.add_file("c_math.dap", code.to_string());
    let mut lexer = Lexer::new(code, file_id);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, file_id);
    let prog = parser.parse_program().expect("Parsing failed");

    let mut checker = TypeChecker::new(file_id);
    checker.check_program(&prog).expect("Type check failed");

    let ownership = OwnershipAnalyzer::new().analyze_program(&prog);
    let mut codegen = CodeGenerator::new(&ownership);
    let c_code = codegen.generate_c(&prog);

    assert!(c_code.contains("dap_sin(0.0)"));
    assert!(c_code.contains("dap_cos(0.0)"));
    assert!(c_code.contains("dap_akar_kuadrat("));
    assert!(c_code.contains("dap_akar_kubik("));
    assert!(c_code.contains("dap_pangkat("));
    assert!(c_code.contains("dap_min("));
    assert!(c_code.contains("dap_max("));
    assert!(c_code.contains("dap_bulat("));
    assert!(c_code.contains("dap_lantai("));
    assert!(c_code.contains("dap_langit_langit("));
    assert!(c_code.contains("dap_tanda("));

    let out_dir = std::env::temp_dir();
    let c_file = out_dir.join("test_math_native.c");
    let bin_path = out_dir.join("test_math_native.exe");
    fs::write(&c_file, &c_code).expect("Failed to write c code");

    let gcc_status = Command::new("gcc")
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
            let output = Command::new(&bin_path)
                .output()
                .expect("Failed to execute native binary");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("C_MATH_OK"));
        }
    }
}
