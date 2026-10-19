use crate::ast::*;
use crate::diag::{Diagnostic, Result};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Representasi tipe data WebAssembly MVP 1.0
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmType {
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
}

/// Tanda tangan fungsi (parameters -> results)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncSignature {
    pub params: Vec<WasmType>,
    pub results: Vec<WasmType>,
}

/// Pembangkit Biner WebAssembly (WASM 1.0 MVP) Murni Rust
pub struct WasmEmitter {
    pub signatures: Vec<FuncSignature>,
    pub func_table: HashMap<String, (u32, u32, FuncSignature)>, // nama -> (func_index, type_idx, signature)
    pub imports: Vec<(String, String, u32)>,                    // (mod_name, func_name, type_idx)
    pub num_imports: u32,
    pub string_data: Vec<u8>,
    pub string_offsets: HashMap<String, (u32, u32)>, // teks -> (offset, len)
}

impl WasmEmitter {
    pub fn new() -> Self {
        let mut emitter = WasmEmitter {
            signatures: Vec::new(),
            func_table: HashMap::new(),
            imports: Vec::new(),
            num_imports: 0,
            string_data: Vec::new(),
            string_offsets: HashMap::new(),
        };

        // 1. Daftarkan tanda tangan import bawaan:
        // Sig 0: env.cetak_u64 (i64) -> ()
        let sig_u64 = emitter.get_or_add_sig(vec![WasmType::I64], vec![]);
        emitter.imports.push(("env".to_string(), "cetak_u64".to_string(), sig_u64));

        // Sig 1: env.cetak_f64 (f64) -> ()
        let sig_f64 = emitter.get_or_add_sig(vec![WasmType::F64], vec![]);
        emitter.imports.push(("env".to_string(), "cetak_f64".to_string(), sig_f64));

        // Sig 2: env.cetak_teks (i32, i32) -> () (ptr, len)
        let sig_teks = emitter.get_or_add_sig(vec![WasmType::I32, WasmType::I32], vec![]);
        emitter.imports.push(("env".to_string(), "cetak_teks".to_string(), sig_teks));

        emitter.num_imports = 3;
        emitter
    }

    pub fn get_or_add_sig(&mut self, params: Vec<WasmType>, results: Vec<WasmType>) -> u32 {
        let sig = FuncSignature { params, results };
        if let Some(pos) = self.signatures.iter().position(|s| s == &sig) {
            pos as u32
        } else {
            let idx = self.signatures.len() as u32;
            self.signatures.push(sig);
            idx
        }
    }

    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        if let Some(&off) = self.string_offsets.get(s) {
            return off;
        }
        let base_offset = 1024 + self.string_data.len() as u32;
        let bytes = s.as_bytes();
        let len = bytes.len() as u32;
        self.string_data.extend_from_slice(bytes);
        self.string_data.push(0); // Null terminator untuk string WebAssembly
        self.string_offsets.insert(s.to_string(), (base_offset, len));
        (base_offset, len)
    }

    fn scan_called_functions(block: &Block, called: &mut std::collections::HashSet<String>) {
        for stmt in &block.stmts {
            match stmt {
                Stmt::VarDecl { init, .. } => Self::scan_expr_calls(init, called),
                Stmt::Assign { value, .. } => Self::scan_expr_calls(value, called),
                Stmt::If { cond, then_branch, else_branch, .. } => {
                    Self::scan_expr_calls(cond, called);
                    Self::scan_called_functions(then_branch, called);
                    if let Some(eb) = else_branch {
                        Self::scan_called_functions(eb, called);
                    }
                }
                Stmt::While { cond, body, .. } => {
                    Self::scan_expr_calls(cond, called);
                    Self::scan_called_functions(body, called);
                }
                Stmt::ForIn { iterable, body, .. } => {
                    Self::scan_expr_calls(iterable, called);
                    Self::scan_called_functions(body, called);
                }
                Stmt::Return { value: Some(v), .. } => Self::scan_expr_calls(v, called),
                Stmt::Expr(e) => Self::scan_expr_calls(e, called),
                _ => {}
            }
        }
        if let Some(ref e) = block.expr {
            Self::scan_expr_calls(e, called);
        }
    }

    fn scan_expr_calls(expr: &Expr, called: &mut std::collections::HashSet<String>) {
        match expr {
            Expr::Call { callee, args, .. } => {
                if let Expr::Ident(name, _) = &**callee {
                    called.insert(name.clone());
                }
                Self::scan_expr_calls(callee, called);
                for a in args {
                    Self::scan_expr_calls(&a.value, called);
                }
            }
            Expr::Binary(lhs, _, rhs, _) => {
                Self::scan_expr_calls(lhs, called);
                Self::scan_expr_calls(rhs, called);
            }
            Expr::Unary(_, inner, _) => Self::scan_expr_calls(inner, called),
            Expr::InterpolatedString(parts, _) => {
                for p in parts {
                    if let InterpolatedExprPart::Expr(e) = p {
                        Self::scan_expr_calls(e, called);
                    }
                }
            }
            _ => {}
        }
    }

    fn has_dynamic_interpolation(block: &Block) -> bool {
        for stmt in &block.stmts {
            match stmt {
                Stmt::VarDecl { init, .. } => if Self::expr_has_dynamic_interpolation(init) { return true; },
                Stmt::Assign { value, .. } => if Self::expr_has_dynamic_interpolation(value) { return true; },
                Stmt::If { cond, then_branch, else_branch, .. } => {
                    if Self::expr_has_dynamic_interpolation(cond) { return true; }
                    if Self::has_dynamic_interpolation(then_branch) { return true; }
                    if let Some(eb) = else_branch {
                        if Self::has_dynamic_interpolation(eb) { return true; }
                    }
                }
                Stmt::While { cond, body, .. } => {
                    if Self::expr_has_dynamic_interpolation(cond) { return true; }
                    if Self::has_dynamic_interpolation(body) { return true; }
                }
                Stmt::Return { value: Some(v), .. } => if Self::expr_has_dynamic_interpolation(v) { return true; },
                Stmt::Expr(e) => if Self::expr_has_dynamic_interpolation(e) { return true; },
                _ => {}
            }
        }
        if let Some(ref e) = block.expr {
            if Self::expr_has_dynamic_interpolation(e) { return true; }
        }
        false
    }

    fn expr_has_dynamic_interpolation(expr: &Expr) -> bool {
        match expr {
            Expr::InterpolatedString(parts, _) => {
                parts.iter().any(|p| matches!(p, InterpolatedExprPart::Expr(_)))
            }
            Expr::Call { callee, args, .. } => {
                if Self::expr_has_dynamic_interpolation(callee) { return true; }
                args.iter().any(|a| Self::expr_has_dynamic_interpolation(&a.value))
            }
            Expr::Binary(lhs, _, rhs, _) => {
                Self::expr_has_dynamic_interpolation(lhs) || Self::expr_has_dynamic_interpolation(rhs)
            }
            Expr::Unary(_, inner, _) => Self::expr_has_dynamic_interpolation(inner),
            _ => false,
        }
    }

    /// Kompilasi Program AST DapCode menjadi biner WebAssembly (.wasm)
    pub fn compile_program(&mut self, program: &Program) -> Result<Vec<u8>> {
        // Kumpulkan semua fungsi
        let mut user_fns: Vec<&FnDecl> = Vec::new();
        for decl in &program.decls {
            if let Decl::Fn(f) = decl {
                user_fns.push(f);
            }
        }

        // 0. Deteksi fungsi bawaan matematika yang benar-benar dipanggil di program
        let mut called = std::collections::HashSet::new();
        for f in &user_fns {
            Self::scan_called_functions(&f.body, &mut called);
        }

        let math_1_fns = [
            "sin", "cos", "tan", "asin", "acos", "atan", "akar_kubik", "log", "log10", "log2", "exp", "tanda",
        ];
        let sig_math_1 = self.get_or_add_sig(vec![WasmType::F64], vec![WasmType::F64]);
        for name in &math_1_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_math_1));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_math_1,
                        FuncSignature {
                            params: vec![WasmType::F64],
                            results: vec![WasmType::F64],
                        },
                    ),
                );
            }
        }

        let math_2_fns = ["atan2", "pangkat"];
        let sig_math_2 = self.get_or_add_sig(vec![WasmType::F64, WasmType::F64], vec![WasmType::F64]);
        for name in &math_2_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_math_2));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_math_2,
                        FuncSignature {
                            params: vec![WasmType::F64, WasmType::F64],
                            results: vec![WasmType::F64],
                        },
                    ),
                );
            }
        }

        let canvas_0_fns = [
            "kanvas_lebar",
            "kanvas_tinggi",
            "kanvas_mouse_x",
            "kanvas_mouse_y",
            "kanvas_delta_waktu",
        ];
        let sig_canvas_0 = self.get_or_add_sig(vec![], vec![WasmType::F64]);
        for name in &canvas_0_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_canvas_0));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_canvas_0,
                        FuncSignature {
                            params: vec![],
                            results: vec![WasmType::F64],
                        },
                    ),
                );
            }
        }

        if called.contains("waktu_sekarang_ms") {
            let sig_waktu = self.get_or_add_sig(vec![], vec![WasmType::I64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "waktu_sekarang_ms".to_string(), sig_waktu));
            self.func_table.insert(
                "waktu_sekarang_ms".to_string(),
                (
                    func_idx,
                    sig_waktu,
                    FuncSignature {
                        params: vec![],
                        results: vec![WasmType::I64],
                    },
                ),
            );
        }

        if called.contains("kanvas_mouse_klik") {
            let sig_mouse_klik = self.get_or_add_sig(vec![], vec![WasmType::I64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_mouse_klik".to_string(), sig_mouse_klik));
            self.func_table.insert(
                "kanvas_mouse_klik".to_string(),
                (
                    func_idx,
                    sig_mouse_klik,
                    FuncSignature {
                        params: vec![],
                        results: vec![WasmType::I64],
                    },
                ),
            );
        }

        if called.contains("kanvas_tombol_ditekan") {
            let sig_key_pressed = self.get_or_add_sig(vec![WasmType::I64], vec![WasmType::I64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_tombol_ditekan".to_string(), sig_key_pressed));
            self.func_table.insert(
                "kanvas_tombol_ditekan".to_string(),
                (
                    func_idx,
                    sig_key_pressed,
                    FuncSignature {
                        params: vec![WasmType::I64],
                        results: vec![WasmType::I64],
                    },
                ),
            );
        }

        let canvas_1_fns = ["kanvas_tebal_garis"];
        let sig_canvas_1 = self.get_or_add_sig(vec![WasmType::F64], vec![]);
        for name in &canvas_1_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_canvas_1));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_canvas_1,
                        FuncSignature {
                            params: vec![WasmType::F64],
                            results: vec![],
                        },
                    ),
                );
            }
        }

        let canvas_3_fns = ["kanvas_lingkaran", "kanvas_bingkai_lingkaran", "kanvas_titik"];
        let sig_canvas_3 = self.get_or_add_sig(vec![WasmType::F64, WasmType::F64, WasmType::F64], vec![]);
        for name in &canvas_3_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_canvas_3));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_canvas_3,
                        FuncSignature {
                            params: vec![WasmType::F64, WasmType::F64, WasmType::F64],
                            results: vec![],
                        },
                    ),
                );
            }
        }

        let canvas_4_fns = [
            "kanvas_bersihkan",
            "kanvas_warna_isi",
            "kanvas_warna_garis",
            "kanvas_persegi",
            "kanvas_bingkai_persegi",
            "kanvas_garis",
        ];
        let sig_canvas_4 = self.get_or_add_sig(vec![WasmType::F64, WasmType::F64, WasmType::F64, WasmType::F64], vec![]);
        for name in &canvas_4_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_canvas_4));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_canvas_4,
                        FuncSignature {
                            params: vec![WasmType::F64, WasmType::F64, WasmType::F64, WasmType::F64],
                            results: vec![],
                        },
                    ),
                );
            }
        }

        let canvas_5_fns = [
            "kanvas_persegi_bulat",
            "kanvas_bingkai_persegi_bulat",
        ];
        let sig_canvas_5 = self.get_or_add_sig(vec![WasmType::F64, WasmType::F64, WasmType::F64, WasmType::F64, WasmType::F64], vec![]);
        for name in &canvas_5_fns {
            if called.contains(*name) {
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_canvas_5));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_canvas_5,
                        FuncSignature {
                            params: vec![WasmType::F64, WasmType::F64, WasmType::F64, WasmType::F64, WasmType::F64],
                            results: vec![],
                        },
                    ),
                );
            }
        }

        for name in &["kanvas_teks", "kanvas_teks_tengah"] {
            if called.contains(*name) {
                let sig_canvas_teks = self.get_or_add_sig(
                    vec![WasmType::I32, WasmType::I32, WasmType::F64, WasmType::F64, WasmType::F64],
                    vec![],
                );
                let func_idx = self.imports.len() as u32;
                self.imports.push(("env".to_string(), name.to_string(), sig_canvas_teks));
                self.func_table.insert(
                    name.to_string(),
                    (
                        func_idx,
                        sig_canvas_teks,
                        FuncSignature {
                            params: vec![WasmType::I32, WasmType::I32, WasmType::F64, WasmType::F64, WasmType::F64],
                            results: vec![],
                        },
                    ),
                );
            }
        }

        // Web Audio Synthesizer imports
        if called.contains("audio_mainkan_nada") {
            let sig = self.get_or_add_sig(vec![WasmType::F64, WasmType::F64, WasmType::F64], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "audio_mainkan_nada".to_string(), sig));
            self.func_table.insert(
                "audio_mainkan_nada".to_string(),
                (
                    func_idx,
                    sig,
                    FuncSignature {
                        params: vec![WasmType::F64, WasmType::F64, WasmType::F64],
                        results: vec![],
                    },
                ),
            );
        }

        if called.contains("audio_mainkan_gelombang") {
            let sig = self.get_or_add_sig(vec![WasmType::I64, WasmType::F64, WasmType::F64, WasmType::F64], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "audio_mainkan_gelombang".to_string(), sig));
            self.func_table.insert(
                "audio_mainkan_gelombang".to_string(),
                (
                    func_idx,
                    sig,
                    FuncSignature {
                        params: vec![WasmType::I64, WasmType::F64, WasmType::F64, WasmType::F64],
                        results: vec![],
                    },
                ),
            );
        }

        if called.contains("audio_efek_suara") {
            let sig = self.get_or_add_sig(vec![WasmType::I64], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "audio_efek_suara".to_string(), sig));
            self.func_table.insert(
                "audio_efek_suara".to_string(),
                (
                    func_idx,
                    sig,
                    FuncSignature {
                        params: vec![WasmType::I64],
                        results: vec![],
                    },
                ),
            );
        }

        if called.contains("audio_set_volume") {
            let sig = self.get_or_add_sig(vec![WasmType::F64], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "audio_set_volume".to_string(), sig));
            self.func_table.insert(
                "audio_set_volume".to_string(),
                (
                    func_idx,
                    sig,
                    FuncSignature {
                        params: vec![WasmType::F64],
                        results: vec![],
                    },
                ),
            );
        }

        if called.contains("audio_hentikan_semua") {
            let sig = self.get_or_add_sig(vec![], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "audio_hentikan_semua".to_string(), sig));
            self.func_table.insert(
                "audio_hentikan_semua".to_string(),
                (
                    func_idx,
                    sig,
                    FuncSignature {
                        params: vec![],
                        results: vec![],
                    },
                ),
            );
        }

        if called.contains("audio_frekuensi_nada") {
            let sig = self.get_or_add_sig(vec![WasmType::I32, WasmType::I32], vec![WasmType::F64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "audio_frekuensi_nada".to_string(), sig));
            self.func_table.insert(
                "audio_frekuensi_nada".to_string(),
                (
                    func_idx,
                    sig,
                    FuncSignature {
                        params: vec![WasmType::I32, WasmType::I32],
                        results: vec![WasmType::F64],
                    },
                ),
            );
        }

        // Kanvas KV & State Persistence Imports
        if called.contains("kanvas_simpan_teks") {
            let sig = self.get_or_add_sig(vec![WasmType::I32, WasmType::I32], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_simpan_teks".to_string(), sig));
            self.func_table.insert("kanvas_simpan_teks".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32, WasmType::I32], results: vec![] }));
        }

        if called.contains("kanvas_ambil_teks") {
            let sig = self.get_or_add_sig(vec![WasmType::I32], vec![WasmType::I32]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_ambil_teks".to_string(), sig));
            self.func_table.insert("kanvas_ambil_teks".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32], results: vec![WasmType::I32] }));
        }

        if called.contains("kanvas_simpan_u64") {
            let sig = self.get_or_add_sig(vec![WasmType::I32, WasmType::I64], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_simpan_u64".to_string(), sig));
            self.func_table.insert("kanvas_simpan_u64".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32, WasmType::I64], results: vec![] }));
        }

        if called.contains("kanvas_ambil_u64") {
            let sig = self.get_or_add_sig(vec![WasmType::I32], vec![WasmType::I64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_ambil_u64".to_string(), sig));
            self.func_table.insert("kanvas_ambil_u64".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32], results: vec![WasmType::I64] }));
        }

        if called.contains("kanvas_simpan_angka") {
            let sig = self.get_or_add_sig(vec![WasmType::I32, WasmType::F64], vec![]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_simpan_angka".to_string(), sig));
            self.func_table.insert("kanvas_simpan_angka".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32, WasmType::F64], results: vec![] }));
        }

        if called.contains("kanvas_ambil_angka") {
            let sig = self.get_or_add_sig(vec![WasmType::I32], vec![WasmType::F64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "kanvas_ambil_angka".to_string(), sig));
            self.func_table.insert("kanvas_ambil_angka".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32], results: vec![WasmType::F64] }));
        }

        if called.contains("ke_desimal") {
            let sig = self.get_or_add_sig(vec![WasmType::I32], vec![WasmType::F64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "ke_desimal".to_string(), sig));
            self.func_table.insert("ke_desimal".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32], results: vec![WasmType::F64] }));
        }

        if called.contains("format_desimal") {
            let sig = self.get_or_add_sig(vec![WasmType::F64], vec![WasmType::I32]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "format_desimal".to_string(), sig));
            self.func_table.insert("format_desimal".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::F64], results: vec![WasmType::I32] }));
        }

        if called.contains("panjang_teks") {
            let sig = self.get_or_add_sig(vec![WasmType::I32], vec![WasmType::I64]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "panjang_teks".to_string(), sig));
            self.func_table.insert("panjang_teks".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32], results: vec![WasmType::I64] }));
        }

        // Teks Concat & Formatting Helpers (untuk dynamic string interpolation)
        let has_interpolated_expr = user_fns.iter().any(|f| Self::has_dynamic_interpolation(&f.body));
        if has_interpolated_expr || called.contains("teks_gabung") {
            let sig = self.get_or_add_sig(vec![WasmType::I32, WasmType::I32], vec![WasmType::I32]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "teks_gabung".to_string(), sig));
            self.func_table.insert("teks_gabung".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I32, WasmType::I32], results: vec![WasmType::I32] }));
        }

        if has_interpolated_expr || called.contains("teks_format_angka") {
            let sig = self.get_or_add_sig(vec![WasmType::F64], vec![WasmType::I32]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "teks_format_angka".to_string(), sig));
            self.func_table.insert("teks_format_angka".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::F64], results: vec![WasmType::I32] }));
        }

        if has_interpolated_expr || called.contains("teks_format_u64") {
            let sig = self.get_or_add_sig(vec![WasmType::I64], vec![WasmType::I32]);
            let func_idx = self.imports.len() as u32;
            self.imports.push(("env".to_string(), "teks_format_u64".to_string(), sig));
            self.func_table.insert("teks_format_u64".to_string(), (func_idx, sig, FuncSignature { params: vec![WasmType::I64], results: vec![WasmType::I32] }));
        }

        self.num_imports = self.imports.len() as u32;

        // 1. Daftarkan tanda tangan setiap fungsi user
        let mut user_func_type_indices = Vec::new();
        for f in &user_fns {
            let mut params = Vec::new();
            for p in &f.params {
                let ty = self.ast_type_to_wasm(p.ty.as_ref());
                params.push(ty);
            }

            let mut results = Vec::new();
            if let Some(ref ret_ty) = f.ret_ty {
                if let TypeExpr::Named(name, _) = ret_ty {
                    if name != "Unit" && name != "unit" {
                        results.push(self.ast_type_to_wasm(Some(ret_ty)));
                    }
                } else if !matches!(ret_ty, TypeExpr::Unit(_)) {
                    results.push(self.ast_type_to_wasm(Some(ret_ty)));
                }
            }

            let type_idx = self.get_or_add_sig(params.clone(), results.clone());
            user_func_type_indices.push(type_idx);

            let func_idx = self.num_imports + user_func_type_indices.len() as u32 - 1;
            self.func_table.insert(
                f.name.clone(),
                (func_idx, type_idx, FuncSignature { params, results }),
            );
        }

        // 2. Hasilkan byte code untuk setiap fungsi
        let mut function_bodies: Vec<Vec<u8>> = Vec::new();
        for f in &user_fns {
            let body_bytes = self.compile_fn(f)?;
            function_bodies.push(body_bytes);
        }

        // 3. Rakit modul WASM biner lengkap
        let mut wasm = Vec::new();
        // Magic header: \0asm
        wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]);
        // Versi: 1
        wasm.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

        // --- Section 1: Type Section ---
        {
            let mut sec = Vec::new();
            encode_u32_leb128(self.signatures.len() as u32, &mut sec);
            for sig in &self.signatures {
                sec.push(0x60); // form: func
                encode_u32_leb128(sig.params.len() as u32, &mut sec);
                for p in &sig.params {
                    sec.push(*p as u8);
                }
                encode_u32_leb128(sig.results.len() as u32, &mut sec);
                for r in &sig.results {
                    sec.push(*r as u8);
                }
            }
            emit_section(1, &sec, &mut wasm);
        }

        // --- Section 2: Import Section ---
        {
            let mut sec = Vec::new();
            encode_u32_leb128(self.imports.len() as u32, &mut sec);

            for (mod_name, func_name, sig_idx) in &self.imports {
                emit_name(mod_name, &mut sec);
                emit_name(func_name, &mut sec);
                sec.push(0x00); // import func
                encode_u32_leb128(*sig_idx, &mut sec);
            }

            emit_section(2, &sec, &mut wasm);
        }

        // --- Section 3: Function Section ---
        {
            let mut sec = Vec::new();
            encode_u32_leb128(user_func_type_indices.len() as u32, &mut sec);
            for type_idx in &user_func_type_indices {
                encode_u32_leb128(*type_idx, &mut sec);
            }
            emit_section(3, &sec, &mut wasm);
        }

        // --- Section 5: Memory Section ---
        {
            let mut sec = Vec::new();
            encode_u32_leb128(1, &mut sec); // 1 memory instance
            sec.push(0x00); // limits: flags (min only)
            encode_u32_leb128(1, &mut sec); // min 1 page (64KB)
            emit_section(5, &sec, &mut wasm);
        }

        // --- Section 7: Export Section ---
        {
            let mut sec = Vec::new();
            let mut exports: Vec<(String, u8, u32)> = Vec::new();

            // Export linear memory
            exports.push(("memory".to_string(), 0x02, 0));

            // Export semua fungsi pengguna
            for f in &user_fns {
                if let Some(&(func_idx, _, _)) = self.func_table.get(&f.name) {
                    exports.push((f.name.clone(), 0x00, func_idx));
                }
            }

            encode_u32_leb128(exports.len() as u32, &mut sec);
            for (name, kind, idx) in exports {
                emit_name(&name, &mut sec);
                sec.push(kind);
                encode_u32_leb128(idx, &mut sec);
            }
            emit_section(7, &sec, &mut wasm);
        }

        // --- Section 10: Code Section ---
        {
            let mut sec = Vec::new();
            encode_u32_leb128(function_bodies.len() as u32, &mut sec);
            for body in &function_bodies {
                sec.extend_from_slice(body);
            }
            emit_section(10, &sec, &mut wasm);
        }

        // --- Section 11: Data Section (jika ada string literal) ---
        if !self.string_data.is_empty() {
            let mut sec = Vec::new();
            encode_u32_leb128(1, &mut sec); // 1 segment
            sec.push(0x00); // active data segment on memory 0
            // Offset expr: i32.const 1024, end
            sec.push(0x41);
            encode_i64_leb128(1024, &mut sec);
            sec.push(0x0B);

            // Payload bytes
            encode_u32_leb128(self.string_data.len() as u32, &mut sec);
            sec.extend_from_slice(&self.string_data);

            emit_section(11, &sec, &mut wasm);
        }

        Ok(wasm)
    }

    fn ast_type_to_wasm(&self, ty: Option<&TypeExpr>) -> WasmType {
        match ty {
            Some(TypeExpr::Named(name, _)) => {
                if name == "f64" || name == "desimal" {
                    WasmType::F64
                } else if name == "Teks" || name == "teks" || name == "String" || name == "str" {
                    WasmType::I32
                } else {
                    WasmType::I64
                }
            }
            _ => WasmType::I64,
        }
    }

    fn compile_fn(&mut self, f: &FnDecl) -> Result<Vec<u8>> {
        let mut locals_map: HashMap<String, (u32, WasmType)> = HashMap::new();
        let mut param_types: Vec<WasmType> = Vec::new();

        // 1. Petakan parameter fungsi ke local index 0..n
        let mut local_idx = 0u32;
        let mut known_locals: HashMap<String, WasmType> = HashMap::new();
        for p in &f.params {
            let ty = self.ast_type_to_wasm(p.ty.as_ref());
            locals_map.insert(p.name.clone(), (local_idx, ty));
            known_locals.insert(p.name.clone(), ty);
            param_types.push(ty);
            local_idx += 1;
        }

        // 2. Kumpulkan variabel lokal di dalam badan fungsi
        let mut extra_locals: Vec<(String, WasmType)> = Vec::new();
        Self::collect_local_vars(&f.body, &mut known_locals, &self.func_table, &mut extra_locals);

        let mut local_groups: Vec<(u32, WasmType)> = Vec::new();
        for (name, ty) in extra_locals {
            if !locals_map.contains_key(&name) {
                locals_map.insert(name, (local_idx, ty));
                local_idx += 1;
                local_groups.push((1, ty));
            }
        }

        // 3. Emit bytecode instruksi fungsi
        let mut code_buf = Vec::new();
        let mut has_ret = false;
        let expected_ret_ty = self
            .func_table
            .get(&f.name)
            .and_then(|(_, _, sig)| sig.results.first().copied());

        for stmt in &f.body.stmts {
            self.compile_stmt(stmt, &locals_map, &mut code_buf, &mut has_ret, expected_ret_ty)?;
        }

        // Evaluasi return expression jika ada
        if let Some(ref expr) = f.body.expr {
            let expr_ty = self.compile_expr(expr, &locals_map, &mut code_buf)?;
            if expected_ret_ty == Some(WasmType::F64) && expr_ty == WasmType::I64 {
                code_buf.push(0xB9); // f64.convert_i64_s
            } else if expected_ret_ty == Some(WasmType::I64) && expr_ty == WasmType::F64 {
                code_buf.push(0xB0); // i64.trunc_f64_s
            }
            has_ret = true;
        }

        // Jika fungsi memiliki nilai balik dan belum ada return eksplisit
        let func_info = self.func_table.get(&f.name).cloned();
        if let Some((_, _, sig)) = func_info {
            if !sig.results.is_empty() && !has_ret {
                match sig.results[0] {
                    WasmType::F64 => {
                        code_buf.push(0x44);
                        code_buf.extend_from_slice(&0.0f64.to_le_bytes());
                    }
                    _ => {
                        code_buf.push(0x42);
                        encode_i64_leb128(0, &mut code_buf);
                    }
                }
            }
        }

        code_buf.push(0x0B); // end of function

        // 4. Bungkus dengan deklarasi locals dan ukuran total
        let mut func_payload = Vec::new();
        encode_u32_leb128(local_groups.len() as u32, &mut func_payload);
        for (count, ty) in local_groups {
            encode_u32_leb128(count, &mut func_payload);
            func_payload.push(ty as u8);
        }
        func_payload.extend_from_slice(&code_buf);

        let mut final_code_entry = Vec::new();
        encode_u32_leb128(func_payload.len() as u32, &mut final_code_entry);
        final_code_entry.extend_from_slice(&func_payload);

        Ok(final_code_entry)
    }

    fn infer_wasm_expr_type(
        expr: &Expr,
        known_locals: &HashMap<String, WasmType>,
        func_table: &HashMap<String, (u32, u32, FuncSignature)>,
    ) -> WasmType {
        match expr {
            Expr::Lit(Literal::Float(_), _) => WasmType::F64,
            Expr::Lit(Literal::String(_), _) => WasmType::I32,
            Expr::InterpolatedString(_, _) => WasmType::I32,
            Expr::Ident(name, _) => {
                if name == "PI" || name == "TAU" || name == "E" {
                    WasmType::F64
                } else if let Some(&ty) = known_locals.get(name) {
                    ty
                } else {
                    WasmType::I64
                }
            }
            Expr::Call { callee, .. } => {
                if let Expr::Ident(name, _) = &**callee {
                    match name.as_str() {
                        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
                        | "akar_kuadrat" | "akar_kubik" | "pangkat" | "nilai_mutlak"
                        | "bulat" | "lantai" | "langit_langit" | "tanda"
                        | "log" | "log10" | "log2" | "exp" | "ke_radian" | "ke_derajat"
                        | "min" | "max" | "kanvas_lebar" | "kanvas_tinggi"
                        | "kanvas_mouse_x" | "kanvas_mouse_y" | "kanvas_delta_waktu"
                        | "ke_desimal" | "kanvas_ambil_angka" => WasmType::F64,
                        "kanvas_ambil_teks" | "format_desimal" | "teks_gabung" | "teks_format_angka" | "teks_format_u64" => WasmType::I32,
                        "panjang_teks" | "kanvas_ambil_u64" => WasmType::I64,
                        _ => {
                            if let Some((_, _, sig)) = func_table.get(name) {
                                if sig.results.first() == Some(&WasmType::F64) {
                                    return WasmType::F64;
                                } else if sig.results.first() == Some(&WasmType::I32) {
                                    return WasmType::I32;
                                }
                            }
                            WasmType::I64
                        }
                    }
                } else {
                    WasmType::I64
                }
            }
            Expr::Unary(UnaryOp::Neg, inner, _) => {
                Self::infer_wasm_expr_type(inner, known_locals, func_table)
            }
            Expr::Binary(lhs, op, rhs, _) => {
                if matches!(
                    op,
                    BinaryOp::Eq
                        | BinaryOp::NotEq
                        | BinaryOp::Lt
                        | BinaryOp::LtEq
                        | BinaryOp::Gt
                        | BinaryOp::GtEq
                        | BinaryOp::And
                        | BinaryOp::Or
                ) {
                    WasmType::I64
                } else {
                    let lty = Self::infer_wasm_expr_type(lhs, known_locals, func_table);
                    let rty = Self::infer_wasm_expr_type(rhs, known_locals, func_table);
                    if lty == WasmType::F64 || rty == WasmType::F64 {
                        WasmType::F64
                    } else if lty == WasmType::I32 && rty == WasmType::I32 {
                        WasmType::I32
                    } else {
                        WasmType::I64
                    }
                }
            }
            _ => WasmType::I64,
        }
    }

    fn collect_local_vars(
        block: &Block,
        known_locals: &mut HashMap<String, WasmType>,
        func_table: &HashMap<String, (u32, u32, FuncSignature)>,
        out: &mut Vec<(String, WasmType)>,
    ) {
        for stmt in &block.stmts {
            match stmt {
                Stmt::VarDecl { name, init, explicit_ty, .. } => {
                    let ty = if let Some(t) = explicit_ty {
                        if let TypeExpr::Named(tname, _) = t {
                            if tname == "f64" || tname == "desimal" {
                                WasmType::F64
                            } else if tname == "Teks" || tname == "teks" || tname == "String" || tname == "str" {
                                WasmType::I32
                            } else {
                                WasmType::I64
                            }
                        } else {
                            WasmType::I64
                        }
                    } else {
                        Self::infer_wasm_expr_type(init, known_locals, func_table)
                    };
                    known_locals.insert(name.clone(), ty);
                    out.push((name.clone(), ty));
                }
                Stmt::ForIn { var_name, body, .. } => {
                    known_locals.insert(var_name.clone(), WasmType::I64);
                    out.push((var_name.clone(), WasmType::I64));
                    Self::collect_local_vars(body, known_locals, func_table, out);
                }
                Stmt::If { then_branch, else_branch, .. } => {
                    Self::collect_local_vars(then_branch, known_locals, func_table, out);
                    if let Some(eb) = else_branch {
                        Self::collect_local_vars(eb, known_locals, func_table, out);
                    }
                }
                Stmt::While { body, .. } => {
                    Self::collect_local_vars(body, known_locals, func_table, out);
                }
                _ => {}
            }
        }
    }

    fn compile_stmt(
        &mut self,
        stmt: &Stmt,
        locals_map: &HashMap<String, (u32, WasmType)>,
        out: &mut Vec<u8>,
        has_ret: &mut bool,
        expected_ret_ty: Option<WasmType>,
    ) -> Result<()> {
        match stmt {
            Stmt::VarDecl { name, init, .. } => {
                let expr_ty = self.compile_expr(init, locals_map, out)?;
                if let Some(&(idx, expected_ty)) = locals_map.get(name) {
                    if expected_ty == WasmType::F64 && expr_ty == WasmType::I64 {
                        out.push(0xB9); // f64.convert_i64_s
                    } else if expected_ty == WasmType::F64 && expr_ty == WasmType::I32 {
                        out.push(0xB8); // f64.convert_i32_s
                    } else if expected_ty == WasmType::I64 && expr_ty == WasmType::F64 {
                        out.push(0xB0); // i64.trunc_f64_s
                    } else if expected_ty == WasmType::I64 && expr_ty == WasmType::I32 {
                        out.push(0xAD); // i64.extend_i32_u
                    } else if expected_ty == WasmType::I32 && expr_ty == WasmType::I64 {
                        out.push(0xA7); // i32.wrap_i64
                    }
                    out.push(0x21); // local.set
                    encode_u32_leb128(idx, out);
                }
            }
            Stmt::Assign { target, value, .. } => {
                if let Expr::Ident(name, _) = target {
                    let expr_ty = self.compile_expr(value, locals_map, out)?;
                    if let Some(&(idx, expected_ty)) = locals_map.get(name) {
                        if expected_ty == WasmType::F64 && expr_ty == WasmType::I64 {
                            out.push(0xB9); // f64.convert_i64_s
                        } else if expected_ty == WasmType::F64 && expr_ty == WasmType::I32 {
                            out.push(0xB8); // f64.convert_i32_s
                        } else if expected_ty == WasmType::I64 && expr_ty == WasmType::F64 {
                            out.push(0xB0); // i64.trunc_f64_s
                        } else if expected_ty == WasmType::I64 && expr_ty == WasmType::I32 {
                            out.push(0xAD); // i64.extend_i32_u
                        } else if expected_ty == WasmType::I32 && expr_ty == WasmType::I64 {
                            out.push(0xA7); // i32.wrap_i64
                        }
                        out.push(0x21); // local.set
                        encode_u32_leb128(idx, out);
                    }
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    let expr_ty = self.compile_expr(v, locals_map, out)?;
                    if expected_ret_ty == Some(WasmType::F64) && expr_ty == WasmType::I64 {
                        out.push(0xB9); // f64.convert_i64_s
                    } else if expected_ret_ty == Some(WasmType::F64) && expr_ty == WasmType::I32 {
                        out.push(0xB8); // f64.convert_i32_s
                    } else if expected_ret_ty == Some(WasmType::I64) && expr_ty == WasmType::F64 {
                        out.push(0xB0); // i64.trunc_f64_s
                    } else if expected_ret_ty == Some(WasmType::I64) && expr_ty == WasmType::I32 {
                        out.push(0xAD); // i64.extend_i32_u
                    } else if expected_ret_ty == Some(WasmType::I32) && expr_ty == WasmType::I64 {
                        out.push(0xA7); // i32.wrap_i64
                    }
                }
                out.push(0x0F); // return
                *has_ret = true;
            }
            Stmt::If { cond, then_branch, else_branch, .. } => {
                let cond_ty = self.compile_expr(cond, locals_map, out)?;
                if cond_ty == WasmType::I64 {
                    out.push(0xA7); // i32.wrap_i64
                }
                out.push(0x04); // if
                out.push(0x40); // void block

                for s in &then_branch.stmts {
                    self.compile_stmt(s, locals_map, out, has_ret, expected_ret_ty)?;
                }

                if let Some(eb) = else_branch {
                    out.push(0x05); // else
                    for s in &eb.stmts {
                        self.compile_stmt(s, locals_map, out, has_ret, expected_ret_ty)?;
                    }
                }

                out.push(0x0B); // end
            }
            Stmt::While { cond, body, .. } => {
                out.push(0x02); // block
                out.push(0x40); // void
                out.push(0x03); // loop
                out.push(0x40); // void

                let cond_ty = self.compile_expr(cond, locals_map, out)?;
                if cond_ty == WasmType::I64 {
                    out.push(0xA7); // i32.wrap_i64
                }
                out.push(0x45); // i32.eqz
                out.push(0x0D); // br_if 1 (keluar dari loop ke enclosing block)
                encode_u32_leb128(1, out);

                for s in &body.stmts {
                    self.compile_stmt(s, locals_map, out, has_ret, expected_ret_ty)?;
                }

                out.push(0x0C); // br 0 (ulangi loop)
                encode_u32_leb128(0, out);

                out.push(0x0B); // end loop
                out.push(0x0B); // end block
            }
            Stmt::Expr(expr) => {
                let ty = self.compile_expr(expr, locals_map, out)?;
                if !matches!(expr, Expr::Call { .. }) || ty != WasmType::I32 {
                    out.push(0x1A); // drop nilai ekspresi yang tidak dipakai
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_expr(
        &mut self,
        expr: &Expr,
        locals_map: &HashMap<String, (u32, WasmType)>,
        out: &mut Vec<u8>,
    ) -> Result<WasmType> {
        match expr {
            Expr::Lit(Literal::Int(n), _) => {
                out.push(0x42); // i64.const
                encode_i64_leb128(*n as i64, out);
                Ok(WasmType::I64)
            }
            Expr::Lit(Literal::Float(f), _) => {
                out.push(0x44); // f64.const
                out.extend_from_slice(&f.to_le_bytes());
                Ok(WasmType::F64)
            }
            Expr::Lit(Literal::Bool(b), _) => {
                out.push(0x42); // i64.const
                encode_i64_leb128(if *b { 1 } else { 0 }, out);
                Ok(WasmType::I64)
            }
            Expr::Lit(Literal::String(s), _) => {
                let (offset, _) = self.intern_string(s);
                out.push(0x41); // i32.const
                encode_i64_leb128(offset as i64, out);
                Ok(WasmType::I32)
            }
            Expr::InterpolatedString(parts, _) => {
                let has_expr = parts.iter().any(|p| matches!(p, InterpolatedExprPart::Expr(_)));
                if !has_expr {
                    let mut static_str = String::new();
                    for p in parts {
                        if let InterpolatedExprPart::Lit(s) = p {
                            static_str.push_str(s);
                        }
                    }
                    let (offset, _) = self.intern_string(&static_str);
                    out.push(0x41); // i32.const
                    encode_i64_leb128(offset as i64, out);
                    Ok(WasmType::I32)
                } else {
                    let (empty_off, _) = self.intern_string("");
                    out.push(0x41); // i32.const base accumulator
                    encode_i64_leb128(empty_off as i64, out);

                    let gabung_idx = self.func_table.get("teks_gabung").map(|(idx, _, _)| *idx);
                    let fmt_f64_idx = self.func_table.get("teks_format_angka").map(|(idx, _, _)| *idx);
                    let fmt_i64_idx = self.func_table.get("teks_format_u64").map(|(idx, _, _)| *idx);

                    for part in parts {
                        match part {
                            InterpolatedExprPart::Lit(s) => {
                                if !s.is_empty() {
                                    let (off, _) = self.intern_string(s);
                                    out.push(0x41); // i32.const
                                    encode_i64_leb128(off as i64, out);
                                    if let Some(idx) = gabung_idx {
                                        out.push(0x10); // call teks_gabung
                                        encode_u32_leb128(idx, out);
                                    }
                                }
                            }
                            InterpolatedExprPart::Expr(e) => {
                                let ty = self.compile_expr(e, locals_map, out)?;
                                match ty {
                                    WasmType::I32 => {}
                                    WasmType::F64 => {
                                        if let Some(idx) = fmt_f64_idx {
                                            out.push(0x10); // call teks_format_angka
                                            encode_u32_leb128(idx, out);
                                        }
                                    }
                                    WasmType::I64 => {
                                        if let Some(idx) = fmt_i64_idx {
                                            out.push(0x10); // call teks_format_u64
                                            encode_u32_leb128(idx, out);
                                        }
                                    }
                                    _ => {}
                                }
                                if let Some(idx) = gabung_idx {
                                    out.push(0x10); // call teks_gabung
                                    encode_u32_leb128(idx, out);
                                }
                            }
                        }
                    }
                    Ok(WasmType::I32)
                }
            }
            Expr::Ident(name, span) => {
                if let Some(&(idx, ty)) = locals_map.get(name) {
                    out.push(0x20); // local.get
                    encode_u32_leb128(idx, out);
                    Ok(ty)
                } else if name == "PI" {
                    out.push(0x44); // f64.const
                    out.extend_from_slice(&std::f64::consts::PI.to_le_bytes());
                    Ok(WasmType::F64)
                } else if name == "TAU" {
                    out.push(0x44); // f64.const
                    out.extend_from_slice(&std::f64::consts::TAU.to_le_bytes());
                    Ok(WasmType::F64)
                } else if name == "E" {
                    out.push(0x44); // f64.const
                    out.extend_from_slice(&std::f64::consts::E.to_le_bytes());
                    Ok(WasmType::F64)
                } else {
                    Err(Diagnostic::error(format!("Variabel '{}' tidak ditemukan", name))
                        .with_span(*span))
                }
            }
            Expr::Binary(lhs, op, rhs, _) => {
                let mut lhs_buf = Vec::new();
                let ty_left = self.compile_expr(lhs, locals_map, &mut lhs_buf)?;
                let mut rhs_buf = Vec::new();
                let ty_right = self.compile_expr(rhs, locals_map, &mut rhs_buf)?;

                let is_float = ty_left == WasmType::F64 || ty_right == WasmType::F64;
                if is_float {
                    if ty_left == WasmType::I64 {
                        lhs_buf.push(0xB9); // f64.convert_i64_s
                    } else if ty_left == WasmType::I32 {
                        lhs_buf.push(0xB8); // f64.convert_i32_s
                    }
                    if ty_right == WasmType::I64 {
                        rhs_buf.push(0xB9); // f64.convert_i64_s
                    } else if ty_right == WasmType::I32 {
                        rhs_buf.push(0xB8); // f64.convert_i32_s
                    }
                    out.extend(lhs_buf);
                    out.extend(rhs_buf);

                    match op {
                        BinaryOp::Add => { out.push(0xA0); Ok(WasmType::F64) }
                        BinaryOp::Sub => { out.push(0xA1); Ok(WasmType::F64) }
                        BinaryOp::Mul => { out.push(0xA2); Ok(WasmType::F64) }
                        BinaryOp::Div => { out.push(0xA3); Ok(WasmType::F64) }
                        BinaryOp::Eq => { out.push(0x61); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::NotEq => { out.push(0x62); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::Lt => { out.push(0x63); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::Gt => { out.push(0x64); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::LtEq => { out.push(0x65); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::GtEq => { out.push(0x66); out.push(0xAD); Ok(WasmType::I64) }
                        _ => { out.push(0xA0); Ok(WasmType::F64) }
                    }
                } else if ty_left == WasmType::I32 && ty_right == WasmType::I32 {
                    out.extend(lhs_buf);
                    out.extend(rhs_buf);

                    match op {
                        BinaryOp::Add => { out.push(0x6A); Ok(WasmType::I32) } // i32.add
                        BinaryOp::Sub => { out.push(0x6B); Ok(WasmType::I32) } // i32.sub
                        BinaryOp::Mul => { out.push(0x6C); Ok(WasmType::I32) } // i32.mul
                        BinaryOp::Div => { out.push(0x6D); Ok(WasmType::I32) } // i32.div_s
                        BinaryOp::Mod => { out.push(0x6F); Ok(WasmType::I32) } // i32.rem_s
                        BinaryOp::And => { out.push(0x71); Ok(WasmType::I32) } // i32.and
                        BinaryOp::Or => { out.push(0x72); Ok(WasmType::I32) }  // i32.or
                        BinaryOp::Eq => { out.push(0x46); out.push(0xAD); Ok(WasmType::I64) } // i32.eq, i64.extend_i32_u
                        BinaryOp::NotEq => { out.push(0x47); out.push(0xAD); Ok(WasmType::I64) } // i32.ne, i64.extend_i32_u
                        BinaryOp::Lt => { out.push(0x48); out.push(0xAD); Ok(WasmType::I64) } // i32.lt_s, i64.extend_i32_u
                        BinaryOp::Gt => { out.push(0x4A); out.push(0xAD); Ok(WasmType::I64) } // i32.gt_s, i64.extend_i32_u
                        BinaryOp::LtEq => { out.push(0x4C); out.push(0xAD); Ok(WasmType::I64) } // i32.le_s, i64.extend_i32_u
                        BinaryOp::GtEq => { out.push(0x4E); out.push(0xAD); Ok(WasmType::I64) } // i32.ge_s, i64.extend_i32_u
                    }
                } else {
                    if ty_left == WasmType::I32 {
                        lhs_buf.push(0xAD); // i64.extend_i32_u
                    }
                    if ty_right == WasmType::I32 {
                        rhs_buf.push(0xAD); // i64.extend_i32_u
                    }
                    out.extend(lhs_buf);
                    out.extend(rhs_buf);

                    match op {
                        BinaryOp::Add => { out.push(0x7C); Ok(WasmType::I64) }
                        BinaryOp::Sub => { out.push(0x7D); Ok(WasmType::I64) }
                        BinaryOp::Mul => { out.push(0x7E); Ok(WasmType::I64) }
                        BinaryOp::Div => { out.push(0x7F); Ok(WasmType::I64) }
                        BinaryOp::Mod => { out.push(0x81); Ok(WasmType::I64) }
                        BinaryOp::And => { out.push(0x83); Ok(WasmType::I64) }
                        BinaryOp::Or => { out.push(0x84); Ok(WasmType::I64) }
                        BinaryOp::Eq => { out.push(0x51); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::NotEq => { out.push(0x52); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::Lt => { out.push(0x53); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::Gt => { out.push(0x55); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::LtEq => { out.push(0x57); out.push(0xAD); Ok(WasmType::I64) }
                        BinaryOp::GtEq => { out.push(0x59); out.push(0xAD); Ok(WasmType::I64) }
                    }
                }
            }
            Expr::Unary(UnaryOp::Neg, inner, _) => {
                let ty = self.compile_expr(inner, locals_map, out)?;
                if ty == WasmType::F64 {
                    out.push(0x9A); // f64.neg
                    Ok(WasmType::F64)
                } else {
                    // 0 - x
                    let mut temp = Vec::new();
                    temp.push(0x42);
                    encode_i64_leb128(0, &mut temp);
                    temp.extend_from_slice(out);
                    temp.push(0x7D); // i64.sub
                    *out = temp;
                    Ok(WasmType::I64)
                }
            }
            Expr::Unary(UnaryOp::Not, inner, _) => {
                let _ = self.compile_expr(inner, locals_map, out)?;
                out.push(0x50); // i64.eqz
                out.push(0xAD); // i64.extend_i32_u
                Ok(WasmType::I64)
            }
            Expr::Call { callee, args, span } => {
                if let Expr::Ident(name, _) = &**callee {
                    if name == "cetak" {
                        if !args.is_empty() {
                            let arg = &args[0].value;
                            if let Expr::Lit(Literal::String(s), _) = arg {
                                let (off, len) = self.intern_string(s);
                                out.push(0x41); // i32.const
                                encode_i64_leb128(off as i64, out);
                                out.push(0x41); // i32.const
                                encode_i64_leb128(len as i64, out);
                                out.push(0x10); // call
                                encode_u32_leb128(2, out); // env.cetak_teks (func 2)
                            } else if let Expr::InterpolatedString(parts, _) = arg {
                                for p in parts {
                                    match p {
                                        InterpolatedExprPart::Lit(s) => {
                                            if !s.is_empty() {
                                                let (off, len) = self.intern_string(s);
                                                out.push(0x41);
                                                encode_i64_leb128(off as i64, out);
                                                out.push(0x41);
                                                encode_i64_leb128(len as i64, out);
                                                out.push(0x10);
                                                encode_u32_leb128(2, out);
                                            }
                                        }
                                        InterpolatedExprPart::Expr(e) => {
                                            let ty = self.compile_expr(e, locals_map, out)?;
                                            if ty == WasmType::F64 {
                                                out.push(0x10);
                                                encode_u32_leb128(1, out); // env.cetak_f64
                                            } else {
                                                out.push(0x10);
                                                encode_u32_leb128(0, out); // env.cetak_u64
                                            }
                                        }
                                    }
                                }
                            } else {
                                let ty = self.compile_expr(arg, locals_map, out)?;
                                if ty == WasmType::F64 {
                                    out.push(0x10); // call
                                    encode_u32_leb128(1, out); // env.cetak_f64 (func 1)
                                } else {
                                    out.push(0x10); // call
                                    encode_u32_leb128(0, out); // env.cetak_u64 (func 0)
                                }
                            }
                        }
                        return Ok(WasmType::I32);
                    }

                    if (name == "kanvas_teks" || name == "kanvas_teks_tengah") && args.len() == 4 {
                        let arg0 = &args[0].value;
                        if let Expr::Lit(Literal::String(s), _) = arg0 {
                            let (off, len) = self.intern_string(s);
                            out.push(0x41); // i32.const
                            encode_i64_leb128(off as i64, out);
                            out.push(0x41); // i32.const
                            encode_i64_leb128(len as i64, out);
                        } else if let Expr::InterpolatedString(parts, _) = arg0 {
                            let mut static_str = String::new();
                            for p in parts {
                                if let InterpolatedExprPart::Lit(s) = p {
                                    static_str.push_str(s);
                                }
                            }
                            let (off, len) = self.intern_string(&static_str);
                            out.push(0x41);
                            encode_i64_leb128(off as i64, out);
                            out.push(0x41);
                            encode_i64_leb128(len as i64, out);
                        } else {
                            let _ = self.compile_expr(arg0, locals_map, out)?;
                            out.push(0x41);
                            encode_i64_leb128(0, out);
                        }

                        for arg in &args[1..4] {
                            let ty = self.compile_expr(&arg.value, locals_map, out)?;
                            if ty == WasmType::I64 {
                                out.push(0xB9); // f64.convert_i64_s
                            }
                        }

                        if let Some((func_idx, _, _)) = self.func_table.get(name).cloned() {
                            out.push(0x10); // call
                            encode_u32_leb128(func_idx, out);
                        }
                        return Ok(WasmType::I32);
                    }

                    if name == "audio_frekuensi_nada" && args.len() == 1 {
                        let arg0 = &args[0].value;
                        if let Expr::Lit(Literal::String(s), _) = arg0 {
                            let (off, len) = self.intern_string(s);
                            out.push(0x41); // i32.const
                            encode_i64_leb128(off as i64, out);
                            out.push(0x41); // i32.const
                            encode_i64_leb128(len as i64, out);
                        } else if let Expr::InterpolatedString(parts, _) = arg0 {
                            let mut static_str = String::new();
                            for p in parts {
                                if let InterpolatedExprPart::Lit(s) = p {
                                    static_str.push_str(s);
                                }
                            }
                            let (off, len) = self.intern_string(&static_str);
                            out.push(0x41);
                            encode_i64_leb128(off as i64, out);
                            out.push(0x41);
                            encode_i64_leb128(len as i64, out);
                        } else {
                            let _ = self.compile_expr(arg0, locals_map, out)?;
                            out.push(0x41);
                            encode_i64_leb128(0, out);
                        }

                        if let Some((func_idx, _, _)) = self.func_table.get("audio_frekuensi_nada").cloned() {
                            out.push(0x10); // call
                            encode_u32_leb128(func_idx, out);
                        }
                        return Ok(WasmType::F64);
                    }

                    // Operasi matematika perangkat keras bawaan WebAssembly MVP (Hardware Native)
                    if name == "akar_kuadrat" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); } // f64.convert_i64_s
                        out.push(0x9F); // f64.sqrt
                        return Ok(WasmType::F64);
                    }
                    if name == "nilai_mutlak" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); } // f64.convert_i64_s
                        out.push(0x99); // f64.abs
                        return Ok(WasmType::F64);
                    }
                    if name == "bulat" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); } // f64.convert_i64_s
                        out.push(0x9E); // f64.nearest
                        return Ok(WasmType::F64);
                    }
                    if name == "lantai" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); } // f64.convert_i64_s
                        out.push(0x9C); // f64.floor
                        return Ok(WasmType::F64);
                    }
                    if name == "langit_langit" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); } // f64.convert_i64_s
                        out.push(0x9B); // f64.ceil
                        return Ok(WasmType::F64);
                    }
                    if name == "min" && args.len() == 2 {
                        let t1 = self.compile_expr(&args[0].value, locals_map, out)?;
                        if t1 == WasmType::I64 { out.push(0xB9); }
                        let t2 = self.compile_expr(&args[1].value, locals_map, out)?;
                        if t2 == WasmType::I64 { out.push(0xB9); }
                        out.push(0xA4); // f64.min
                        return Ok(WasmType::F64);
                    }
                    if name == "max" && args.len() == 2 {
                        let t1 = self.compile_expr(&args[0].value, locals_map, out)?;
                        if t1 == WasmType::I64 { out.push(0xB9); }
                        let t2 = self.compile_expr(&args[1].value, locals_map, out)?;
                        if t2 == WasmType::I64 { out.push(0xB9); }
                        out.push(0xA5); // f64.max
                        return Ok(WasmType::F64);
                    }
                    if name == "ke_radian" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); }
                        out.push(0x44); // f64.const
                        let factor = std::f64::consts::PI / 180.0;
                        out.extend_from_slice(&factor.to_le_bytes());
                        out.push(0xA2); // f64.mul
                        return Ok(WasmType::F64);
                    }
                    if name == "ke_derajat" && args.len() == 1 {
                        let ty = self.compile_expr(&args[0].value, locals_map, out)?;
                        if ty == WasmType::I64 { out.push(0xB9); }
                        out.push(0x44); // f64.const
                        let factor = 180.0 / std::f64::consts::PI;
                        out.extend_from_slice(&factor.to_le_bytes());
                        out.push(0xA2); // f64.mul
                        return Ok(WasmType::F64);
                    }

                    if let Some((func_idx, _, sig)) = self.func_table.get(name).cloned() {
                        for (idx, arg) in args.iter().enumerate() {
                            let expected_ty = sig.params.get(idx).copied();
                            let ty = self.compile_expr(&arg.value, locals_map, out)?;
                            if expected_ty == Some(WasmType::F64) && ty == WasmType::I64 {
                                out.push(0xB9); // f64.convert_i64_s
                            }
                        }
                        out.push(0x10); // call
                        encode_u32_leb128(func_idx, out);
                        if let Some(&ret_ty) = sig.results.first() {
                            return Ok(ret_ty);
                        } else {
                            return Ok(WasmType::I32);
                        }
                    }
                }
                Err(Diagnostic::error("Pemanggilan fungsi tidak dikenal pada target WebAssembly")
                    .with_span(*span))
            }
            _ => Ok(WasmType::I64),
        }
    }
}

/// Helper encoding LEB128 unsigned 32-bit integer
pub fn encode_u32_leb128(mut val: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if val == 0 {
            break;
        }
    }
}

/// Helper encoding LEB128 signed 64-bit integer
pub fn encode_i64_leb128(mut val: i64, out: &mut Vec<u8>) {
    let mut more = true;
    while more {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if (val == 0 && (byte & 0x40) == 0) || (val == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        out.push(byte);
    }
}

fn emit_section(section_id: u8, payload: &[u8], out: &mut Vec<u8>) {
    out.push(section_id);
    encode_u32_leb128(payload.len() as u32, out);
    out.extend_from_slice(payload);
}

fn emit_name(name: &str, out: &mut Vec<u8>) {
    let bytes = name.as_bytes();
    encode_u32_leb128(bytes.len() as u32, out);
    out.extend_from_slice(bytes);
}

/// Tulis berkas .wasm beserta template peluncur Web (HTML & JS)
pub fn write_wasm_artifacts(wasm_bytes: &[u8], wasm_path: &Path) -> io::Result<()> {
    fs::write(wasm_path, wasm_bytes)?;

    let stem = wasm_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dapcode_app");
    let wasm_filename = wasm_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("app.wasm");

    // Hasilkan peluncur Node.js (<nama>.js)
    let js_path = wasm_path.with_extension("js");
    let js_code = format!(
        r#"// Peluncur Node.js untuk Modul WebAssembly DapCode ({})
const fs = require('fs');
const path = require('path');

const wasmBuffer = fs.readFileSync(path.join(__dirname, '{}'));
let memory;
const kvStore = new Map();
let wasmStrHeap = 48000;
function decodeStr(ptr) {{
    if (!ptr || !memory) return '';
    const u8 = new Uint8Array(memory.buffer);
    let end = ptr;
    while (end < u8.length && u8[end] !== 0) end++;
    return new TextDecoder().decode(u8.subarray(ptr, end));
}}
function writeStr(str) {{
    if (!memory) return 0;
    const enc = new TextEncoder().encode(str);
    const u8 = new Uint8Array(memory.buffer);
    const p = wasmStrHeap;
    u8.set(enc, p);
    u8[p + enc.length] = 0;
    wasmStrHeap += enc.length + 1;
    if (wasmStrHeap > 64000) wasmStrHeap = 48000;
    return p;
}}

const importObject = {{
    env: {{
        cetak_u64: (val) => console.log(val.toString()),
        cetak_f64: (val) => console.log(val),
        cetak_teks: (ptr, len) => {{
            const arr = new Uint8Array(memory.buffer, ptr, len);
            console.log(new TextDecoder().decode(arr));
        }},
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
        kanvas_bersihkan: () => {{}},
        kanvas_warna_isi: () => {{}},
        kanvas_warna_garis: () => {{}},
        kanvas_tebal_garis: () => {{}},
        kanvas_persegi: () => {{}},
        kanvas_bingkai_persegi: () => {{}},
        kanvas_persegi_bulat: () => {{}},
        kanvas_bingkai_persegi_bulat: () => {{}},
        kanvas_lingkaran: () => {{}},
        kanvas_bingkai_lingkaran: () => {{}},
        kanvas_garis: () => {{}},
        kanvas_titik: () => {{}},
        kanvas_teks: () => {{}},
        kanvas_teks_tengah: () => {{}},
        kanvas_lebar: () => 800.0,
        kanvas_tinggi: () => 480.0,
        kanvas_mouse_x: () => 0.0,
        kanvas_mouse_y: () => 0.0,
        kanvas_mouse_klik: () => 0n,
        kanvas_tombol_ditekan: () => 0n,
        kanvas_delta_waktu: () => 0.0166,
        waktu_sekarang_ms: () => BigInt(Date.now()),
        kanvas_simpan_teks: (kPtr, vPtr) => {{ kvStore.set(decodeStr(kPtr), decodeStr(vPtr)); }},
        kanvas_ambil_teks: (kPtr) => writeStr(kvStore.get(decodeStr(kPtr)) || ''),
        kanvas_simpan_u64: (kPtr, val) => {{ kvStore.set(decodeStr(kPtr), BigInt(val)); }},
        kanvas_ambil_u64: (kPtr) => kvStore.get(decodeStr(kPtr)) !== undefined ? BigInt(kvStore.get(decodeStr(kPtr))) : 0n,
        kanvas_simpan_angka: (kPtr, val) => {{ kvStore.set(decodeStr(kPtr), Number(val)); }},
        kanvas_ambil_angka: (kPtr) => kvStore.get(decodeStr(kPtr)) !== undefined ? Number(kvStore.get(decodeStr(kPtr))) : 0.0,
        ke_desimal: (sPtr) => {{ const n = parseFloat(decodeStr(sPtr)); return isNaN(n) ? 0.0 : n; }},
        format_desimal: (val) => writeStr((Math.round(val * 10000) / 10000).toString()),
        panjang_teks: (sPtr) => BigInt(decodeStr(sPtr).length),
        teks_gabung: (s1Ptr, s2Ptr) => writeStr(decodeStr(s1Ptr) + decodeStr(s2Ptr)),
        teks_format_angka: (val) => writeStr((Math.round(val * 10000) / 10000).toString()),
        teks_format_u64: (val) => writeStr(val.toString()),
    }}
}};

WebAssembly.instantiate(wasmBuffer, importObject).then(({{ instance }}) => {{
    memory = instance.exports.memory;
    console.log('[DapCode WASM] Modul {} berhasil dimuat.');
    if (instance.exports.utama) {{
        const hasil = instance.exports.utama();
        if (hasil !== undefined) console.log('[DapCode WASM] Hasil utama():', hasil);
    }}
}}).catch(err => {{
    console.error('[DapCode WASM] Gagal memuat modul:', err);
}});
"#,
        wasm_filename, wasm_filename, stem
    );
    let _ = fs::write(&js_path, js_code);

    // Hasilkan peluncur Web Browser (<nama>.html)
    let html_path = wasm_path.with_extension("html");
    let html_code = format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DapCode WebAssembly Runner - {}</title>
    <style>
        :root {{
            --bg: #090d16;
            --card-bg: rgba(30, 41, 59, 0.7);
            --border: rgba(148, 163, 184, 0.15);
            --primary: #38bdf8;
            --accent: #818cf8;
            --text: #f8fafc;
            --muted: #94a3b8;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: radial-gradient(circle at top right, #1e1b4b, var(--bg));
            color: var(--text);
            min-height: 100vh;
            padding: 2.5rem 1.5rem;
            display: flex;
            justify-content: center;
        }}
        .container {{ width: 100%; max-width: 860px; }}
        .header {{ margin-bottom: 2rem; }}
        .badge {{
            display: inline-block;
            background: rgba(56, 189, 248, 0.15);
            color: var(--primary);
            font-size: 0.8rem;
            font-weight: 700;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            border: 1px solid rgba(56, 189, 248, 0.3);
            margin-bottom: 0.75rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }}
        h1 {{ font-size: 2.2rem; font-weight: 800; background: linear-gradient(135deg, #fff, var(--primary)); -webkit-background-clip: text; -webkit-text-fill-color: transparent; margin-bottom: 0.5rem; }}
        p.subtitle {{ color: var(--muted); font-size: 1.05rem; }}
        .card {{
            background: var(--card-bg);
            backdrop-filter: blur(12px);
            border: 1px solid var(--border);
            border-radius: 1rem;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
            box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.5);
        }}
        .canvas-card {{
            background: #0b1120;
            border: 1px solid rgba(56, 189, 248, 0.25);
            padding: 1.25rem;
        }}
        .canvas-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            flex-wrap: wrap;
            gap: 0.75rem;
            margin-bottom: 0.85rem;
        }}
        .canvas-title {{
            font-size: 1.1rem;
            font-weight: 700;
            color: var(--primary);
        }}
        .frame-controls {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            flex-wrap: wrap;
        }}
        .frame-select {{
            background: #1e293b;
            color: #f8fafc;
            border: 1px solid rgba(148, 163, 184, 0.25);
            padding: 0.35rem 0.65rem;
            border-radius: 0.4rem;
            font-size: 0.85rem;
            outline: none;
            cursor: pointer;
        }}
        .dim-badge {{
            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
            font-size: 0.8rem;
            color: var(--primary);
            background: rgba(56, 189, 248, 0.1);
            padding: 0.25rem 0.6rem;
            border-radius: 9999px;
            border: 1px solid rgba(56, 189, 248, 0.3);
        }}
        .canvas-viewport {{
            width: 100%;
            background: #060913;
            border: 1px solid rgba(56, 189, 248, 0.2);
            border-radius: 0.6rem;
            display: flex;
            justify-content: center;
            align-items: center;
            padding: 0.75rem;
            overflow: hidden;
            box-shadow: inset 0 2px 10px rgba(0, 0, 0, 0.7);
        }}
        #canvas {{
            display: block;
            max-width: 100%;
            max-height: 60vh;
            width: auto;
            height: auto;
            border-radius: 6px;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.6);
            touch-action: none;
            cursor: crosshair;
        }}
        .btn-group {{ display: flex; gap: 0.75rem; margin-bottom: 1.25rem; }}
        button {{
            background: linear-gradient(135deg, #0284c7, #2563eb);
            color: white;
            border: none;
            padding: 0.7rem 1.4rem;
            border-radius: 0.5rem;
            font-weight: 600;
            font-size: 0.95rem;
            cursor: pointer;
            transition: all 0.2s ease;
            box-shadow: 0 4px 14px rgba(2, 132, 199, 0.3);
        }}
        button:hover {{ transform: translateY(-1px); box-shadow: 0 6px 20px rgba(2, 132, 199, 0.45); }}
        button.btn-clear {{ background: rgba(51, 65, 85, 0.8); box-shadow: none; border: 1px solid var(--border); }}
        button.btn-clear:hover {{ background: #334155; }}
        #terminal {{
            background: #030712;
            border: 1px solid #1f2937;
            border-radius: 0.75rem;
            padding: 1.25rem;
            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
            font-size: 0.95rem;
            line-height: 1.6;
            min-height: 160px;
            white-space: pre-wrap;
            color: #4ade80;
            box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.8);
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <span class="badge">WebAssembly (WASM) 1.0 MVP</span>
            <h1>DapCode Web Runner</h1>
            <p class="subtitle">Modul biner <code>{}</code> dijalankan secara native di mesin peramban web Anda.</p>
        </div>

        <div class="card canvas-card" id="canvas-card">
            <div class="canvas-header">
                <span class="canvas-title">🎨 Kanvas 2D Interaktif</span>
                <div class="frame-controls">
                    <select id="frame-select" class="frame-select">
                        <option value="desktop_std" selected>Desktop (800×480)</option>
                        <option value="desktop_hd">Desktop HD (960×540)</option>
                        <option value="mobile_portrait">Ponsel (360×640)</option>
                        <option value="mobile_landscape">Ponsel Lanskap (640×360)</option>
                        <option value="tablet">Tablet (768×576)</option>
                        <option value="persegi">Persegi (500×500)</option>
                    </select>
                    <span id="canvas-dim-badge" class="dim-badge">800 × 480 px</span>
                </div>
            </div>
            <div class="canvas-viewport" id="canvas-viewport">
                <canvas id="canvas" width="800" height="480"></canvas>
            </div>
        </div>

        <div class="card">
            <div class="btn-group">
                <button id="btn-run">▶ Jalankan Program</button>
                <button id="btn-clear" class="btn-clear">Bersihkan Konsol</button>
            </div>
            <div id="terminal">[Sistem] Menunggu inisialisasi WebAssembly...</div>
        </div>
    </div>

    <script>
        const terminal = document.getElementById('terminal');
        const canvas = document.getElementById('canvas');
        const ctx = canvas ? canvas.getContext('2d') : null;
        const frameSelect = document.getElementById('frame-select');
        const dimBadge = document.getElementById('canvas-dim-badge');

        let mouseX = 400.0, mouseY = 240.0, mouseClick = 0n;

        function updateCanvasPointer(cx, cy) {{
            if (!canvas) return;
            const r = canvas.getBoundingClientRect();
            if (r.width <= 0 || r.height <= 0) return;
            mouseX = Math.max(0, Math.min(canvas.width, (cx - r.left) * (canvas.width / r.width)));
            mouseY = Math.max(0, Math.min(canvas.height, (cy - r.top) * (canvas.height / r.height)));
        }}

        if (canvas) {{
            canvas.addEventListener('mousemove', (e) => {{
                updateCanvasPointer(e.clientX, e.clientY);
                if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
            }});
            canvas.addEventListener('mousedown', (e) => {{
                mouseClick = 1n;
                updateCanvasPointer(e.clientX, e.clientY);
                if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
            }});
            canvas.addEventListener('mouseup', () => {{
                mouseClick = 0n;
                if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
            }});
            canvas.addEventListener('touchstart', (e) => {{
                const t = e.touches[0];
                if (t) {{
                    mouseClick = 1n;
                    updateCanvasPointer(t.clientX, t.clientY);
                    if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
                }}
                e.preventDefault();
            }}, {{ passive: false }});
            canvas.addEventListener('touchmove', (e) => {{
                const t = e.touches[0];
                if (t) {{
                    updateCanvasPointer(t.clientX, t.clientY);
                    if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
                }}
                e.preventDefault();
            }}, {{ passive: false }});
            canvas.addEventListener('touchend', (e) => {{
                mouseClick = 0n;
                if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
                e.preventDefault();
            }}, {{ passive: false }});
        }}

        function setFrame(w, h) {{
            if (!canvas) return;
            canvas.width = w;
            canvas.height = h;
            if (dimBadge) dimBadge.textContent = w + ' × ' + h + ' px';
            if (wasmInstance && wasmInstance.exports.utama) wasmInstance.exports.utama();
        }}

        if (frameSelect) {{
            frameSelect.addEventListener('change', (e) => {{
                const v = e.target.value;
                if (v === 'desktop_std') setFrame(800, 480);
                else if (v === 'desktop_hd') setFrame(960, 540);
                else if (v === 'mobile_portrait') setFrame(360, 640);
                else if (v === 'mobile_landscape') setFrame(640, 360);
                else if (v === 'tablet') setFrame(768, 576);
                else if (v === 'persegi') setFrame(500, 500);
            }});
        }}

        function log(msg) {{
            const time = new Date().toLocaleTimeString();
            terminal.textContent += `\n[${{time}}] ${{msg}}`;
        }}

        let wasmInstance = null;
        let memory = null;

        async function initWasm() {{
            try {{
                terminal.textContent = '[Sistem] Memuat {} ...';
                const resp = await fetch('{}');
                const bytes = await resp.arrayBuffer();

                const kvStore = new Map();
                let wasmStrHeap = 48000;
                function decodeStr(ptr) {{
                    if (!ptr || !memory) return '';
                    const u8 = new Uint8Array(memory.buffer);
                    let end = ptr;
                    while (end < u8.length && u8[end] !== 0) end++;
                    return new TextDecoder().decode(u8.subarray(ptr, end));
                }}
                function writeStr(str) {{
                    if (!memory) return 0;
                    const enc = new TextEncoder().encode(str);
                    const u8 = new Uint8Array(memory.buffer);
                    const p = wasmStrHeap;
                    u8.set(enc, p);
                    u8[p + enc.length] = 0;
                    wasmStrHeap += enc.length + 1;
                    if (wasmStrHeap > 64000) wasmStrHeap = 48000;
                    return p;
                }}

                const importObject = {{
                    env: {{
                        cetak_u64: (val) => log('Output: ' + val.toString()),
                        cetak_f64: (val) => log('Output: ' + val.toString()),
                        cetak_teks: (ptr, len) => {{
                            const arr = new Uint8Array(memory.buffer, ptr, len);
                            log('Cetak: ' + new TextDecoder().decode(arr));
                        }},
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
                        kanvas_bersihkan: (r, g, b, a) => {{
                            if (!ctx) return;
                            ctx.fillStyle = 'rgba(' + Math.round(r * 255) + ',' + Math.round(g * 255) + ',' + Math.round(b * 255) + ',' + a + ')';
                            ctx.fillRect(0, 0, canvas.width, canvas.height);
                        }},
                        kanvas_warna_isi: (r, g, b, a) => {{
                            if (!ctx) return;
                            ctx.fillStyle = 'rgba(' + Math.round(r * 255) + ',' + Math.round(g * 255) + ',' + Math.round(b * 255) + ',' + a + ')';
                        }},
                        kanvas_warna_garis: (r, g, b, a) => {{
                            if (!ctx) return;
                            ctx.strokeStyle = 'rgba(' + Math.round(r * 255) + ',' + Math.round(g * 255) + ',' + Math.round(b * 255) + ',' + a + ')';
                        }},
                        kanvas_tebal_garis: (t) => {{ if (ctx) ctx.lineWidth = t; }},
                        kanvas_persegi: (x, y, w, h) => {{ if (ctx) ctx.fillRect(x, y, w, h); }},
                        kanvas_bingkai_persegi: (x, y, w, h) => {{ if (ctx) ctx.strokeRect(x, y, w, h); }},
                        kanvas_persegi_bulat: (x, y, w, h, r) => {{
                            if (!ctx) return;
                            ctx.beginPath();
                            if (ctx.roundRect) {{ ctx.roundRect(x, y, w, h, r); }} else {{ ctx.rect(x, y, w, h); }}
                            ctx.fill();
                        }},
                        kanvas_bingkai_persegi_bulat: (x, y, w, h, r) => {{
                            if (!ctx) return;
                            ctx.beginPath();
                            if (ctx.roundRect) {{ ctx.roundRect(x, y, w, h, r); }} else {{ ctx.strokeRect(x, y, w, h); }}
                            ctx.stroke();
                        }},
                        kanvas_lingkaran: (x, y, rad) => {{ if (!ctx) return; ctx.beginPath(); ctx.arc(x, y, rad, 0, Math.PI * 2); ctx.fill(); }},
                        kanvas_bingkai_lingkaran: (x, y, rad) => {{ if (!ctx) return; ctx.beginPath(); ctx.arc(x, y, rad, 0, Math.PI * 2); ctx.stroke(); }},
                        kanvas_garis: (x1, y1, x2, y2) => {{ if (!ctx) return; ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke(); }},
                        kanvas_titik: (x, y, rad) => {{ if (!ctx) return; ctx.beginPath(); ctx.arc(x, y, rad / 2, 0, Math.PI * 2); ctx.fill(); }},
                        kanvas_teks: (ptr, len, x, y, s) => {{
                            if (!ctx) return;
                            const str = decodeStr(ptr);
                            const sz = (typeof s === 'number' && s > 0) ? Math.round(s) : 14;
                            ctx.font = sz + 'px sans-serif';
                            ctx.fillText(str, x, y);
                        }},
                        kanvas_teks_tengah: (ptr, len, cx, cy, s) => {{
                            if (!ctx) return;
                            const str = decodeStr(ptr);
                            const sz = (typeof s === 'number' && s > 0) ? Math.round(s) : 14;
                            ctx.font = sz + 'px sans-serif';
                            ctx.textAlign = 'center';
                            ctx.textBaseline = 'middle';
                            ctx.fillText(str, cx, cy);
                            ctx.textAlign = 'left';
                            ctx.textBaseline = 'alphabetic';
                        }},
                        kanvas_lebar: () => (canvas ? canvas.width : 800.0),
                        kanvas_tinggi: () => (canvas ? canvas.height : 480.0),
                        kanvas_mouse_x: () => mouseX,
                        kanvas_mouse_y: () => mouseY,
                        kanvas_mouse_klik: () => mouseClick,
                        kanvas_tombol_ditekan: () => 0n,
                        kanvas_delta_waktu: () => 0.0166,
                        waktu_sekarang_ms: () => BigInt(Date.now()),
                        kanvas_simpan_teks: (kPtr, vPtr) => {{ kvStore.set(decodeStr(kPtr), decodeStr(vPtr)); }},
                        kanvas_ambil_teks: (kPtr) => writeStr(kvStore.get(decodeStr(kPtr)) || ''),
                        kanvas_simpan_u64: (kPtr, val) => {{ kvStore.set(decodeStr(kPtr), BigInt(val)); }},
                        kanvas_ambil_u64: (kPtr) => kvStore.get(decodeStr(kPtr)) !== undefined ? BigInt(kvStore.get(decodeStr(kPtr))) : 0n,
                        kanvas_simpan_angka: (kPtr, val) => {{ kvStore.set(decodeStr(kPtr), Number(val)); }},
                        kanvas_ambil_angka: (kPtr) => kvStore.get(decodeStr(kPtr)) !== undefined ? Number(kvStore.get(decodeStr(kPtr))) : 0.0,
                        ke_desimal: (sPtr) => {{ const n = parseFloat(decodeStr(sPtr)); return isNaN(n) ? 0.0 : n; }},
                        format_desimal: (val) => writeStr((Math.round(val * 10000) / 10000).toString()),
                        panjang_teks: (sPtr) => BigInt(decodeStr(sPtr).length),
                        teks_gabung: (s1Ptr, s2Ptr) => writeStr(decodeStr(s1Ptr) + decodeStr(s2Ptr)),
                        teks_format_angka: (val) => writeStr((Math.round(val * 10000) / 10000).toString()),
                        teks_format_u64: (val) => writeStr(val.toString()),
                        waktu_sekarang_ms: () => BigInt(Date.now()),
                        audio_mainkan_nada: (freq, dur, vol) => {{
                            try {{
                                const ctx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                                if (ctx.state === 'suspended') ctx.resume();
                                const osc = ctx.createOscillator();
                                const gain = ctx.createGain();
                                osc.type = 'sine';
                                osc.frequency.setValueAtTime(freq, ctx.currentTime);
                                const now = ctx.currentTime;
                                gain.gain.setValueAtTime(0.0001, now);
                                gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                                gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                                osc.connect(gain);
                                gain.connect(ctx.destination);
                                osc.start(now);
                                osc.stop(now + dur + 0.05);
                                log('[Audio] Mainkan nada ' + freq + 'Hz, durasi=' + dur + 's, vol=' + vol);
                            }} catch(e) {{ log('[Audio Error] ' + e.message); }}
                        }},
                        audio_mainkan_gelombang: (tipe, freq, dur, vol) => {{
                            try {{
                                const ctx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                                if (ctx.state === 'suspended') ctx.resume();
                                const types = ['sine', 'square', 'sawtooth', 'triangle'];
                                const osc = ctx.createOscillator();
                                const gain = ctx.createGain();
                                osc.type = types[Number(tipe) % 4] || 'sine';
                                osc.frequency.setValueAtTime(freq, ctx.currentTime);
                                const now = ctx.currentTime;
                                gain.gain.setValueAtTime(0.0001, now);
                                gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                                gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                                osc.connect(gain);
                                gain.connect(ctx.destination);
                                osc.start(now);
                                osc.stop(now + dur + 0.05);
                                log('[Audio] Gelombang ' + osc.type + ' ' + freq + 'Hz, durasi=' + dur + 's');
                            }} catch(e) {{ log('[Audio Error] ' + e.message); }}
                        }},
                        audio_efek_suara: (tipe) => {{
                            try {{
                                const ctx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                                if (ctx.state === 'suspended') ctx.resume();
                                const now = ctx.currentTime;
                                const osc = ctx.createOscillator();
                                const gain = ctx.createGain();
                                osc.connect(gain);
                                gain.connect(ctx.destination);
                                const t = Number(tipe);
                                if (t === 0) {{
                                    osc.type = 'sine';
                                    osc.frequency.setValueAtTime(1000, now);
                                    gain.gain.setValueAtTime(0.3, now);
                                    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.08);
                                    osc.start(now); osc.stop(now + 0.09);
                                    log('[Audio SFX] Bip');
                                }} else if (t === 1) {{
                                    osc.type = 'square';
                                    osc.frequency.setValueAtTime(150, now);
                                    osc.frequency.exponentialRampToValueAtTime(600, now + 0.15);
                                    gain.gain.setValueAtTime(0.25, now);
                                    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.16);
                                    osc.start(now); osc.stop(now + 0.17);
                                    log('[Audio SFX] Lompat');
                                }} else if (t === 2) {{
                                    osc.type = 'sine';
                                    osc.frequency.setValueAtTime(987.77, now);
                                    osc.frequency.setValueAtTime(1318.51, now + 0.08);
                                    gain.gain.setValueAtTime(0.3, now);
                                    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.35);
                                    osc.start(now); osc.stop(now + 0.36);
                                    log('[Audio SFX] Koin');
                                }} else if (t === 3) {{
                                    osc.type = 'sawtooth';
                                    osc.frequency.setValueAtTime(220, now);
                                    osc.frequency.exponentialRampToValueAtTime(30, now + 0.35);
                                    gain.gain.setValueAtTime(0.4, now);
                                    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.38);
                                    osc.start(now); osc.stop(now + 0.39);
                                    log('[Audio SFX] Ledakan');
                                }} else if (t === 4) {{
                                    osc.type = 'triangle';
                                    osc.frequency.setValueAtTime(330, now);
                                    osc.frequency.setValueAtTime(293, now + 0.12);
                                    osc.frequency.setValueAtTime(261, now + 0.24);
                                    osc.frequency.setValueAtTime(196, now + 0.36);
                                    gain.gain.setValueAtTime(0.3, now);
                                    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.55);
                                    osc.start(now); osc.stop(now + 0.56);
                                    log('[Audio SFX] Kalah');
                                }} else if (t === 5) {{
                                    osc.type = 'triangle';
                                    osc.frequency.setValueAtTime(261, now);
                                    osc.frequency.setValueAtTime(329, now + 0.1);
                                    osc.frequency.setValueAtTime(392, now + 0.2);
                                    osc.frequency.setValueAtTime(523, now + 0.3);
                                    gain.gain.setValueAtTime(0.3, now);
                                    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.55);
                                    osc.start(now); osc.stop(now + 0.56);
                                    log('[Audio SFX] Menang');
                                }}
                            }} catch(e) {{ log('[Audio Error] ' + e.message); }}
                        }},
                        audio_frekuensi_nada: (ptr, len) => {{
                            try {{
                                const arr = new Uint8Array(memory.buffer, ptr, len);
                                const note = new TextDecoder().decode(arr).trim().toUpperCase();
                                const map = {{
                                    'C0': 16.35, 'C#0': 17.32, 'DB0': 17.32, 'D0': 18.35, 'D#0': 19.45, 'EB0': 19.45, 'E0': 20.60, 'F0': 21.83, 'F#0': 23.12, 'GB0': 23.12, 'G0': 24.50, 'G#0': 25.96, 'AB0': 25.96, 'A0': 27.50, 'A#0': 29.14, 'BB0': 29.14, 'B0': 30.87,
                                    'C1': 32.70, 'C#1': 34.65, 'DB1': 34.65, 'D1': 36.71, 'D#1': 38.89, 'EB1': 38.89, 'E1': 41.20, 'F1': 43.65, 'F#1': 46.25, 'GB1': 46.25, 'G1': 49.00, 'G#1': 51.91, 'AB1': 51.91, 'A1': 55.00, 'A#1': 58.27, 'BB1': 58.27, 'B1': 61.74,
                                    'C2': 65.41, 'C#2': 69.30, 'DB2': 69.30, 'D2': 73.42, 'D#2': 77.78, 'EB2': 77.78, 'E2': 82.41, 'F2': 87.31, 'F#2': 92.50, 'GB2': 92.50, 'G2': 98.00, 'G#2': 103.83, 'AB2': 103.83, 'A2': 110.00, 'A#2': 116.54, 'BB2': 116.54, 'B2': 123.47,
                                    'C3': 130.81, 'C#3': 138.59, 'DB3': 138.59, 'D3': 146.83, 'D#3': 155.56, 'EB3': 155.56, 'E3': 164.81, 'F3': 174.61, 'F#3': 185.00, 'GB3': 185.00, 'G3': 196.00, 'G#3': 207.65, 'AB3': 207.65, 'A3': 220.00, 'A#3': 233.08, 'BB3': 233.08, 'B3': 246.94,
                                    'C4': 261.63, 'C#4': 277.18, 'DB4': 277.18, 'D4': 293.66, 'D#4': 311.13, 'EB4': 311.13, 'E4': 329.63, 'F4': 349.23, 'F#4': 369.99, 'GB4': 369.99, 'G4': 392.00, 'G#4': 415.30, 'AB4': 415.30, 'A4': 440.00, 'A#4': 466.16, 'BB4': 466.16, 'B4': 493.88,
                                    'C5': 523.25, 'C#5': 554.37, 'DB5': 554.37, 'D5': 587.33, 'D#5': 622.25, 'EB5': 622.25, 'E5': 659.25, 'F5': 698.46, 'F#5': 739.99, 'GB5': 739.99, 'G5': 783.99, 'G#5': 830.61, 'AB5': 830.61, 'A5': 880.00, 'A#5': 932.33, 'BB5': 932.33, 'B5': 987.77,
                                    'C6': 1046.50, 'C#6': 1108.73, 'DB6': 1108.73, 'D6': 1174.66, 'D#6': 1244.51, 'EB6': 1244.51, 'E6': 1318.51, 'F6': 1396.91, 'F#6': 1479.98, 'GB6': 1479.98, 'G6': 1567.98, 'G#6': 1661.22, 'AB6': 1661.22, 'A6': 1760.00, 'A#6': 1864.66, 'BB6': 1864.66, 'B6': 1975.53
                                }};
                                return map[note] || 440.0;
                            }} catch(e) {{ return 440.0; }}
                        }},
                        audio_set_volume: (vol) => {{
                            log('[Audio] Set master volume=' + vol);
                        }},
                        audio_hentikan_semua: () => {{
                            try {{
                                if (window.__dapAudioCtx) {{
                                    window.__dapAudioCtx.close();
                                    window.__dapAudioCtx = null;
                                }}
                                log('[Audio] Hentikan semua audio');
                            }} catch(e) {{}}
                        }},
                    }}
                }};

                const {{ instance }} = await WebAssembly.instantiate(bytes, importObject);
                wasmInstance = instance;
                memory = instance.exports.memory;
                window.dapInstance = instance;

                log('Sukses: Modul WebAssembly siap!');
                const exportNames = Object.keys(instance.exports).filter(k => k !== 'memory');
                log('Ekspor yang tersedia: ' + exportNames.join(', '));

                if (instance.exports.utama) {{
                    log('Mengeksekusi fungsi utama()...');
                    const ret = instance.exports.utama();
                    if (ret !== undefined) log('Nilai Kembali utama(): ' + ret);
                }}
            }} catch (err) {{
                log('Galat: ' + err.message);
            }}
        }}

        document.getElementById('btn-run').addEventListener('click', () => {{
            if (wasmInstance && wasmInstance.exports.utama) {{
                log('Mengeksekusi ulang fungsi utama()...');
                const ret = wasmInstance.exports.utama();
                if (ret !== undefined) log('Nilai Kembali utama(): ' + ret);
            }} else {{
                initWasm();
            }}
        }});

        document.getElementById('btn-clear').addEventListener('click', () => {{
            terminal.textContent = '[Konsol dibersihkan]';
        }});

        initWasm();
    </script>
</body>
</html>
"#,
        stem, wasm_filename, wasm_filename, wasm_filename
    );
    let _ = fs::write(&html_path, html_code);

    Ok(())
}
