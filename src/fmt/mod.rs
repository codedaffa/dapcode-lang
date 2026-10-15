use crate::ast::*;
use std::fmt::Write;

pub struct Formatter {
    indent_level: usize,
    indent_str: &'static str,
}

impl Formatter {
    pub fn new() -> Self {
        Formatter {
            indent_level: 0,
            indent_str: "    ",
        }
    }

    fn indent(&self) -> String {
        self.indent_str.repeat(self.indent_level)
    }

    pub fn format_program(&mut self, program: &Program) -> String {
        let mut out = String::new();
        for (i, decl) in program.decls.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&self.format_decl(decl));
            out.push('\n');
        }
        out
    }

    pub fn format_decl(&mut self, decl: &Decl) -> String {
        match decl {
            Decl::Import(imp) => format!("impor \"{}\"", imp.path),
            Decl::Struct(s) => self.format_struct(s),
            Decl::Trait(t) => self.format_trait(t),
            Decl::Impl(i) => self.format_impl(i),
            Decl::Fn(f) => self.format_fn(f),
        }
    }

    fn format_struct(&self, s: &StructDecl) -> String {
        let mut out = format!("struktur {} {{\n", s.name);
        for f in &s.fields {
            out.push_str(&format!("{}    {}: {}\n", self.indent(), f.name, self.format_type(&f.ty)));
        }
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn format_trait(&self, t: &TraitDecl) -> String {
        let mut out = format!("sifat {} {{\n", t.name);
        for m in &t.methods {
            let mut params_str = Vec::new();
            for p in &m.params {
                if p.is_self {
                    params_str.push("ini".to_string());
                } else if let Some(ty) = &p.ty {
                    params_str.push(format!("{}: {}", p.name, self.format_type(ty)));
                } else {
                    params_str.push(p.name.clone());
                }
            }
            let ret_str = if let Some(r) = &m.ret_ty {
                format!(" -> {}", self.format_type(r))
            } else {
                "".to_string()
            };
            out.push_str(&format!(
                "{}    fungsi {}({}){}\n",
                self.indent(),
                m.name,
                params_str.join(", "),
                ret_str
            ));
        }
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn format_impl(&mut self, i: &ImplDecl) -> String {
        let header = if let Some(tr) = &i.trait_name {
            format!("terapkan {} untuk {} {{\n", tr, i.target_struct)
        } else {
            format!("terapkan {} {{\n", i.target_struct)
        };

        let mut out = header;
        self.indent_level += 1;
        for (idx, m) in i.methods.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            out.push_str(&self.format_fn(m));
            out.push('\n');
        }
        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn format_fn(&mut self, f: &FnDecl) -> String {
        let comptime_str = if f.is_comptime { "comptime " } else { "" };
        let mut params_str = Vec::new();
        for p in &f.params {
            if p.is_self {
                params_str.push("ini".to_string());
            } else if let Some(ty) = &p.ty {
                params_str.push(format!("{}: {}", p.name, self.format_type(ty)));
            } else {
                params_str.push(p.name.clone());
            }
        }
        let ret_str = if let Some(r) = &f.ret_ty {
            format!(" -> {}", self.format_type(r))
        } else {
            "".to_string()
        };

        let mut out = format!(
            "{}{}fungsi {}({}){} {{\n",
            self.indent(),
            comptime_str,
            f.name,
            params_str.join(", "),
            ret_str
        );

        self.indent_level += 1;
        for s in &f.body.stmts {
            out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
        }
        if let Some(e) = &f.body.expr {
            out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
        }
        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn format_stmt(&mut self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::VarDecl { name, explicit_ty, init, .. } => {
                if let Some(ty) = explicit_ty {
                    format!("{}: {} = {}", name, self.format_type(ty), self.format_expr(init))
                } else {
                    format!("{} := {}", name, self.format_expr(init))
                }
            }
            Stmt::Assign { target, value, .. } => {
                format!("{} = {}", self.format_expr(target), self.format_expr(value))
            }
            Stmt::If { cond, then_branch, else_branch, .. } => {
                let mut out = format!("jika {} {{\n", self.format_expr(cond));
                self.indent_level += 1;
                for s in &then_branch.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &then_branch.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));

                if let Some(eb) = else_branch {
                    out.push_str(" selain_itu {\n");
                    self.indent_level += 1;
                    for s in &eb.stmts {
                        out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                    }
                    if let Some(e) = &eb.expr {
                        out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                    }
                    self.indent_level -= 1;
                    out.push_str(&format!("{}}}", self.indent()));
                }
                out
            }
            Stmt::While { cond, body, .. } => {
                let mut out = format!("selama {} {{\n", self.format_expr(cond));
                self.indent_level += 1;
                for s in &body.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &body.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
                out
            }
            Stmt::ForIn { var_name, iterable, body, .. } => {
                let mut out = format!("untuk {} di {} {{\n", var_name, self.format_expr(iterable));
                self.indent_level += 1;
                for s in &body.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &body.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
                out
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    format!("kembali {}", self.format_expr(v))
                } else {
                    "kembali".to_string()
                }
            }
            Stmt::Fail { value, .. } => format!("gagal {}", self.format_expr(value)),
            Stmt::Expr(e) => self.format_expr(e),
        }
    }

    fn format_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Lit(lit, _) => match lit {
                Literal::Int(n) => n.to_string(),
                Literal::Float(f) => {
                    if f.fract() == 0.0 {
                        format!("{:.1}", f)
                    } else {
                        f.to_string()
                    }
                }
                Literal::String(s) => format!("\"{}\"", s),
                Literal::Bool(b) => if *b { "benar".to_string() } else { "salah".to_string() },
                Literal::Unit => "()".to_string(),
            },
            Expr::Ident(name, _) => name.clone(),
            Expr::SelfVar(_) => "ini".to_string(),
            Expr::Binary(left, op, right, _) => {
                format!("{} {} {}", self.format_expr(left), op, self.format_expr(right))
            }
            Expr::Unary(op, operand, _) => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                format!("{}{}", op_str, self.format_expr(operand))
            }
            Expr::Call { callee, args, .. } => {
                let mut arg_strs = Vec::new();
                for a in args {
                    if let Some(n) = &a.name {
                        arg_strs.push(format!("{}: {}", n, self.format_expr(&a.value)));
                    } else {
                        arg_strs.push(self.format_expr(&a.value));
                    }
                }
                format!("{}({})", self.format_expr(callee), arg_strs.join(", "))
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                let mut arg_strs = Vec::new();
                for a in args {
                    if let Some(n) = &a.name {
                        arg_strs.push(format!("{}: {}", n, self.format_expr(&a.value)));
                    } else {
                        arg_strs.push(self.format_expr(&a.value));
                    }
                }
                format!("{}.{}({})", self.format_expr(receiver), method, arg_strs.join(", "))
            }
            Expr::FieldAccess { receiver, field, .. } => {
                format!("{}.{}", self.format_expr(receiver), field)
            }
            Expr::InterpolatedString(parts, _) => {
                let mut s = String::from("\"");
                for p in parts {
                    match p {
                        InterpolatedExprPart::Lit(l) => s.push_str(l),
                        InterpolatedExprPart::Expr(e) => {
                            write!(s, "{{{}}}", self.format_expr(e)).unwrap();
                        }
                    }
                }
                s.push('"');
                s
            }
            Expr::ErrorPropagate { inner, .. } => format!("{} ?", self.format_expr(inner)),
            Expr::Catch { inner, err_var, handler, .. } => {
                let mut out = format!("{} ? tangkap {} {{\n", self.format_expr(inner), err_var);
                self.indent_level += 1;
                for s in &handler.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &handler.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
                out
            }
            Expr::Parallel { body, .. } => {
                let mut out = String::from("jalan_paralel {\n");
                self.indent_level += 1;
                for s in &body.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &body.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
                out
            }
            Expr::Await { task, .. } => format!("tunggu({})", self.format_expr(task)),
            Expr::Comptime { body, .. } => {
                let mut out = String::from("comptime {\n");
                self.indent_level += 1;
                for s in &body.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &body.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
                out
            }
            Expr::Block(b) => {
                let mut out = String::from("{\n");
                self.indent_level += 1;
                for s in &b.stmts {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_stmt(s)));
                }
                if let Some(e) = &b.expr {
                    out.push_str(&format!("{}{}\n", self.indent(), self.format_expr(e)));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
                out
            }
        }
    }

    fn format_type(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Named(name, _) => name.clone(),
            TypeExpr::ErrorUnion(ok, err, _) => {
                format!("{} ! {}", self.format_type(ok), self.format_type(err))
            }
            TypeExpr::Task(inner, _) => format!("Tugas<{}>", self.format_type(inner)),
            TypeExpr::List(inner, _) => format!("Daftar<{}>", self.format_type(inner)),
            TypeExpr::Unit(_) => "()".to_string(),
            TypeExpr::Inferred(_) => "_".to_string(),
        }
    }
}
