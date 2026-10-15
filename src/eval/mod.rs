use crate::ast::*;
use crate::diag::{Diagnostic, Result};
use crate::json::{parse_json, value_to_json, JsonValue};
use crate::span::Span;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Mengonversi notasi nada musik standar (contoh: "C4", "A4", "F#4") ke frekuensi Hertz (Hz).
pub fn note_to_frequency(note: &str) -> f64 {
    let s = note.trim();
    if s.is_empty() {
        return 440.0;
    }
    let bytes = s.as_bytes();
    let letter = bytes[0].to_ascii_uppercase();
    let base = match letter {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return 440.0,
    };

    let mut idx = 1;
    let mut accidental = 0i32;
    if idx < bytes.len() {
        if bytes[idx] == b'#' || bytes[idx] == b'+' {
            accidental = 1;
            idx += 1;
        } else if bytes[idx] == b'b' || bytes[idx] == b'-' {
            accidental = -1;
            idx += 1;
        }
    }

    let octave: i32 = if idx < bytes.len() {
        s[idx..].parse::<i32>().unwrap_or(4)
    } else {
        4
    };

    let midi_note = (octave + 1) * 12 + base + accidental;
    440.0 * (2.0f64).powf((midi_note - 69) as f64 / 12.0)
}

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

impl Value {
    pub fn is_equal(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::Struct { name: n1, fields: f1 }, Value::Struct { name: n2, fields: f2 }) => {
                if n1 != n2 || f1.len() != f2.len() {
                    return false;
                }
                for (k, v1) in f1 {
                    if let Some(v2) = f2.get(k) {
                        if !v1.is_equal(v2) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (Value::List(l1), Value::List(l2)) => {
                let lock1 = l1.lock().unwrap();
                let lock2 = l2.lock().unwrap();
                if lock1.len() != lock2.len() {
                    return false;
                }
                for i in 0..lock1.len() {
                    if !lock1[i].is_equal(&lock2[i]) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            Value::Int(_) => "u64".to_string(),
            Value::Float(_) => "f64".to_string(),
            Value::String(_) => "Teks".to_string(),
            Value::Bool(_) => "Bool".to_string(),
            Value::Unit => "Unit".to_string(),
            Value::Struct { name, .. } => name.clone(),
            Value::ErrorUnion(_) => "ErrorUnion".to_string(),
            Value::Task(_) => "Tugas".to_string(),
            Value::List(_) => "Daftar".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    pub duration: std::time::Duration,
    pub span: Span,
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

    pub fn get_all_variables(&self) -> Vec<(String, Value)> {
        let mut vars = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                vars.insert(k.clone(), v.clone());
            }
        }
        let mut list: Vec<(String, Value)> = vars.into_iter().collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugAction {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Pause,
    Terminate,
}

pub trait DebugHook: Send + Sync {
    fn on_statement(
        &mut self,
        span: Span,
        env: &Environment,
        fn_name: &str,
        call_stack: &[(String, Span)],
    ) -> DebugAction;

    fn on_output(&mut self, text: &str);
}

pub struct Evaluator {
    pub output: Arc<Mutex<Vec<String>>>,
    functions: HashMap<String, FnDecl>,
    // struct_name -> method_name -> FnDecl
    methods: HashMap<String, HashMap<String, FnDecl>>,
    cli_args: Vec<String>,
    pub call_stack: Vec<(String, Span)>,
    pub debug_hook: Option<Arc<Mutex<dyn DebugHook>>>,
}

impl Evaluator {
    pub fn new() -> Self {
        Evaluator {
            output: Arc::new(Mutex::new(Vec::new())),
            functions: HashMap::new(),
            methods: HashMap::new(),
            cli_args: std::env::args().collect(),
            call_stack: Vec::new(),
            debug_hook: None,
        }
    }

    pub fn with_cli_args(args: Vec<String>) -> Self {
        Evaluator {
            output: Arc::new(Mutex::new(Vec::new())),
            functions: HashMap::new(),
            methods: HashMap::new(),
            cli_args: args,
            call_stack: Vec::new(),
            debug_hook: None,
        }
    }

    pub fn set_debug_hook(&mut self, hook: Arc<Mutex<dyn DebugHook>>) {
        self.debug_hook = Some(hook);
    }

    pub fn set_cli_args(&mut self, args: Vec<String>) {
        self.cli_args = args;
    }

    pub fn get_output(&self) -> Vec<String> {
        self.output.lock().unwrap().clone()
    }

    pub fn register_decl(&mut self, decl: &Decl) {
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

    pub fn get_defined_functions(&self) -> Vec<String> {
        let mut list: Vec<String> = self.functions.keys().cloned().collect();
        list.sort();
        list
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

    pub fn run_tests(&mut self, program: &Program) -> Vec<TestResult> {
        // Collect functions and impl methods first so tests can call them
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

        let mut results = Vec::new();

        for decl in &program.decls {
            if let Decl::Test(t) = decl {
                let start = std::time::Instant::now();
                let mut env = Environment::new();
                let res = self.eval_block(&t.body, &mut env);
                let duration = start.elapsed();

                match res {
                    Ok(ControlFlow::Next(_)) | Ok(ControlFlow::Return(_)) => {
                        results.push(TestResult {
                            name: t.name.clone(),
                            passed: true,
                            error: None,
                            duration,
                            span: t.span,
                        });
                    }
                    Ok(ControlFlow::Fail(e)) => {
                        results.push(TestResult {
                            name: t.name.clone(),
                            passed: false,
                            error: Some(e.to_string()),
                            duration,
                            span: t.span,
                        });
                    }
                    Err(diag) => {
                        results.push(TestResult {
                            name: t.name.clone(),
                            passed: false,
                            error: Some(diag.message),
                            duration,
                            span: t.span,
                        });
                    }
                }
            }
        }

        results
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

        self.call_stack.push((f.name.clone(), f.span));
        let res = self.eval_block(&f.body, &mut local_env);
        self.call_stack.pop();
        res
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
        if let Some(ref hook) = self.debug_hook {
            let fn_name = self
                .call_stack
                .last()
                .map(|(n, _)| n.as_str())
                .unwrap_or("utama");
            let action = hook
                .lock()
                .unwrap()
                .on_statement(stmt.span(), env, fn_name, &self.call_stack);
            if action == DebugAction::Terminate {
                return Err(Diagnostic::error("Eksekusi dihentikan oleh debugger").with_span(stmt.span()));
            }
        }

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
                    match name.as_str() {
                        "PI" => Ok(Value::Float(std::f64::consts::PI)),
                        "TAU" => Ok(Value::Float(std::f64::consts::TAU)),
                        "E" => Ok(Value::Float(std::f64::consts::E)),
                        _ => Err(Diagnostic::error(format!("Variabel '{}' tidak terdefinisi", name))
                            .with_span(*span)),
                    }
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
                self.eval_call(callee, args, *span, env)
            }
            Expr::MethodCall { receiver, method, args, span } => {
                self.eval_method_call(receiver, method, args, *span, env)
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
                    cli_args: self.cli_args.clone(),
                    call_stack: Vec::new(),
                    debug_hook: self.debug_hook.clone(),
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

    #[inline(never)]
    pub fn eval_call(
        &mut self,
        callee: &Box<Expr>,
        args: &[CallArg],
        span: Span,
        env: &mut Environment,
    ) -> Result<Value> {
        let span = &span;

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
                        self.output.lock().unwrap().push(msg.clone());
                        if let Some(ref hook) = self.debug_hook {
                            hook.lock().unwrap().on_output(&format!("{}\n", msg));
                        }
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

                // Built-in Stdlib: Test Assertions
                if let Expr::Ident(name, _) = &**callee {
                    if name == "pastikan" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'pastikan' membutuhkan 1 argumen (kondisi: bool)").with_span(*span));
                        }
                        let cond_val = self.eval_expr(&args[0].value, env)?;
                        match cond_val {
                            Value::Bool(true) => return Ok(Value::Unit),
                            Value::Bool(false) => {
                                return Err(Diagnostic::error("Asersi gagal: kondisi bernilai 'salah'").with_span(*span));
                            }
                            other => {
                                return Err(Diagnostic::error(format!("Asersi gagal: kondisi bukan boolean ({:?})", other)).with_span(*span));
                            }
                        }
                    }
                    if name == "pastikan_sama" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'pastikan_sama' membutuhkan 2 argumen (sebenarnya, diharapkan)").with_span(*span));
                        }
                        let a = self.eval_expr(&args[0].value, env)?;
                        let b = self.eval_expr(&args[1].value, env)?;
                        if a.is_equal(&b) {
                            return Ok(Value::Unit);
                        } else {
                            return Err(Diagnostic::error(format!(
                                "Asersi gagal: nilai '{:?}' tidak sama dengan '{:?}'",
                                a, b
                            )).with_span(*span));
                        }
                    }
                    if name == "pastikan_tidak_sama" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'pastikan_tidak_sama' membutuhkan 2 argumen (a, b)").with_span(*span));
                        }
                        let a = self.eval_expr(&args[0].value, env)?;
                        let b = self.eval_expr(&args[1].value, env)?;
                        if !a.is_equal(&b) {
                            return Ok(Value::Unit);
                        } else {
                            return Err(Diagnostic::error(format!(
                                "Asersi gagal: kedua nilai bernilai sama ({:?})",
                                a
                            )).with_span(*span));
                        }
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
                    if name == "buat_direktori" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'buat_direktori' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::fs::create_dir_all(&path) {
                            Ok(_) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(true))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e.to_string()))))),
                        };
                    }
                    if name == "daftar_direktori" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'daftar_direktori' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::fs::read_dir(&path) {
                            Ok(entries) => {
                                let mut list = Vec::new();
                                for entry in entries.flatten() {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    list.push(Value::String(name));
                                }
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::List(Arc::new(Mutex::new(list)))))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e.to_string()))))),
                        };
                    }
                    if name == "hapus_berkas" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'hapus_berkas' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::fs::remove_file(&path) {
                            Ok(_) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(true))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e.to_string()))))),
                        };
                    }
                    if name == "hapus_direktori" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'hapus_direktori' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::fs::remove_dir_all(&path) {
                            Ok(_) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(true))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e.to_string()))))),
                        };
                    }
                    if name == "gabung_jalur" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'gabung_jalur' membutuhkan 2 argumen (induk, anak)").with_span(*span));
                        }
                        let a_val = self.eval_expr(&args[0].value, env)?;
                        let b_val = self.eval_expr(&args[1].value, env)?;
                        let a = match a_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let b = match b_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let joined = std::path::Path::new(&a).join(&b);
                        let norm = joined.to_string_lossy().replace('\\', "/");
                        return Ok(Value::String(norm));
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
                    // Standard Library: Scientific Math & Trigonometry
                    match name.as_str() {
                        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "akar_kuadrat" | "akar_kubik"
                        | "nilai_mutlak" | "bulat" | "lantai" | "langit_langit" | "tanda"
                        | "log" | "log10" | "log2" | "exp" | "ke_radian" | "ke_derajat" => {
                            if args.len() != 1 {
                                return Err(Diagnostic::error(format!("'{}' membutuhkan tepat 1 argumen angka", name)).with_span(*span));
                            }
                            let val = self.eval_expr(&args[0].value, env)?;
                            let num = match val {
                                Value::Float(f) => f,
                                Value::Int(i) => i as f64,
                                _ => return Err(Diagnostic::error("Argumen harus bertipe angka/desimal").with_span(*span)),
                            };
                            let res = match name.as_str() {
                                "sin" => num.sin(),
                                "cos" => num.cos(),
                                "tan" => num.tan(),
                                "asin" => num.asin(),
                                "acos" => num.acos(),
                                "atan" => num.atan(),
                                "akar_kuadrat" => num.sqrt(),
                                "akar_kubik" => num.cbrt(),
                                "nilai_mutlak" => num.abs(),
                                "bulat" => num.round(),
                                "lantai" => num.floor(),
                                "langit_langit" => num.ceil(),
                                "tanda" => if num == 0.0 { 0.0 } else { num.signum() },
                                "log" => num.ln(),
                                "log10" => num.log10(),
                                "log2" => num.log2(),
                                "exp" => num.exp(),
                                "ke_radian" => num.to_radians(),
                                "ke_derajat" => num.to_degrees(),
                                _ => unreachable!(),
                            };
                            return Ok(Value::Float(res));
                        }
                        "pangkat" | "atan2" | "min" | "max" => {
                            if args.len() != 2 {
                                return Err(Diagnostic::error(format!("'{}' membutuhkan tepat 2 argumen angka", name)).with_span(*span));
                            }
                            let a_val = self.eval_expr(&args[0].value, env)?;
                            let b_val = self.eval_expr(&args[1].value, env)?;
                            let a = match a_val {
                                Value::Float(f) => f,
                                Value::Int(i) => i as f64,
                                _ => return Err(Diagnostic::error("Argumen pertama harus bertipe angka/desimal").with_span(*span)),
                            };
                            let b = match b_val {
                                Value::Float(f) => f,
                                Value::Int(i) => i as f64,
                                _ => return Err(Diagnostic::error("Argumen kedua harus bertipe angka/desimal").with_span(*span)),
                            };
                            let res = match name.as_str() {
                                "pangkat" => a.powf(b),
                                "atan2" => a.atan2(b),
                                "min" => a.min(b),
                                "max" => a.max(b),
                                _ => unreachable!(),
                            };
                            return Ok(Value::Float(res));
                        }
                        _ => {}
                    }
                    if name == "waktu_sekarang_ms" {
                        let ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);
                        return Ok(Value::Int(ms));
                    }
                    if name == "tidur" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'tidur' membutuhkan 1 argumen (milidetik: u64)").with_span(*span));
                        }
                        let ms_val = self.eval_expr(&args[0].value, env)?;
                        let ms = match ms_val {
                            Value::Int(n) => n,
                            Value::Float(f) => f as u64,
                            _ => return Err(Diagnostic::error("Argumen 'tidur' harus berupa angka milidetik").with_span(*span)),
                        };
                        std::thread::sleep(std::time::Duration::from_millis(ms));
                        return Ok(Value::Unit);
                    }
                    if name == "ambil_argumen_cli" {
                        if !args.is_empty() {
                            return Err(Diagnostic::error("'ambil_argumen_cli' tidak membutuhkan argumen").with_span(*span));
                        }
                        let items: Vec<Value> = self.cli_args
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect();
                        return Ok(Value::List(Arc::new(Mutex::new(items))));
                    }
                    if name == "ambil_lingkungan" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ambil_lingkungan' membutuhkan 1 argumen (nama_variabel)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let key = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match std::env::var(&key) {
                            Ok(v) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(v))))),
                            Err(_) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(format!(
                                "Variabel lingkungan '{}' tidak ditemukan",
                                key
                            )))))),
                        };
                    }
                    if name == "atur_lingkungan" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'atur_lingkungan' membutuhkan 2 argumen (nama_variabel, nilai)").with_span(*span));
                        }
                        let k_val = self.eval_expr(&args[0].value, env)?;
                        let v_val = self.eval_expr(&args[1].value, env)?;
                        let key = match k_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let value = match v_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        unsafe {
                            std::env::set_var(&key, &value);
                        }
                        return Ok(Value::Unit);
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

                    // HTTP Web Server Response Primitives
                    if name == "kirim_respons_http" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'kirim_respons_http' membutuhkan 3 argumen (status, tipe_konten, konten)").with_span(*span));
                        }
                        let s_val = self.eval_expr(&args[0].value, env)?;
                        let t_val = self.eval_expr(&args[1].value, env)?;
                        let k_val = self.eval_expr(&args[2].value, env)?;
                        let status = match s_val {
                            Value::Int(n) => n,
                            other => other.to_string().parse().unwrap_or(200),
                        };
                        let mut fields = HashMap::new();
                        fields.insert("status".to_string(), Value::Int(status));
                        fields.insert("tipe_konten".to_string(), Value::String(t_val.to_string()));
                        fields.insert("konten".to_string(), Value::String(k_val.to_string()));
                        fields.insert("header".to_string(), Value::String(String::new()));
                        return Ok(Value::Struct {
                            name: "ResponsHttp".to_string(),
                            fields,
                        });
                    }
                    if name == "respons_json" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_json' membutuhkan 1 argumen (data_json)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let mut fields = HashMap::new();
                        fields.insert("status".to_string(), Value::Int(200));
                        fields.insert("tipe_konten".to_string(), Value::String("application/json".to_string()));
                        fields.insert("konten".to_string(), Value::String(val.to_string()));
                        fields.insert("header".to_string(), Value::String(String::new()));
                        return Ok(Value::Struct {
                            name: "ResponsHttp".to_string(),
                            fields,
                        });
                    }
                    if name == "respons_teks" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_teks' membutuhkan 1 argumen (teks)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let mut fields = HashMap::new();
                        fields.insert("status".to_string(), Value::Int(200));
                        fields.insert("tipe_konten".to_string(), Value::String("text/plain; charset=utf-8".to_string()));
                        fields.insert("konten".to_string(), Value::String(val.to_string()));
                        fields.insert("header".to_string(), Value::String(String::new()));
                        return Ok(Value::Struct {
                            name: "ResponsHttp".to_string(),
                            fields,
                        });
                    }
                    if name == "tambah_header" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'tambah_header' membutuhkan 3 argumen: (resp, kunci, nilai)").with_span(*span));
                        }
                        let resp_val = self.eval_expr(&args[0].value, env)?;
                        let k_val = self.eval_expr(&args[1].value, env)?.to_string();
                        let v_val = self.eval_expr(&args[2].value, env)?.to_string();
                        if let Value::Struct { name: s_name, mut fields } = resp_val {
                            if s_name == "ResponsHttp" {
                                let mut current_h = fields.get("header").map(|v| v.to_string()).unwrap_or_default();
                                current_h.push_str(&format!("{}: {}\r\n", k_val, v_val));
                                fields.insert("header".to_string(), Value::String(current_h));
                                return Ok(Value::Struct { name: s_name, fields });
                            }
                        }
                        return Err(Diagnostic::error("Argumen pertama 'tambah_header' harus berupa ResponsHttp").with_span(*span));
                    }
                    if name == "respons_cors" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_cors' membutuhkan 1 argumen (resp)").with_span(*span));
                        }
                        let resp_val = self.eval_expr(&args[0].value, env)?;
                        if let Value::Struct { name: s_name, mut fields } = resp_val {
                            if s_name == "ResponsHttp" {
                                let mut current_h = fields.get("header").map(|v| v.to_string()).unwrap_or_default();
                                current_h.push_str("Access-Control-Allow-Origin: *\r\n");
                                current_h.push_str("Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n");
                                current_h.push_str("Access-Control-Allow-Headers: Content-Type, Authorization\r\n");
                                fields.insert("header".to_string(), Value::String(current_h));
                                return Ok(Value::Struct { name: s_name, fields });
                            }
                        }
                        return Err(Diagnostic::error("Argumen 'respons_cors' harus berupa ResponsHttp").with_span(*span));
                    }
                    if name == "cocok_rute" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'cocok_rute' membutuhkan 2 argumen: (pola, jalur)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let jalur = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::Bool(crate::net::cocok_rute(&pola, &jalur)));
                    }
                    if name == "ambil_param_rute" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'ambil_param_rute' membutuhkan 3 argumen: (pola, jalur, nama_param)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let jalur = self.eval_expr(&args[1].value, env)?.to_string();
                        let nama = self.eval_expr(&args[2].value, env)?.to_string();
                        return match crate::net::ambil_param_rute(&pola, &jalur, &nama) {
                            Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                            Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                        };
                    }
                    if name == "ambil_kueri_param" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ambil_kueri_param' membutuhkan 2 argumen: (kueri, kunci)").with_span(*span));
                        }
                        let kueri = self.eval_expr(&args[0].value, env)?.to_string();
                        let kunci = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::net::ambil_kueri_param(&kueri, &kunci) {
                            Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                            Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                        };
                    }
                    if name == "ada_kueri_param" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ada_kueri_param' membutuhkan 2 argumen: (kueri, kunci)").with_span(*span));
                        }
                        let kueri = self.eval_expr(&args[0].value, env)?.to_string();
                        let kunci = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::Bool(crate::net::ada_kueri_param(&kueri, &kunci)));
                    }
                    if name == "cocok_awalan" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'cocok_awalan' membutuhkan 2 argumen: (jalur, awalan)").with_span(*span));
                        }
                        let jalur = self.eval_expr(&args[0].value, env)?.to_string();
                        let awalan = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::Bool(crate::net::cocok_awalan(&jalur, &awalan)));
                    }
                    if name == "sisa_jalur" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'sisa_jalur' membutuhkan 2 argumen: (jalur, awalan)").with_span(*span));
                        }
                        let jalur = self.eval_expr(&args[0].value, env)?.to_string();
                        let awalan = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::String(crate::net::sisa_jalur(&jalur, &awalan)));
                    }
                    if name == "kueri_param_atau" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'kueri_param_atau' membutuhkan 3 argumen: (kueri, kunci, bawaan)").with_span(*span));
                        }
                        let kueri = self.eval_expr(&args[0].value, env)?.to_string();
                        let kunci = self.eval_expr(&args[1].value, env)?.to_string();
                        let bawaan = self.eval_expr(&args[2].value, env)?.to_string();
                        return Ok(Value::String(crate::net::kueri_param_atau(&kueri, &kunci, &bawaan)));
                    }
                    if name == "semua_kueri_param" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'semua_kueri_param' membutuhkan 1 argumen: (kueri)").with_span(*span));
                        }
                        let kueri = self.eval_expr(&args[0].value, env)?.to_string();
                        let all = crate::net::semua_kueri_param(&kueri);
                        let v_all: Vec<Value> = all.into_iter().map(Value::String).collect();
                        return Ok(Value::List(Arc::new(Mutex::new(v_all))));
                    }
                    if name == "ambil_header_http" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ambil_header_http' membutuhkan 2 argumen: (raw_headers, nama)").with_span(*span));
                        }
                        let raw = self.eval_expr(&args[0].value, env)?.to_string();
                        let nama = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::net::ambil_header_http(&raw, &nama) {
                            Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                            Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                        };
                    }
                    if name == "ada_header_http" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ada_header_http' membutuhkan 2 argumen: (raw_headers, nama)").with_span(*span));
                        }
                        let raw = self.eval_expr(&args[0].value, env)?.to_string();
                        let nama = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::Bool(crate::net::ada_header_http(&raw, &nama)));
                    }
                    if name == "header_atau" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'header_atau' membutuhkan 3 argumen: (raw_headers, nama, bawaan)").with_span(*span));
                        }
                        let raw = self.eval_expr(&args[0].value, env)?.to_string();
                        let nama = self.eval_expr(&args[1].value, env)?.to_string();
                        let bawaan = self.eval_expr(&args[2].value, env)?.to_string();
                        return Ok(Value::String(crate::net::header_atau(&raw, &nama, &bawaan)));
                    }
                    if name == "ambil_otorisasi_bearer" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ambil_otorisasi_bearer' membutuhkan 1 argumen: (raw_headers)").with_span(*span));
                        }
                        let raw = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::net::ambil_otorisasi_bearer(&raw) {
                            Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                            Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                        };
                    }
                    if name == "respons_401" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_401' membutuhkan 1 argumen (pesan)").with_span(*span));
                        }
                        let pesan = self.eval_expr(&args[0].value, env)?.to_string();
                        let mut fields = HashMap::new();
                        fields.insert("status".to_string(), Value::Int(401));
                        fields.insert("tipe_konten".to_string(), Value::String("application/json".to_string()));
                        fields.insert("konten".to_string(), Value::String(format!("{{\"galat\":\"{}\"}}", pesan)));
                        fields.insert("header".to_string(), Value::String("WWW-Authenticate: Bearer\r\n".to_string()));
                        return Ok(Value::Struct {
                            name: "ResponsHttp".to_string(),
                            fields,
                        });
                    }
                    if name == "respons_403" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_403' membutuhkan 1 argumen (pesan)").with_span(*span));
                        }
                        let pesan = self.eval_expr(&args[0].value, env)?.to_string();
                        let mut fields = HashMap::new();
                        fields.insert("status".to_string(), Value::Int(403));
                        fields.insert("tipe_konten".to_string(), Value::String("application/json".to_string()));
                        fields.insert("konten".to_string(), Value::String(format!("{{\"galat\":\"{}\"}}", pesan)));
                        fields.insert("header".to_string(), Value::String(String::new()));
                        return Ok(Value::Struct {
                            name: "ResponsHttp".to_string(),
                            fields,
                        });
                    }
                    if name == "respons_redirect" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'respons_redirect' membutuhkan 2 argumen: (lokasi, kode)").with_span(*span));
                        }
                        let lokasi = self.eval_expr(&args[0].value, env)?.to_string();
                        let kode = match self.eval_expr(&args[1].value, env)? {
                            Value::Int(i) => i,
                            Value::Float(f) => f as u64,
                            _ => 302,
                        };
                        let mut fields = HashMap::new();
                        fields.insert("status".to_string(), Value::Int(kode));
                        fields.insert("tipe_konten".to_string(), Value::String("text/plain; charset=utf-8".to_string()));
                        fields.insert("konten".to_string(), Value::String("Redirecting...".to_string()));
                        fields.insert("header".to_string(), Value::String(format!("Location: {}\r\n", lokasi)));
                        return Ok(Value::Struct {
                            name: "ResponsHttp".to_string(),
                            fields,
                        });
                    }
                    if name == "respons_berkas" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_berkas' membutuhkan 1 argumen (jalur_berkas)").with_span(*span));
                        }
                        let path_val = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::net::respons_berkas(&path_val) {
                            Ok(http_resp) => {
                                let mut fields = HashMap::new();
                                fields.insert("status".to_string(), Value::Int(http_resp.status));
                                fields.insert("tipe_konten".to_string(), Value::String(http_resp.content_type));
                                fields.insert("konten".to_string(), Value::String(http_resp.body));
                                fields.insert("header".to_string(), Value::String(String::new()));
                                fields.insert("_jalur_berkas".to_string(), Value::String(path_val));
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::Struct {
                                    name: "ResponsHttp".to_string(),
                                    fields,
                                }))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "sajikan_berkas_statis" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'sajikan_berkas_statis' membutuhkan 2 argumen: (folder_dasar, jalur)").with_span(*span));
                        }
                        let folder = self.eval_expr(&args[0].value, env)?.to_string();
                        let path_val = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::net::sajikan_berkas_statis(&folder, &path_val) {
                            Ok(http_resp) => {
                                let mut fields = HashMap::new();
                                fields.insert("status".to_string(), Value::Int(http_resp.status));
                                fields.insert("tipe_konten".to_string(), Value::String(http_resp.content_type));
                                fields.insert("konten".to_string(), Value::String(http_resp.body));
                                fields.insert("header".to_string(), Value::String(String::new()));
                                let clean = path_val.split('?').next().unwrap_or("").trim_start_matches('/');
                                let rel = if clean.is_empty() { "index.html" } else { clean };
                                let full_p = std::path::Path::new(&folder).join(rel);
                                fields.insert("_jalur_berkas".to_string(), Value::String(full_p.to_string_lossy().to_string()));
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::Struct {
                                    name: "ResponsHttp".to_string(),
                                    fields,
                                }))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }

                    // Cryptography & Encoding Utilities
                    if name == "sha1" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'sha1' membutuhkan 1 argumen (teks)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let s = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return Ok(Value::String(crate::crypto::sha1(&s)));
                    }
                    if name == "kunci_websocket" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'kunci_websocket' membutuhkan 1 argumen (kunci_klien)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let s = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return Ok(Value::String(crate::crypto::hitung_kunci_websocket(&s)));
                    }
                    if name == "respons_upgrade_websocket" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'respons_upgrade_websocket' membutuhkan 1 argumen (kunci_klien)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let s = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match crate::net::buat_respons_upgrade_websocket(&s) {
                            Ok(http_resp) => {
                                let mut fields = HashMap::new();
                                fields.insert("status".to_string(), Value::Int(101));
                                fields.insert("tipe_konten".to_string(), Value::String(String::new()));
                                fields.insert("konten".to_string(), Value::String(String::new()));
                                let mut h_buf = String::new();
                                for (k, v) in &http_resp.headers {
                                    h_buf.push_str(&format!("{}: {}\r\n", k, v));
                                }
                                fields.insert("header".to_string(), Value::String(h_buf));
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::Struct {
                                    name: "ResponsHttp".to_string(),
                                    fields,
                                }))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "sha256" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'sha256' membutuhkan 1 argumen (teks)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let text = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return Ok(Value::String(crate::crypto::sha256(&text)));
                    }
                    if name == "md5" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'md5' membutuhkan 1 argumen (teks)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let text = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return Ok(Value::String(crate::crypto::md5(&text)));
                    }
                    if name == "ke_base64" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ke_base64' membutuhkan 1 argumen (teks)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let text = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return Ok(Value::String(crate::crypto::ke_base64(&text)));
                    }
                    if name == "dari_base64" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'dari_base64' membutuhkan 1 argumen (teks)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        let text = match val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match crate::crypto::dari_base64(&text) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }

                    // Standard Library: Embedded Database (BasisData)
                    if name == "buka_basis_data" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'buka_basis_data' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p_val = self.eval_expr(&args[0].value, env)?;
                        let path = match p_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        return match crate::db::db_buka(&path) {
                            Ok(_) => {
                                let mut fields = HashMap::new();
                                fields.insert("jalur".to_string(), Value::String(path));
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::Struct {
                                    name: "BasisData".to_string(),
                                    fields,
                                }))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_simpan" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'db_simpan' membutuhkan 3 argumen (jalur, kunci, nilai)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let k = self.eval_expr(&args[1].value, env)?.to_string();
                        let v = self.eval_expr(&args[2].value, env)?.to_string();
                        return match crate::db::db_simpan(&p, &k, &v) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_ambil" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'db_ambil' membutuhkan 2 argumen (jalur, kunci)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let k = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::db::db_ambil(&p, &k) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_hapus" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'db_hapus' membutuhkan 2 argumen (jalur, kunci)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let k = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::db::db_hapus(&p, &k) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_ada" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'db_ada' membutuhkan 2 argumen (jalur, kunci)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let k = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::Bool(crate::db::db_ada(&p, &k)));
                    }
                    if name == "db_daftar_kunci" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_daftar_kunci' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::db::db_daftar_kunci(&p) {
                            Ok(keys) => {
                                let v_keys: Vec<Value> = keys.into_iter().map(Value::String).collect();
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::List(Arc::new(Mutex::new(v_keys)))))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_padatkan" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_padatkan' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::db::db_padatkan(&p) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_ambil_awalan" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'db_ambil_awalan' membutuhkan 2 argumen (jalur, awalan)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let a = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::db::db_ambil_awalan(&p, &a) {
                            Ok(keys) => {
                                let v_keys: Vec<Value> = keys.into_iter().map(Value::String).collect();
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::List(Arc::new(Mutex::new(v_keys)))))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_mulai_transaksi" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_mulai_transaksi' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::db::db_mulai_transaksi(&p) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_sedang_transaksi" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_sedang_transaksi' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return Ok(Value::Bool(crate::db::db_sedang_transaksi(&p)));
                    }
                    if name == "db_komit" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_komit' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::db::db_komit(&p) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_batal" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_batal' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::db::db_batal(&p) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_ekspor_json" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'db_ekspor_json' membutuhkan 1 argumen (jalur)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        return match crate::db::db_ekspor_json(&p) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_ekspor_json_awalan" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'db_ekspor_json_awalan' membutuhkan 2 argumen (jalur, awalan)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let a = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::db::db_ekspor_json_awalan(&p, &a) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "db_impor_json" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'db_impor_json' membutuhkan 2 argumen (jalur, json_teks)").with_span(*span));
                        }
                        let p = self.eval_expr(&args[0].value, env)?.to_string();
                        let j = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::db::db_impor_json(&p, &j) {
                            Ok(count) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Int(count as u64))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }

                    // Standard Library: Reguler Ekspresi (Regex)
                    if name == "cocok_regex" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'cocok_regex' membutuhkan 2 argumen: (pola, teks)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let teks = self.eval_expr(&args[1].value, env)?.to_string();
                        return Ok(Value::Bool(crate::regex::cocok_regex(&pola, &teks)));
                    }
                    if name == "cari_regex" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'cari_regex' membutuhkan 2 argumen: (pola, teks)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let teks = self.eval_expr(&args[1].value, env)?.to_string();
                        let matches = crate::regex::cari_regex(&pola, &teks);
                        let v_list: Vec<Value> = matches.into_iter().map(Value::String).collect();
                        return Ok(Value::List(std::sync::Arc::new(std::sync::Mutex::new(v_list))));
                    }
                    if name == "ganti_regex" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'ganti_regex' membutuhkan 3 argumen: (pola, teks, pengganti)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let teks = self.eval_expr(&args[1].value, env)?.to_string();
                        let pengganti = self.eval_expr(&args[2].value, env)?.to_string();
                        return Ok(Value::String(crate::regex::ganti_regex(&pola, &teks, &pengganti)));
                    }
                    if name == "ekstrak_regex" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ekstrak_regex' membutuhkan 2 argumen: (pola, teks)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let teks = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::regex::ekstrak_regex(&pola, &teks) {
                            Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "ekstrak_grup_regex" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'ekstrak_grup_regex' membutuhkan 2 argumen: (pola, teks)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let teks = self.eval_expr(&args[1].value, env)?.to_string();
                        return match crate::regex::ekstrak_grup_regex(&pola, &teks) {
                            Ok(groups) => {
                                let v_groups: Vec<Value> = groups.into_iter().map(Value::String).collect();
                                Ok(Value::ErrorUnion(Ok(Box::new(Value::List(Arc::new(Mutex::new(v_groups)))))))
                            }
                            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                        };
                    }
                    if name == "pisah_regex" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'pisah_regex' membutuhkan 2 argumen: (pola, teks)").with_span(*span));
                        }
                        let pola = self.eval_expr(&args[0].value, env)?.to_string();
                        let teks = self.eval_expr(&args[1].value, env)?.to_string();
                        let parts = crate::regex::pisah_regex(&pola, &teks);
                        let v_parts: Vec<Value> = parts.into_iter().map(Value::String).collect();
                        return Ok(Value::List(Arc::new(Mutex::new(v_parts))));
                    }

                    // Standard Library: WebAssembly HTML5 Canvas 2D (Headless Mock Runtime)
                    if name == "kanvas_lebar" {
                        return Ok(Value::Float(800.0));
                    }
                    if name == "kanvas_tinggi" {
                        return Ok(Value::Float(600.0));
                    }
                    if name == "kanvas_mouse_x" {
                        return Ok(Value::Float(400.0));
                    }
                    if name == "kanvas_mouse_y" {
                        return Ok(Value::Float(300.0));
                    }
                    if name == "kanvas_mouse_klik" {
                        return Ok(Value::Int(0));
                    }
                    if name == "kanvas_tombol_ditekan" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'kanvas_tombol_ditekan' membutuhkan 1 argumen: (kode)").with_span(*span));
                        }
                        let _ = self.eval_expr(&args[0].value, env)?;
                        return Ok(Value::Int(0));
                    }
                    if name == "kanvas_delta_waktu" {
                        return Ok(Value::Float(0.0166));
                    }
                    if name == "kanvas_bersihkan"
                        || name == "kanvas_warna_isi"
                        || name == "kanvas_warna_garis"
                        || name == "kanvas_tebal_garis"
                        || name == "kanvas_persegi"
                        || name == "kanvas_bingkai_persegi"
                        || name == "kanvas_lingkaran"
                        || name == "kanvas_bingkai_lingkaran"
                        || name == "kanvas_garis"
                        || name == "kanvas_titik"
                        || name == "kanvas_teks"
                    {
                        for arg in args {
                            let _ = self.eval_expr(&arg.value, env)?;
                        }
                        return Ok(Value::Unit);
                    }

                    // Standard Library: Web Audio Synthesizer
                    if name == "audio_frekuensi_nada" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'audio_frekuensi_nada' membutuhkan 1 argumen: (notasi)").with_span(*span));
                        }
                        let val = self.eval_expr(&args[0].value, env)?;
                        if let Value::String(s) = val {
                            let freq = note_to_frequency(&s);
                            return Ok(Value::Float(freq));
                        }
                        return Err(Diagnostic::error("Argumen 'audio_frekuensi_nada' harus berupa teks notasi (contoh: 'A4', 'C4')").with_span(*span));
                    }
                    if name == "audio_mainkan_nada" {
                        if args.len() != 3 {
                            return Err(Diagnostic::error("'audio_mainkan_nada' membutuhkan 3 argumen: (frekuensi, durasi, volume)").with_span(*span));
                        }
                        let freq = match self.eval_expr(&args[0].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Frekuensi audio harus berupa angka").with_span(*span)),
                        };
                        let dur = match self.eval_expr(&args[1].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Durasi audio harus berupa angka (detik)").with_span(*span)),
                        };
                        let vol = match self.eval_expr(&args[2].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Volume audio harus berupa angka (0.0 - 1.0)").with_span(*span)),
                        };
                        let log_msg = format!("[Audio] Mainkan nada freq={:.2}Hz durasi={:.2}s vol={:.2}", freq, dur, vol);
                        self.output.lock().unwrap().push(log_msg);
                        return Ok(Value::Unit);
                    }
                    if name == "audio_mainkan_gelombang" {
                        if args.len() != 4 {
                            return Err(Diagnostic::error("'audio_mainkan_gelombang' membutuhkan 4 argumen: (tipe, frekuensi, durasi, volume)").with_span(*span));
                        }
                        let tipe = match self.eval_expr(&args[0].value, env)? {
                            Value::Int(i) => i,
                            _ => 0,
                        };
                        let freq = match self.eval_expr(&args[1].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Frekuensi audio harus berupa angka").with_span(*span)),
                        };
                        let dur = match self.eval_expr(&args[2].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Durasi audio harus berupa angka (detik)").with_span(*span)),
                        };
                        let vol = match self.eval_expr(&args[3].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => return Err(Diagnostic::error("Volume audio harus berupa angka (0.0 - 1.0)").with_span(*span)),
                        };
                        let tipe_name = match tipe % 4 {
                            0 => "sine",
                            1 => "square",
                            2 => "sawtooth",
                            _ => "triangle",
                        };
                        let log_msg = format!("[Audio] Gelombang {} freq={:.2}Hz durasi={:.2}s vol={:.2}", tipe_name, freq, dur, vol);
                        self.output.lock().unwrap().push(log_msg);
                        return Ok(Value::Unit);
                    }
                    if name == "audio_efek_suara" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'audio_efek_suara' membutuhkan 1 argumen: (tipe_efek)").with_span(*span));
                        }
                        let tipe = match self.eval_expr(&args[0].value, env)? {
                            Value::Int(i) => i,
                            _ => 0,
                        };
                        let nama_efek = match tipe {
                            0 => "bip",
                            1 => "lompat",
                            2 => "koin",
                            3 => "ledakan",
                            4 => "kalah",
                            5 => "menang",
                            _ => "kustom",
                        };
                        let log_msg = format!("[Audio SFX] Efek suara: {}", nama_efek);
                        self.output.lock().unwrap().push(log_msg);
                        return Ok(Value::Unit);
                    }
                    if name == "audio_set_volume" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'audio_set_volume' membutuhkan 1 argumen: (volume)").with_span(*span));
                        }
                        let vol = match self.eval_expr(&args[0].value, env)? {
                            Value::Float(f) => f,
                            Value::Int(i) => i as f64,
                            _ => 1.0,
                        };
                        let log_msg = format!("[Audio] Set master volume={:.2}", vol);
                        self.output.lock().unwrap().push(log_msg);
                        return Ok(Value::Unit);
                    }
                    if name == "audio_hentikan_semua" {
                        let log_msg = "[Audio] Hentikan semua audio".to_string();
                        self.output.lock().unwrap().push(log_msg);
                        return Ok(Value::Unit);
                    }

                    // Compiler Service: In-Memory WebAssembly Compilation
                    if name == "kompilasi_ke_wasm" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'kompilasi_ke_wasm' membutuhkan 1 argumen: (kode)").with_span(*span));
                        }
                        let kode_val = self.eval_expr(&args[0].value, env)?;
                        let kode = match kode_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };

                        let mut lexer = crate::lexer::Lexer::new(&kode, 0);
                        let tokens = match lexer.tokenize() {
                            Ok(t) => t,
                            Err(diag) => return Ok(Value::ErrorUnion(Err(Box::new(Value::String(diag.message))))),
                        };
                        let mut parser = crate::parser::Parser::new(tokens, 0);
                        let program = match parser.parse_program() {
                            Ok(p) => p,
                            Err(diag) => return Ok(Value::ErrorUnion(Err(Box::new(Value::String(diag.message))))),
                        };

                        let mut tc = crate::types::TypeChecker::new(0);
                        if let Err(diag) = tc.check_program(&program) {
                            return Ok(Value::ErrorUnion(Err(Box::new(Value::String(diag.message)))));
                        }

                        let mut emitter = crate::wasm::WasmEmitter::new();
                        let wasm_bytes = match emitter.compile_program(&program) {
                            Ok(b) => b,
                            Err(diag) => return Ok(Value::ErrorUnion(Err(Box::new(Value::String(diag.message))))),
                        };

                        let b64 = crate::crypto::ke_base64_bytes(&wasm_bytes);
                        return Ok(Value::ErrorUnion(Ok(Box::new(Value::String(b64)))));
                    }
                    if name == "buat_server_http" {
                        if args.len() < 2 || args.len() > 3 {
                            return Err(Diagnostic::error("'buat_server_http' membutuhkan 2 atau 3 argumen: (port, nama_penangan, maks_permintaan: opsional)").with_span(*span));
                        }
                        let port_val = self.eval_expr(&args[0].value, env)?;
                        let port = match port_val {
                            Value::Int(n) => n as u16,
                            Value::Float(f) => f as u16,
                            _ => return Err(Diagnostic::error("Port harus berupa angka bulat").with_span(*span)),
                        };
                        let handler_val = self.eval_expr(&args[1].value, env)?;
                        let handler_name = match handler_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let max_requests = if args.len() == 3 {
                            let m_val = self.eval_expr(&args[2].value, env)?;
                            match m_val {
                                Value::Int(n) => n,
                                Value::Float(f) => f as u64,
                                _ => 0,
                            }
                        } else {
                            0
                        };

                        return self.eval_builtin_buat_server_http(port, max_requests, &handler_name, *span);
                    }

                    if name == "buat_server_websocket" {
                        if args.len() < 2 || args.len() > 3 {
                            return Err(Diagnostic::error("'buat_server_websocket' membutuhkan 2 atau 3 argumen: (port, nama_penangan, maks_permintaan: opsional)").with_span(*span));
                        }
                        let port_val = self.eval_expr(&args[0].value, env)?;
                        let port = match port_val {
                            Value::Int(n) => n as u16,
                            Value::Float(f) => f as u16,
                            _ => return Err(Diagnostic::error("Port harus bernilai bilangan (u64)").with_span(*span)),
                        };
                        let handler_val = self.eval_expr(&args[1].value, env)?;
                        let handler_name = match handler_val {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        let max_requests = if args.len() == 3 {
                            let m_val = self.eval_expr(&args[2].value, env)?;
                            match m_val {
                                Value::Int(n) => n,
                                Value::Float(f) => f as u64,
                                _ => 0,
                            }
                        } else {
                            0
                        };

                        return self.eval_builtin_buat_server_websocket(port, max_requests, &handler_name);
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

    #[inline(never)]
    pub fn eval_method_call(
        &mut self,
        receiver: &Expr,
        method: &String,
        args: &[CallArg],
        span: Span,
        env: &mut Environment,
    ) -> Result<Value> {
        let span = &span;

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

                if let Value::Struct { name: struct_name, fields } = &rec_val {
                    if struct_name == "BasisData" {
                        let path = fields.get("jalur").map(|v| match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        }).unwrap_or_default();
                        match method.as_str() {
                            "simpan" => {
                                let k = self.eval_expr(&args[0].value, env)?.to_string();
                                let v = self.eval_expr(&args[1].value, env)?.to_string();
                                return match crate::db::db_simpan(&path, &k, &v) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "ambil" => {
                                let k = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::db::db_ambil(&path, &k) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "hapus" => {
                                let k = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::db::db_hapus(&path, &k) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "ada" => {
                                let k = self.eval_expr(&args[0].value, env)?.to_string();
                                return Ok(Value::Bool(crate::db::db_ada(&path, &k)));
                            }
                            "daftar_kunci" => {
                                return match crate::db::db_daftar_kunci(&path) {
                                    Ok(keys) => {
                                        let v_keys: Vec<Value> = keys.into_iter().map(Value::String).collect();
                                        Ok(Value::ErrorUnion(Ok(Box::new(Value::List(Arc::new(Mutex::new(v_keys)))))))
                                    }
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "padatkan" => {
                                return match crate::db::db_padatkan(&path) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "ambil_awalan" => {
                                let a = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::db::db_ambil_awalan(&path, &a) {
                                    Ok(keys) => {
                                        let v_keys: Vec<Value> = keys.into_iter().map(Value::String).collect();
                                        Ok(Value::ErrorUnion(Ok(Box::new(Value::List(Arc::new(Mutex::new(v_keys)))))))
                                    }
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "mulai_transaksi" => {
                                return match crate::db::db_mulai_transaksi(&path) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "sedang_transaksi" => {
                                return Ok(Value::Bool(crate::db::db_sedang_transaksi(&path)));
                            }
                            "komit" => {
                                return match crate::db::db_komit(&path) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "batal" => {
                                return match crate::db::db_batal(&path) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Bool(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "ekspor_json" => {
                                return match crate::db::db_ekspor_json(&path) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "ekspor_json_awalan" => {
                                let a = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::db::db_ekspor_json_awalan(&path, &a) {
                                    Ok(res) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(res))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "impor_json" => {
                                let j = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::db::db_impor_json(&path, &j) {
                                    Ok(count) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Int(count as u64))))),
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            other => return Err(Diagnostic::error(format!("Metode '{}' tidak ditemukan pada BasisData", other)).with_span(*span)),
                        }
                    }
                    if struct_name == "PermintaanHttp" {
                        let jalur = fields.get("jalur").map(|v| v.to_string()).unwrap_or_default();
                        let kueri = fields.get("kueri").map(|v| v.to_string()).unwrap_or_default();
                        match method.as_str() {
                            "cocok" => {
                                let pola = self.eval_expr(&args[0].value, env)?.to_string();
                                return Ok(Value::Bool(crate::net::cocok_rute(&pola, &jalur)));
                            }
                            "param" => {
                                let pola = self.eval_expr(&args[0].value, env)?.to_string();
                                let nama = self.eval_expr(&args[1].value, env)?.to_string();
                                return match crate::net::ambil_param_rute(&pola, &jalur, &nama) {
                                    Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                                    Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                                };
                            }
                            "kueri_param" => {
                                let kunci = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::net::ambil_kueri_param(&kueri, &kunci) {
                                    Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                                    Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                                };
                            }
                            "ada_kueri" => {
                                let kunci = self.eval_expr(&args[0].value, env)?.to_string();
                                return Ok(Value::Bool(crate::net::ada_kueri_param(&kueri, &kunci)));
                            }
                            "kueri_atau" => {
                                let kunci = self.eval_expr(&args[0].value, env)?.to_string();
                                let bawaan = self.eval_expr(&args[1].value, env)?.to_string();
                                return Ok(Value::String(crate::net::kueri_param_atau(&kueri, &kunci, &bawaan)));
                            }
                            "semua_kueri" => {
                                let all = crate::net::semua_kueri_param(&kueri);
                                let v_all: Vec<Value> = all.into_iter().map(Value::String).collect();
                                return Ok(Value::List(Arc::new(Mutex::new(v_all))));
                            }
                            "header" => {
                                let raw = fields.get("_raw_headers").or_else(|| fields.get("header")).map(|v| v.to_string()).unwrap_or_default();
                                let kunci = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::net::ambil_header_http(&raw, &kunci) {
                                    Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                                    Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                                };
                            }
                            "ada_header" => {
                                let raw = fields.get("_raw_headers").or_else(|| fields.get("header")).map(|v| v.to_string()).unwrap_or_default();
                                let kunci = self.eval_expr(&args[0].value, env)?.to_string();
                                return Ok(Value::Bool(crate::net::ada_header_http(&raw, &kunci)));
                            }
                            "header_atau" => {
                                let raw = fields.get("_raw_headers").or_else(|| fields.get("header")).map(|v| v.to_string()).unwrap_or_default();
                                let kunci = self.eval_expr(&args[0].value, env)?.to_string();
                                let bawaan = self.eval_expr(&args[1].value, env)?.to_string();
                                return Ok(Value::String(crate::net::header_atau(&raw, &kunci, &bawaan)));
                            }
                            "otorisasi_bearer" => {
                                let raw = fields.get("_raw_headers").or_else(|| fields.get("header")).map(|v| v.to_string()).unwrap_or_default();
                                return match crate::net::ambil_otorisasi_bearer(&raw) {
                                    Ok(val) => Ok(Value::ErrorUnion(Ok(Box::new(Value::String(val))))),
                                    Err(err) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(err))))),
                                };
                            }
                            "cocok_awalan" => {
                                let awalan = self.eval_expr(&args[0].value, env)?.to_string();
                                return Ok(Value::Bool(crate::net::cocok_awalan(&jalur, &awalan)));
                            }
                            "sisa_jalur" => {
                                let awalan = self.eval_expr(&args[0].value, env)?.to_string();
                                return Ok(Value::String(crate::net::sisa_jalur(&jalur, &awalan)));
                            }
                            "sajikan_statis" => {
                                let folder = self.eval_expr(&args[0].value, env)?.to_string();
                                return match crate::net::sajikan_berkas_statis(&folder, &jalur) {
                                    Ok(http_resp) => {
                                        let mut fields = HashMap::new();
                                        fields.insert("status".to_string(), Value::Int(http_resp.status));
                                        fields.insert("tipe_konten".to_string(), Value::String(http_resp.content_type));
                                        fields.insert("konten".to_string(), Value::String(http_resp.body));
                                        fields.insert("header".to_string(), Value::String(String::new()));
                                        let clean = jalur.split('?').next().unwrap_or("").trim_start_matches('/');
                                        let rel = if clean.is_empty() { "index.html" } else { clean };
                                        let full_p = std::path::Path::new(&folder).join(rel);
                                        fields.insert("_jalur_berkas".to_string(), Value::String(full_p.to_string_lossy().to_string()));
                                        Ok(Value::ErrorUnion(Ok(Box::new(Value::Struct {
                                            name: "ResponsHttp".to_string(),
                                            fields,
                                        }))))
                                    }
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            "adalah_websocket" => {
                                let raw = fields.get("_raw_headers").map(|v| v.to_string()).unwrap_or_default();
                                let lower = raw.to_lowercase();
                                let is_ws = lower.contains("upgrade: websocket") && lower.contains("sec-websocket-key:");
                                return Ok(Value::Bool(is_ws));
                            }
                            "kunci_websocket" => {
                                let raw = fields.get("_raw_headers").map(|v| v.to_string()).unwrap_or_default();
                                for line in raw.lines() {
                                    if let Some((k, v)) = line.split_once(':') {
                                        if k.trim().eq_ignore_ascii_case("sec-websocket-key") {
                                            return Ok(Value::ErrorUnion(Ok(Box::new(Value::String(v.trim().to_string())))));
                                        }
                                    }
                                }
                                return Ok(Value::ErrorUnion(Err(Box::new(Value::String("Header Sec-WebSocket-Key tidak ditemukan".to_string())))));
                            }
                            "tingkatkan_websocket" => {
                                let ws_fn = self.eval_expr(&args[0].value, env)?.to_string();
                                let raw = fields.get("_raw_headers").map(|v| v.to_string()).unwrap_or_default();
                                let mut client_key = None;
                                for line in raw.lines() {
                                    if let Some((k, v)) = line.split_once(':') {
                                        if k.trim().eq_ignore_ascii_case("sec-websocket-key") {
                                            client_key = Some(v.trim().to_string());
                                            break;
                                        }
                                    }
                                }
                                let key = match client_key {
                                    Some(k) => k,
                                    None => return Ok(Value::ErrorUnion(Err(Box::new(Value::String("Header Sec-WebSocket-Key tidak ditemukan".to_string()))))),
                                };

                                return match crate::net::buat_respons_upgrade_websocket(&key) {
                                    Ok(http_resp) => {
                                        let mut r_fields = HashMap::new();
                                        r_fields.insert("status".to_string(), Value::Int(101));
                                        r_fields.insert("tipe_konten".to_string(), Value::String(String::new()));
                                        r_fields.insert("konten".to_string(), Value::String(String::new()));
                                        let mut h_buf = String::new();
                                        for (k, v) in &http_resp.headers {
                                            h_buf.push_str(&format!("{}: {}\r\n", k, v));
                                        }
                                        r_fields.insert("header".to_string(), Value::String(h_buf));
                                        r_fields.insert("_ws_handler".to_string(), Value::String(ws_fn));
                                        Ok(Value::ErrorUnion(Ok(Box::new(Value::Struct {
                                            name: "ResponsHttp".to_string(),
                                            fields: r_fields,
                                        }))))
                                    }
                                    Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
                                };
                            }
                            other => return Err(Diagnostic::error(format!("Metode '{}' tidak ditemukan pada PermintaanHttp", other)).with_span(*span)),
                        }
                    }
                    if struct_name == "ResponsHttp" {
                        match method.as_str() {
                            "tambah_header" => {
                                let k = self.eval_expr(&args[0].value, env)?.to_string();
                                let v = self.eval_expr(&args[1].value, env)?.to_string();
                                let mut new_fields = fields.clone();
                                let mut current_h = new_fields.get("header").map(|val| val.to_string()).unwrap_or_default();
                                current_h.push_str(&format!("{}: {}\r\n", k, v));
                                new_fields.insert("header".to_string(), Value::String(current_h));
                                return Ok(Value::Struct {
                                    name: "ResponsHttp".to_string(),
                                    fields: new_fields,
                                });
                            }
                            "cors" => {
                                let mut new_fields = fields.clone();
                                let mut current_h = new_fields.get("header").map(|val| val.to_string()).unwrap_or_default();
                                current_h.push_str("Access-Control-Allow-Origin: *\r\n");
                                current_h.push_str("Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n");
                                current_h.push_str("Access-Control-Allow-Headers: Content-Type, Authorization\r\n");
                                new_fields.insert("header".to_string(), Value::String(current_h));
                                return Ok(Value::Struct {
                                    name: "ResponsHttp".to_string(),
                                    fields: new_fields,
                                });
                            }
                            "status_teks" => {
                                let status = fields.get("status").map(|v| match v {
                                    Value::Int(i) => *i as u64,
                                    Value::Float(f) => *f as u64,
                                    _ => 200,
                                }).unwrap_or(200);
                                return Ok(Value::String(crate::net::status_ke_teks(status).to_string()));
                            }
                            other => return Err(Diagnostic::error(format!("Metode '{}' tidak ditemukan pada ResponsHttp", other)).with_span(*span)),
                        }
                    }
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

    #[inline(never)]
    fn eval_builtin_buat_server_http(
        &self,
        port: u16,
        max_requests: u64,
        handler_name: &str,
        span: Span,
    ) -> Result<Value> {
        let target_fn = match self.functions.get(handler_name).cloned() {
            Some(f) => f,
            None => {
                return Err(Diagnostic::error(format!(
                    "Fungsi penangan HTTP '{}' tidak ditemukan pada kode sumber",
                    handler_name
                ))
                .with_span(span));
            }
        };

        let output_arc = self.output.clone();
        let functions_clone = self.functions.clone();
        let methods_clone = self.methods.clone();
        let cli_args_clone = self.cli_args.clone();

        let functions_ws = self.functions.clone();
        let output_ws = self.output.clone();
        let methods_ws = self.methods.clone();
        let cli_args_ws = self.cli_args.clone();

        let ws_callback = move |fn_name: &str, incoming_text: &str| -> Option<String> {
            if let Some(ws_target_fn) = functions_ws.get(fn_name) {
                let mut eval = Evaluator {
                    output: output_ws.clone(),
                    functions: functions_ws.clone(),
                    methods: methods_ws.clone(),
                    cli_args: cli_args_ws.clone(),
                    call_stack: Vec::new(),
                    debug_hook: None,
                };
                let mut inner_env = Environment::new();
                let fn_args = vec![Value::String(incoming_text.to_string())];
                match eval.eval_fn(ws_target_fn, fn_args, &mut inner_env) {
                    Ok(ControlFlow::Next(v) | ControlFlow::Return(v)) => match v {
                        Value::String(s) => Some(s),
                        Value::Unit => None,
                        other => Some(other.to_string()),
                    },
                    _ => None,
                }
            } else {
                None
            }
        };

        let server_res = crate::net::run_http_server(
            port,
            max_requests,
            move |req| {
                let mut eval = Evaluator {
                    output: output_arc.clone(),
                    functions: functions_clone.clone(),
                    methods: methods_clone.clone(),
                    cli_args: cli_args_clone.clone(),
                    call_stack: Vec::new(),
                    debug_hook: None,
                };
                let mut inner_env = Environment::new();

                let fn_args = if target_fn.params.len() == 1 {
                    let mut req_fields = HashMap::new();
                    req_fields.insert("metode".to_string(), Value::String(req.method.clone()));
                    req_fields.insert("jalur".to_string(), Value::String(req.path.clone()));
                    req_fields.insert("kueri".to_string(), Value::String(req.query.clone()));
                    req_fields.insert("isi".to_string(), Value::String(req.body.clone()));
                    let mut h_buf = String::new();
                    for (k, v) in &req.headers {
                        h_buf.push_str(&format!("{}: {}\r\n", k, v));
                    }
                    req_fields.insert("header".to_string(), Value::String(h_buf.clone()));
                    req_fields.insert("_raw_headers".to_string(), Value::String(h_buf));
                    vec![Value::Struct {
                        name: "PermintaanHttp".to_string(),
                        fields: req_fields,
                    }]
                } else if target_fn.params.len() == 2 {
                    vec![Value::String(req.method.clone()), Value::String(req.path.clone())]
                } else if target_fn.params.len() == 3 {
                    vec![
                        Value::String(req.method.clone()),
                        Value::String(req.path.clone()),
                        Value::String(req.body.clone()),
                    ]
                } else {
                    vec![
                        Value::String(req.method.clone()),
                        Value::String(req.path.clone()),
                        Value::String(req.query.clone()),
                        Value::String(req.body.clone()),
                    ]
                };

                let result_val = match eval.eval_fn(&target_fn, fn_args, &mut inner_env) {
                    Ok(ControlFlow::Next(v) | ControlFlow::Return(v)) => v,
                    Ok(ControlFlow::Fail(e)) => Value::ErrorUnion(Err(Box::new(e))),
                    Err(d) => Value::ErrorUnion(Err(Box::new(Value::String(d.to_string())))),
                };

                match result_val {
                    Value::Struct { name, fields } if name == "ResponsHttp" => {
                        let status = fields
                            .get("status")
                            .and_then(|v| match v {
                                Value::Int(n) => Some(*n),
                                Value::Float(f) => Some(*f as u64),
                                _ => None,
                            })
                            .unwrap_or(200);
                        let content_type = fields
                            .get("tipe_konten")
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "text/plain; charset=utf-8".to_string());
                        let body = fields.get("konten").map(|v| v.to_string()).unwrap_or_default();
                        let mut headers = HashMap::new();
                        if let Some(h_val) = fields.get("header") {
                            let h_str = h_val.to_string();
                            for line in h_str.lines() {
                                let line = line.trim();
                                if let Some((k, v)) = line.split_once(':') {
                                    headers.insert(k.trim().to_string(), v.trim().to_string());
                                }
                            }
                        }
                        if let Some(ws_h) = fields.get("_ws_handler") {
                            headers.insert("_ws_handler".to_string(), ws_h.to_string());
                        }
                        let raw_body = if let Some(h_p) = fields.get("_jalur_berkas") {
                            std::fs::read(h_p.to_string()).ok()
                        } else {
                            None
                        };
                        Ok(crate::net::HttpServerResponse {
                            status,
                            content_type,
                            body,
                            raw_body,
                            headers,
                        })
                    }
                    Value::String(s) => {
                        let trimmed = s.trim();
                        let is_json = (trimmed.starts_with('{') && trimmed.ends_with('}'))
                            || (trimmed.starts_with('[') && trimmed.ends_with(']'));
                        let content_type = if is_json {
                            "application/json; charset=utf-8".to_string()
                        } else {
                            "text/plain; charset=utf-8".to_string()
                        };
                        Ok(crate::net::HttpServerResponse {
                            status: 200,
                            content_type,
                            body: s,
                            raw_body: None,
                            headers: HashMap::new(),
                        })
                    }
                    Value::ErrorUnion(Err(e)) => Ok(crate::net::HttpServerResponse {
                        status: 500,
                        content_type: "text/plain; charset=utf-8".to_string(),
                        body: format!("500 Internal Server Error: {}", e),
                        raw_body: None,
                        headers: HashMap::new(),
                    }),
                    Value::ErrorUnion(Ok(ok_v)) => {
                        let s = ok_v.to_string();
                        Ok(crate::net::HttpServerResponse {
                            status: 200,
                            content_type: "text/plain; charset=utf-8".to_string(),
                            body: s,
                            raw_body: None,
                            headers: HashMap::new(),
                        })
                    }
                    other => Ok(crate::net::HttpServerResponse {
                        status: 200,
                        content_type: "application/json; charset=utf-8".to_string(),
                        body: value_to_json(&other),
                        raw_body: None,
                        headers: HashMap::new(),
                    }),
                }
            },
            ws_callback,
        );

        match server_res {
            Ok(count) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Int(count))))),
            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
        }
    }

    #[inline(never)]
    fn eval_builtin_buat_server_websocket(
        &self,
        port: u16,
        max_requests: u64,
        handler_name: &str,
    ) -> Result<Value> {
        let h_name_clone = handler_name.to_string();
        let functions_ws = self.functions.clone();
        let output_ws = self.output.clone();
        let methods_ws = self.methods.clone();
        let cli_args_ws = self.cli_args.clone();

        let ws_callback = move |fn_name: &str, incoming_text: &str| -> Option<String> {
            if let Some(ws_target_fn) = functions_ws.get(fn_name) {
                let mut eval = Evaluator {
                    output: output_ws.clone(),
                    functions: functions_ws.clone(),
                    methods: methods_ws.clone(),
                    cli_args: cli_args_ws.clone(),
                    call_stack: Vec::new(),
                    debug_hook: None,
                };
                let mut inner_env = Environment::new();
                let fn_args = vec![Value::String(incoming_text.to_string())];
                match eval.eval_fn(ws_target_fn, fn_args, &mut inner_env) {
                    Ok(ControlFlow::Next(v) | ControlFlow::Return(v)) => match v {
                        Value::String(s) => Some(s),
                        Value::Unit => None,
                        other => Some(other.to_string()),
                    },
                    _ => None,
                }
            } else {
                None
            }
        };

        let server_res = crate::net::run_http_server(
            port,
            max_requests,
            move |req| {
                if crate::net::adalah_permintaan_websocket(&req) {
                    if let Some(key) = crate::net::ambil_kunci_websocket(&req) {
                        let mut resp = crate::net::buat_respons_upgrade_websocket(&key)?;
                        resp.headers.insert("_ws_handler".to_string(), h_name_clone.clone());
                        return Ok(resp);
                    }
                }
                Ok(crate::net::HttpServerResponse {
                    status: 400,
                    content_type: "text/plain; charset=utf-8".to_string(),
                    body: "Hanya menerima koneksi WebSocket".to_string(),
                    raw_body: None,
                    headers: HashMap::new(),
                })
            },
            ws_callback,
        );

        match server_res {
            Ok(count) => Ok(Value::ErrorUnion(Ok(Box::new(Value::Int(count))))),
            Err(e) => Ok(Value::ErrorUnion(Err(Box::new(Value::String(e))))),
        }
    }
}
