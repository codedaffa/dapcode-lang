use crate::ast::*;
use crate::diag::{Diagnostic, Result};
use crate::json::{parse_json, value_to_json, JsonValue};
use crate::span::Span;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub enum Value {
    Int(u64),
    Float(f64),
    String(String),
    Bool(bool),
    Unit,
    Struct {
        name: String,
        fields: HashMap<String, Value>,
    },
    ErrorUnion(std::result::Result<Box<Value>, Box<Value>>),
    Task(Arc<Mutex<Option<thread::JoinHandle<Value>>>>),
    List(Arc<Mutex<Vec<Value>>>),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", if *b { "benar" } else { "salah" }),
            Value::Unit => write!(f, "()"),
            Value::Struct { name, fields } => {
                write!(f, "{}(", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {:?}", k, v)?;
                }
                write!(f, ")")
            }
            Value::ErrorUnion(res) => match res {
                Ok(v) => write!(f, "Sukses({:?})", v),
                Err(e) => write!(f, "Gagal({:?})", e),
            },
            Value::Task(_) => write!(f, "<Tugas>"),
            Value::List(items) => {
                let lock = items.lock().unwrap();
                write!(f, "{:?}", *lock)
            }
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{:.1}", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", if *b { "benar" } else { "salah" }),
            Value::Unit => write!(f, "()"),
            Value::Struct { name, fields } => {
                write!(f, "{}(", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, ")")
            }
            Value::ErrorUnion(res) => match res {
                Ok(v) => write!(f, "{}", v),
                Err(e) => write!(f, "Gagal: {}", e),
            },
            Value::Task(_) => write!(f, "<Tugas>"),
            Value::List(items) => {
                let lock = items.lock().unwrap();
                write!(f, "[")?;
                for (i, item) in lock.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ControlFlow {
    Next(Value),
    Return(Value),
    Fail(Value),
}

#[derive(Clone, Default)]
pub struct Environment {
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn insert(&mut self, name: impl Into<String>, val: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), val);
        }
    }

    pub fn assign(&mut self, name: &str, val: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = val;
                return true;
            }
        }
        false
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}

pub struct Evaluator {
    pub output: Arc<Mutex<Vec<String>>>,
    functions: HashMap<String, FnDecl>,
    // struct_name -> method_name -> FnDecl
    methods: HashMap<String, HashMap<String, FnDecl>>,
}

impl Evaluator {
    pub fn new() -> Self {
        Evaluator {
            output: Arc::new(Mutex::new(Vec::new())),
            functions: HashMap::new(),
            methods: HashMap::new(),
        }
    }

    pub fn get_output(&self) -> Vec<String> {
        self.output.lock().unwrap().clone()
    }

    pub fn run_program(&mut self, program: &Program) -> Result<Value> {
        // Collect functions and impl methods
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                Decl::Impl(i) => {
                    let struct_methods = self.methods.entry(i.target_struct.clone()).or_default();
                    for m in &i.methods {
                        struct_methods.insert(m.name.clone(), m.clone());
                    }
                }
                _ => {}
            }
        }

        // Run 'utama' function
        if let Some(main_fn) = self.functions.get("utama").cloned() {
            let mut env = Environment::new();
            match self.eval_fn(&main_fn, vec![], &mut env)? {
                ControlFlow::Next(v) | ControlFlow::Return(v) => Ok(v),
                ControlFlow::Fail(e) => Err(Diagnostic::error(format!(
                    "Eksekusi gagal pada fungsi utama: {}",
                    e
                ))
                .with_span(main_fn.span)),
            }
        } else {
            Err(Diagnostic::error("Fungsi 'utama()' tidak ditemukan").with_span(program.span))
        }
    }

    pub fn eval_fn(
        &mut self,
        f: &FnDecl,
        args: Vec<Value>,
        _caller_env: &mut Environment,
    ) -> Result<ControlFlow> {
        let mut local_env = Environment::new();

        for (idx, p) in f.params.iter().enumerate() {
            if idx < args.len() {
                local_env.insert(&p.name, args[idx].clone());
            }
        }

        self.eval_block(&f.body, &mut local_env)
    }

    pub fn eval_block(&mut self, block: &Block, env: &mut Environment) -> Result<ControlFlow> {
        env.enter_scope();
        let mut last_val = Value::Unit;

        for stmt in &block.stmts {
            match self.eval_stmt(stmt, env)? {
                ControlFlow::Next(v) => last_val = v,
                ret @ ControlFlow::Return(_) => {
                    env.exit_scope();
                    return Ok(ret);
                }
                fail @ ControlFlow::Fail(_) => {
                    env.exit_scope();
                    return Ok(fail);
                }
            }
        }

        if let Some(expr) = &block.expr {
            last_val = self.eval_expr(expr, env)?;
        }

        env.exit_scope();
        Ok(ControlFlow::Next(last_val))
    }

    pub fn eval_stmt(&mut self, stmt: &Stmt, env: &mut Environment) -> Result<ControlFlow> {
        match stmt {
            Stmt::VarDecl { name, init, .. } => {
                if let Expr::Catch { inner, err_var, handler, .. } = init {
                    let inner_val = self.eval_expr(inner, env)?;
                    match inner_val {
                        Value::ErrorUnion(Err(err_val)) => {
                            env.enter_scope();
                            env.insert(err_var, *err_val);
                            let res = self.eval_block(handler, env)?;
                            env.exit_scope();
                            return match res {
                                ControlFlow::Next(v) => {
                                    env.insert(name, v);
                                    Ok(ControlFlow::Next(Value::Unit))
                                }
                                ControlFlow::Return(v) => Ok(ControlFlow::Return(v)),
                                ControlFlow::Fail(e) => Ok(ControlFlow::Fail(e)),
                            };
                        }
                        Value::ErrorUnion(Ok(ok_val)) => {
                            env.insert(name, *ok_val);
                            return Ok(ControlFlow::Next(Value::Unit));
                        }
                        other => {
                            env.insert(name, other);
                            return Ok(ControlFlow::Next(Value::Unit));
                        }
                    }
                }
                let val = self.eval_expr(init, env)?;
                env.insert(name, val);
                Ok(ControlFlow::Next(Value::Unit))
            }
            Stmt::Assign { target, value, span } => {
                let val = self.eval_expr(value, env)?;
                if let Expr::Ident(name, _) = target {
                    if !env.assign(name, val) {
                        return Err(Diagnostic::error(format!("Variabel '{}' belum dideklarasikan", name))
                            .with_span(*span));
                    }
                } else if let Expr::FieldAccess { receiver, field, .. } = target {
                    if let Expr::Ident(rec_name, _) = &**receiver {
                        if let Some(Value::Struct { name, mut fields }) = env.get(rec_name) {
                            fields.insert(field.clone(), val);
                            env.assign(rec_name, Value::Struct { name, fields });
                        }
                    }
                }
                Ok(ControlFlow::Next(Value::Unit))
            }
            Stmt::If { cond, then_branch, else_branch, .. } => {
                let cond_val = self.eval_expr(cond, env)?;
                let is_true = match cond_val {
                    Value::Bool(b) => b,
                    _ => false,
                };
                if is_true {
                    self.eval_block(then_branch, env)
                } else if let Some(eb) = else_branch {
                    self.eval_block(eb, env)
                } else {
                    Ok(ControlFlow::Next(Value::Unit))
                }
            }
            Stmt::While { cond, body, .. } => {
                while match self.eval_expr(cond, env)? {
                    Value::Bool(b) => b,
                    _ => false,
                } {
                    match self.eval_block(body, env)? {
                        ControlFlow::Next(_) => {}
                        ret @ ControlFlow::Return(_) => return Ok(ret),
                        fail @ ControlFlow::Fail(_) => return Ok(fail),
                    }
                }
                Ok(ControlFlow::Next(Value::Unit))
            }
            Stmt::ForIn { var_name, iterable, body, span } => {
                let iter_val = self.eval_expr(iterable, env)?;
                match iter_val {
                    Value::List(items_arc) => {
                        let items = items_arc.lock().unwrap().clone();
                        for item in items {
                            env.enter_scope();
                            env.insert(var_name, item);
                            let flow = self.eval_block(body, env)?;
                            env.exit_scope();
                            match flow {
                                ControlFlow::Next(_) => {}
                                ret @ ControlFlow::Return(_) => return Ok(ret),
                                fail @ ControlFlow::Fail(_) => return Ok(fail),
                            }
                        }
                        Ok(ControlFlow::Next(Value::Unit))
                    }
                    _ => Err(Diagnostic::error("Objek bukan koleksi yang dapat diiterasi").with_span(*span)),
                }
            }
            Stmt::Return { value, .. } => {
                let val = if let Some(v) = value {
                    self.eval_expr(v, env)?
                } else {
                    Value::Unit
                };
                Ok(ControlFlow::Return(val))
            }
            Stmt::Fail { value, .. } => {
                let val = self.eval_expr(value, env)?;
                Ok(ControlFlow::Fail(val))
            }
            Stmt::Expr(e) => {
                let val = self.eval_expr(e, env)?;
                Ok(ControlFlow::Next(val))
            }
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr, env: &mut Environment) -> Result<Value> {
        match expr {
            Expr::Lit(lit, _) => Ok(match lit {
                Literal::Int(n) => Value::Int(*n),
                Literal::Float(f) => Value::Float(*f),
                Literal::String(s) => Value::String(s.clone()),
                Literal::Bool(b) => Value::Bool(*b),
                Literal::Unit => Value::Unit,
            }),
            Expr::Ident(name, span) => {
                if let Some(v) = env.get(name) {
                    Ok(v)
                } else {
                    Err(Diagnostic::error(format!("Variabel '{}' tidak terdefinisi", name))
                        .with_span(*span))
                }
            }
            Expr::SelfVar(span) => {
                if let Some(v) = env.get("ini") {
                    Ok(v)
                } else {
                    Err(Diagnostic::error("'ini' tidak terdefinisi di luar metode").with_span(*span))
                }
            }
            Expr::InterpolatedString(parts, _) => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        InterpolatedExprPart::Lit(s) => out.push_str(s),
                        InterpolatedExprPart::Expr(e) => {
                            let val = self.eval_expr(e, env)?;
                            out.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::String(out))
            }
            Expr::Binary(left, op, right, span) => {
                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                match (l, r) {
                    (Value::Int(a), Value::Int(b)) => match op {
                        BinaryOp::Add => Ok(Value::Int(a + b)),
                        BinaryOp::Sub => Ok(Value::Int(a.saturating_sub(b))),
                        BinaryOp::Mul => Ok(Value::Int(a * b)),
                        BinaryOp::Div => {
                            if b == 0 {
                                Err(Diagnostic::error("Pembagian dengan nol").with_span(*span))
                            } else {
                                Ok(Value::Int(a / b))
                            }
                        }
                        BinaryOp::Mod => Ok(Value::Int(a % b)),
                        BinaryOp::Eq => Ok(Value::Bool(a == b)),
                        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
                        BinaryOp::Lt => Ok(Value::Bool(a < b)),
                        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
                        BinaryOp::Gt => Ok(Value::Bool(a > b)),
                        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
                        _ => Err(Diagnostic::error("Operator tidak valid").with_span(*span)),
                    },
                    (Value::Float(a), Value::Float(b)) => match op {
                        BinaryOp::Add => Ok(Value::Float(a + b)),
                        BinaryOp::Sub => Ok(Value::Float(a - b)),
                        BinaryOp::Mul => Ok(Value::Float(a * b)),
                        BinaryOp::Div => Ok(Value::Float(a / b)),
                        BinaryOp::Eq => Ok(Value::Bool((a - b).abs() < f64::EPSILON)),
                        BinaryOp::NotEq => Ok(Value::Bool((a - b).abs() >= f64::EPSILON)),
                        BinaryOp::Lt => Ok(Value::Bool(a < b)),
                        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
                        BinaryOp::Gt => Ok(Value::Bool(a > b)),
                        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
                        _ => Err(Diagnostic::error("Operator tidak valid").with_span(*span)),
                    },
                    (Value::Int(a), Value::Float(b)) => match op {
                        BinaryOp::Sub => Ok(Value::Float(a as f64 - b)),
                        BinaryOp::Add => Ok(Value::Float(a as f64 + b)),
                        BinaryOp::Mul => Ok(Value::Float(a as f64 * b)),
                        BinaryOp::Div => Ok(Value::Float(a as f64 / b)),
                        _ => Err(Diagnostic::error("Operator tidak valid").with_span(*span)),
                    },
                    (Value::Float(a), Value::Int(b)) => match op {
                        BinaryOp::Sub => Ok(Value::Float(a - b as f64)),
                        BinaryOp::Add => Ok(Value::Float(a + b as f64)),
                        BinaryOp::Mul => Ok(Value::Float(a * b as f64)),
                        BinaryOp::Div => Ok(Value::Float(a / b as f64)),
                        _ => Err(Diagnostic::error("Operator tidak valid").with_span(*span)),
                    },
                    (Value::String(a), Value::String(b)) => match op {
                        BinaryOp::Add => Ok(Value::String(format!("{}{}", a, b))),
                        BinaryOp::Eq => Ok(Value::Bool(a == b)),
                        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
                        _ => Err(Diagnostic::error("Operator tidak valid untuk Teks").with_span(*span)),
                    },
                    (Value::Bool(a), Value::Bool(b)) => match op {
                        BinaryOp::And => Ok(Value::Bool(a && b)),
                        BinaryOp::Or => Ok(Value::Bool(a || b)),
                        BinaryOp::Eq => Ok(Value::Bool(a == b)),
                        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
                        _ => Err(Diagnostic::error("Operator tidak valid untuk Bool").with_span(*span)),
                    },
                    _ => Err(Diagnostic::error("Ketidakcocokan tipe pada operator biner").with_span(*span)),
                }
            }
            Expr::Unary(op, operand, span) => {
                let v = self.eval_expr(operand, env)?;
                match (op, v) {
                    (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(n.wrapping_neg())),
                    (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
                    (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    _ => Err(Diagnostic::error("Operasi unary tidak valid").with_span(*span)),
                }
            }
            Expr::Call { callee, args, span } => {
                // 1. Built-in: cetak
                if let Expr::Ident(name, _) = &**callee {
                    if name == "cetak" {
                        let mut msg = String::new();
                        for (i, a) in args.iter().enumerate() {
                            if i > 0 {
                                msg.push(' ');
                            }
                            let val = self.eval_expr(&a.value, env)?;
                            msg.push_str(&val.to_string());
                        }
                        println!("{}", msg);
                        self.output.lock().unwrap().push(msg);
                        return Ok(Value::Unit);
                    }
                }

                // 2. Built-in: tunggu
                if let Expr::Ident(name, _) = &**callee {
                    if name == "tunggu" {
                        if args.is_empty() {
                            return Err(Diagnostic::error("'tunggu' membutuhkan argumen tugas").with_span(*span));
                        }
                        let task_val = self.eval_expr(&args[0].value, env)?;
                        return self.await_task(task_val, *span);
                    }
                }

                // 3. Built-in Stdlib: File I/O
                if let Expr::Ident(name, _) = &**callee {
                    if name == "baca_berkas" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'baca_berkas' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::fs::read_to_string(&path) {
                            Ok(content) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(content))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e.to_string()))))),
                        };
                    }
                    if name == "tulis_berkas" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'tulis_berkas' membutuhkan 2 argumen (jalur, isi)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let c_val = self.eval_expr(&args[1].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let content = match c_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::fs::write(&path, content) {
                            Ok(_) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(true))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e.to_string()))))),
                        };
                    }
                    if name == "ada_berkas" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ada_berkas' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let exists = std::path::Path::new(&path).exists();
                        return Ok(Value::Bool(exists));
                    }
                }

                // 4. Built-in Stdlib: String & Math Utilities
                if let Expr::Ident(name, _) = &**callee {
                    if name == "panjang_teks" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'panjang_teks' membutuhkan 1 argumen").with_span(*span));
                        }
                        let s_val = self.eval_expr(&args[0].value, env)?;
                        let len = match s_val {
                            Value::String(s) => s.chars().count() as u64,
                            other => other.to_string().chars().count() as u64,
                        };
                        return Ok(Value::Int(len));
                    }
                    if name == "ke_huruf_besar" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ke_huruf_besar' membutuhkan 1 argumen").with_span(*span));
                        }
                        let s_val = self.eval_expr(&args[0].value, env)?;
                        let s = match s_val {
                            Value::String(s) => s.to_uppercase(),
                            other => other.to_string().to_uppercase(),
                        };
                        return Ok(Value::String(s));
                    }
                    if name == "ke_huruf_kecil" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ke_huruf_kecil' membutuhkan 1 argumen").with_span(*span));
                        }
                        let s_val = self.eval_expr(&args[0].value, env)?;
                        let s = match s_val {
                            Value::String(s) => s.to_lowercase(),
                            other => other.to_string().to_lowercase(),
                        };
                        return Ok(Value::String(s));
                    }
                    if name == "akar_kuadrat" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'akar_kuadrat' membutuhkan 1 argumen").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let num = match val {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Argumen harus bertipe angka/desimal").with_span(*span)),
                        };
                        return Ok(Value::Float(num.sqrt()));
                    }
                    if name == "pangkat" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'pangkat' membutuhkan 2 argumen (dasar, eksponen)").with_span(*span));
                        }
                        let b_val = self.eval_expr(&args[0].value, env)?;
                        let e_val = self.eval_expr(&args[1].value, env)?;
                        let base = match b_val {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Argumen harus bertipe angka/desimal").with_span(*span)),
                        };
                        let exp = match e_val {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Argumen harus bertipe angka/desimal").with_span(*span)),
                        };
                        return Ok(Value::Float(base.powf(exp)));
                    }
                    if name == "nilai_mutlak" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'nilai_mutlak' membutuhkan 1 argumen").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let num = match val {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Argumen harus bertipe angka/desimal").with_span(*span)),
                        };
                        return Ok(Value::Float(num.abs()));
                    }
                    if name == "waktu_sekarang_ms" {
                        let ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);
                        return Ok(Value::Int(ms));
                    }
                    if name == "ke_json" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ke_json' membutuhkan 1 argumen").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        return Ok(Value::String(value_to_json(&val)));
                    }
                    if name == "uraikan_json" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'uraikan_json' membutuhkan 1 argumen (teks_json)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let raw = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match parse_json(&raw) {
                            Ok(jv) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(jv.to_json_string()))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "ambil_kunci_json" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ambil_kunci_json' membutuhkan 2 argumen (teks_json, kunci)").with_span(*span));
                        }
                        let j_val = self.eval_expr(&args[0].value, env)?;
                        let k_val = self.eval_expr(&args[1].value, env)?;
                        let json_str = match j_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let key = match k_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match parse_json(&json_str) {
                            Ok(jv) => match jv.get(&key) {
                                Some(JsonValue::String(s)) => {
                                    Ok(Value::ErrorUnion(Ok(Box::new(Value::String(s.clone())))))
                                }
                                Some(other) => {
                                    Ok(Value::ErrorUnion(Ok(Box::new(Value::String(other.to_json_string())))))
                                }
                                None => Ok(Value::ErrorUnion(Err(Box::new(Value::String(format!(
                                    "Kunci '{}' tidak ditemukan pada JSON",
                                    key
                                )))))),
                            },
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "ambil_angka_json" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ambil_angka_json' membutuhkan 2 argumen (teks_json, kunci)").with_span(*span));
                        }
                        let j_val = self.eval_expr(&args[0].value, env)?;
                        let k_val = self.eval_expr(&args[1].value, env)?;
                        let json_str = match j_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let key = match k_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match parse_json(&json_str) {
                            Ok(jv) => match jv.get(&key) {
                                Some(JsonValue::Number(n)) => {
                                    Ok(Value::ErrorUnion(Ok(Box::new(Value::Float(*n)))))
                                }
                                Some(other) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(format!(
                                    "Nilai untuk kunci '{}' bukan angka: {}",
                                    key, other
                                )))))),
                                None => Ok(Value::ErrorUnion(Err(Box::new(Value::String(format!(
                                    "Kunci '{}' tidak ditemukan pada JSON",
                                    key
                                )))))),
                            },
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "ambil_bool_json" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ambil_bool_json' membutuhkan 2 argumen (teks_json, kunci)").with_span(*span));
                        }
                        let j_val = self.eval_expr(&args[0].value, env)?;
                        let k_val = self.eval_expr(&args[1].value, env)?;
                        let json_str = match j_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let key = match k_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match parse_json(&json_str) {
                            Ok(jv) => match jv.get(&key) {
                                Some(JsonValue::Bool(b)) => {
                                    Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(*b)))))
                                }
                                Some(other) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(format!(
                                    "Nilai untuk kunci '{}' bukan boolean: {}",
                                    key, other
                                )))))),
                                None => Ok(Value::ErrorUnion(Err(Box::new(Value::String(format!(
                                    "Kunci '{}' tidak ditemukan pada JSON",
                                    key
                                )))))),
                            },
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "ada_kunci_json" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ada_kunci_json' membutuhkan 2 argumen (teks_json, kunci)").with_span(*span));
                        }
                        let j_val = self.eval_expr(&args[0].value, env)?;
                        let k_val = self.eval_expr(&args[1].value, env)?;
                        let json_str = match j_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let key = match k_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match parse_json(&json_str) {
                            Ok(jv) => Ok(Value::Bool(jv.has_key(&key))),
                            Err(_) => Ok(Value::Bool(false)),
                        };
                    }
                    if name == "valid_json" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'valid_json' membutuhkan 1 argumen").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let raw = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return Ok(Value::Bool(parse_json(&raw).is_ok()));
                    }

                    // HTTP Network Utilities
                    if name == "ambil_http" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ambil_http' membutuhkan 1 argumen (url)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let url = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match crate::net::ambil_http(&url) {
                            Ok(body) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(body))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "kirim_http_post" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'kirim_http_post' membutuhkan 2 argumen (url, data_json)").with_span(*span));
                        }
                        let u_val = self.eval_expr(&args[0].value, env)?;
                        let d_val = self.eval_expr(&args[1].value, env)?;
                        let url = match u_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let body = match d_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match crate::net::kirim_http_post(&url, &body) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "ambil_status_http" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ambil_status_http' membutuhkan 1 argumen (url)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let url = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match crate::net::ambil_status_http(&url) {
                            Ok(code) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Int(code))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                }

                // 5. Daftar Constructor: Daftar(...)
                if let Expr::Ident(name, _) = &**callee {
                    if name == "Daftar" {
                        let mut items = Vec::new();
                        for a in args {
                            items.push(self.eval_expr(&a.value, env)?);
                        }
                        return Ok(Value::List(Arc::new(Mutex::new(items))));
                    }
                }

                // 6. Struct Constructor: Name(field: val, ...)
                if let Expr::Ident(name, _) = &**callee {
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        let mut fields = HashMap::new();
                        for (idx, arg) in args.iter().enumerate() {
                            let f_name = arg.name.clone().unwrap_or_else(|| format!("field_{}", idx));
                            let f_val = self.eval_expr(&arg.value, env)?;
                            fields.insert(f_name, f_val);
                        }
                        return Ok(Value::Struct {
                            name: name.clone(),
                            fields,
                        });
                    }
                }

                // 4. User function call
                if let Expr::Ident(name, _) = &**callee {
                    if let Some(target_fn) = self.functions.get(name).cloned() {
                        let mut evaluated_args = Vec::new();
                        for a in args {
                            evaluated_args.push(self.eval_expr(&a.value, env)?);
                        }
                        match self.eval_fn(&target_fn, evaluated_args, env)? {
                            ControlFlow::Next(v) | ControlFlow::Return(v) => {
                                // If function returns ErrorUnion, wrap in Value::ErrorUnion
                                if matches!(target_fn.ret_ty, Some(TypeExpr::ErrorUnion(_, _, _))) {
                                    match v {
                                        Value::ErrorUnion(_) => Ok(v),
                                        other => Ok(Value::ErrorUnion(Ok(Box::new(other)))),
                                    }
                                } else {
                                    Ok(v)
                                }
                            }
                            ControlFlow::Fail(e) => Ok(Value::ErrorUnion(Err(Box::new(e)))),
                        }
                    } else {
                        Err(Diagnostic::error(format!("Fungsi '{}' tidak ditemukan", name)).with_span(*span))
                    }
                } else {
                    Err(Diagnostic::error("Bukan fungsi").with_span(*span))
                }
            }
            Expr::MethodCall { receiver, method, args, span } => {
                let rec_val = self.eval_expr(receiver, env)?;

                // Check methods on Daftar
                if let Value::List(items_arc) = &rec_val {
                    match method.as_str() {
                        "tambah" => {
                            if args.len() != 1 {
                                return Err(Diagnostic::error("Metode 'tambah' membutuhkan 1 argumen").with_span(*span));
                            }
                            let val = self.eval_expr(&args[0].value, env)?;
                            items_arc.lock().unwrap().push(val);
                            return Ok(Value::Unit);
                        }
                        "panjang" => {
                            let len = items_arc.lock().unwrap().len() as u64;
                            return Ok(Value::Int(len));
                        }
                        "ambil" => {
                            if args.len() != 1 {
                                return Err(Diagnostic::error("Metode 'ambil' membutuhkan 1 argumen (indeks)").with_span(*span));
                            }
                            let idx_val = self.eval_expr(&args[0].value, env)?;
                            let idx = match idx_val {
                                Value::Int(i) => i as usize,
                                _ => return Err(Diagnostic::error("Indeks harus bertipe integer").with_span(*span)),
                            };
                            let items = items_arc.lock().unwrap();
                            if idx < items.len() {
                                return Ok(Value::ErrorUnion(Ok(Box::new(items[idx].clone()))));
                            } else {
                                return Ok(Value::ErrorUnion(Err(Box::new(Value::String(
                                    format!("Indeks di luar batas: {} (panjang {})", idx, items.len())
                                )))));
                            }
                        }
                        other => {
                            return Err(Diagnostic::error(format!(
                                "Metode '{}' tidak ditemukan pada Daftar", other
                            )).with_span(*span));
                        }
                    }
                }

                if let Value::Struct { name: struct_name, .. } = &rec_val {
                    if let Some(struct_methods) = self.methods.get(struct_name) {
                        if let Some(target_method) = struct_methods.get(method).cloned() {
                            let mut all_args = vec![rec_val];
                            for a in args {
                                all_args.push(self.eval_expr(&a.value, env)?);
                            }
                            return match self.eval_fn(&target_method, all_args, env)? {
                                ControlFlow::Next(v) | ControlFlow::Return(v) => Ok(v),
                                ControlFlow::Fail(e) => Ok(Value::ErrorUnion(Err(Box::new(e)))),
                            };
                        }
                    }
                    Err(Diagnostic::error(format!(
                        "Metode '{}' tidak ditemukan pada struktur '{}'",
                        method, struct_name
                    ))
                    .with_span(*span))
                } else {
                    Err(Diagnostic::error(format!(
                        "Tidak dapat memanggil metode '{}' pada tipe non-struktur",
                        method
                    ))
                    .with_span(*span))
                }
            }
            Expr::FieldAccess { receiver, field, span } => {
                let rec_val = self.eval_expr(receiver, env)?;
                if let Value::Struct { fields, .. } = rec_val {
                    if let Some(fv) = fields.get(field) {
                        Ok(fv.clone())
                    } else {
                        Err(Diagnostic::error(format!("Field '{}' tidak ditemukan", field)).with_span(*span))
                    }
                } else {
                    Err(Diagnostic::error("Akses field pada tipe non-struktur").with_span(*span))
                }
            }
            Expr::ErrorPropagate { inner, span } => {
                let inner_val = self.eval_expr(inner, env)?;
                match inner_val {
                    Value::ErrorUnion(Ok(ok_val)) => Ok(*ok_val),
                    Value::ErrorUnion(Err(err_val)) => {
                        // Return early with error
                        Err(Diagnostic::error(format!("Kesalahan tidak ditangani: {}", err_val))
                            .with_span(*span))
                    }
                    other => Ok(other),
                }
            }
            Expr::Catch { inner, err_var, handler, .. } => {
                let inner_val = self.eval_expr(inner, env)?;
                match inner_val {
                    Value::ErrorUnion(Err(err_val)) => {
                        env.enter_scope();
                        env.insert(err_var, *err_val);
                        let res = self.eval_block(handler, env)?;
                        env.exit_scope();
                        match res {
                            ControlFlow::Next(v) => Ok(v),
                            ControlFlow::Return(v) => Ok(v),
                            ControlFlow::Fail(e) => Ok(Value::ErrorUnion(Err(Box::new(e)))),
                        }
                    }
                    Value::ErrorUnion(Ok(ok_val)) => Ok(*ok_val),
                    other => Ok(other),
                }
            }
            Expr::Parallel { body, .. } => {
                // Spawn colorless stackless coroutine task using thread
                let body_clone = body.clone();
                let mut captured_env = env.clone();
                let mut sub_eval = Evaluator {
                    output: Arc::clone(&self.output),
                    functions: self.functions.clone(),
                    methods: self.methods.clone(),
                };

                let handle = thread::spawn(move || {
                    match sub_eval.eval_block(&body_clone, &mut captured_env) {
                        Ok(ControlFlow::Next(v) | ControlFlow::Return(v)) => v,
                        Ok(ControlFlow::Fail(e)) => Value::ErrorUnion(Err(Box::new(e))),
                        Err(_) => Value::Unit,
                    }
                });

                Ok(Value::Task(Arc::new(Mutex::new(Some(handle)))))
            }
            Expr::Await { task, span } => {
                let task_val = self.eval_expr(task, env)?;
                self.await_task(task_val, *span)
            }
            Expr::Comptime { body, .. } => {
                // Comptime evaluated directly
                match self.eval_block(body, env)? {
                    ControlFlow::Next(v) | ControlFlow::Return(v) => Ok(v),
                    ControlFlow::Fail(e) => Ok(Value::ErrorUnion(Err(Box::new(e)))),
                }
            }
            Expr::Block(b) => match self.eval_block(b, env)? {
                ControlFlow::Next(v) | ControlFlow::Return(v) => Ok(v),
                ControlFlow::Fail(e) => Ok(Value::ErrorUnion(Err(Box::new(e)))),
            },
        }
    }

    fn await_task(&self, task_val: Value, span: Span) -> Result<Value> {
        if let Value::Task(lock) = task_val {
            let mut guard = lock.lock().unwrap();
            if let Some(handle) = guard.take() {
                match handle.join() {
                    Ok(val) => Ok(val),
                    Err(_) => Err(Diagnostic::error("Tugas paralel panik saat eksekusi").with_span(span)),
                }
            } else {
                Ok(Value::Unit)
            }
        } else {
            Err(Diagnostic::error("Bukan objek tugas").with_span(span))
        }
    }
}
