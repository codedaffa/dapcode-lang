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
                        | "min" | "max" => WasmType::F64,
                        _ => {
                            if let Some((_, _, sig)) = func_table.get(name) {
                                if sig.results.first() == Some(&WasmType::F64) {
                                    return WasmType::F64;
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
                    } else if expected_ty == WasmType::I64 && expr_ty == WasmType::F64 {
                        out.push(0xB0); // i64.trunc_f64_s
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
                        } else if expected_ty == WasmType::I64 && expr_ty == WasmType::F64 {
                            out.push(0xB0); // i64.trunc_f64_s
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
                    } else if expected_ret_ty == Some(WasmType::I64) && expr_ty == WasmType::F64 {
                        out.push(0xB0); // i64.trunc_f64_s
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
                let ty_left = self.compile_expr(lhs, locals_map, out)?;
                let ty_right = self.compile_expr(rhs, locals_map, out)?;

                let is_float = ty_left == WasmType::F64 || ty_right == WasmType::F64;
                if is_float {
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
                } else {
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
        .container {{ width: 100%; max-width: 800px; }}
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
            min-height: 220px;
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
