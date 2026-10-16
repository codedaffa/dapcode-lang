use crate::ast::*;
use crate::ownership::OwnershipReport;
use std::fmt;

#[derive(Debug, Clone)]
pub enum IROp {
    Label(String),
    VarDecl {
        name: String,
        init: String,
        is_borrow: bool,
    },
    Assign {
        target: String,
        value: String,
    },
    Binary {
        dest: String,
        op: String,
        left: String,
        right: String,
    },
    ConstructStruct {
        dest: String,
        struct_name: String,
        fields: Vec<(String, String)>,
    },
    FieldAccess {
        dest: String,
        receiver: String,
        field: String,
    },
    Call {
        dest: Option<String>,
        callee: String,
        args: Vec<String>,
    },
    MethodCall {
        dest: Option<String>,
        receiver: String,
        method: String,
        args: Vec<String>,
        receiver_borrowed: bool,
    },
    SpawnTask {
        dest: String,
        task_label: String,
    },
    AwaitTask {
        dest: String,
        task_var: String,
    },
    DropVar {
        var_name: String,
        perceus_reuse: Option<String>,
    },
    ErrorPropagate {
        dest: String,
        source: String,
    },
    CatchError {
        dest: String,
        err_var: String,
        handler_label: String,
    },
    Return {
        value: Option<String>,
    },
    Fail {
        value: String,
    },
    BranchIf {
        cond: String,
        then_label: String,
        else_label: Option<String>,
    },
    Goto(String),
}

impl fmt::Display for IROp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IROp::Label(lbl) => write!(f, "{}:", lbl),
            IROp::VarDecl { name, init, is_borrow } => {
                let borrow_str = if *is_borrow { "&" } else { "owned" };
                write!(f, "    let {} [{}] = {}", name, borrow_str, init)
            }
            IROp::Assign { target, value } => write!(f, "    {} = {}", target, value),
            IROp::Binary { dest, op, left, right } => {
                write!(f, "    {} = {} {} {}", dest, left, op, right)
            }
            IROp::ConstructStruct { dest, struct_name, fields } => {
                write!(f, "    {} = alloc {}(", dest, struct_name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, ")")
            }
            IROp::FieldAccess { dest, receiver, field } => {
                write!(f, "    {} = {}.{}", dest, receiver, field)
            }
            IROp::Call { dest, callee, args } => {
                if let Some(d) = dest {
                    write!(f, "    {} = call {}({})", d, callee, args.join(", "))
                } else {
                    write!(f, "    call {}({})", callee, args.join(", "))
                }
            }
            IROp::MethodCall { dest, receiver, method, args, receiver_borrowed } => {
                let rec_ref = if *receiver_borrowed { "&" } else { "" };
                let mut all_args = vec![format!("{}{}", rec_ref, receiver)];
                all_args.extend(args.iter().cloned());
                if let Some(d) = dest {
                    write!(f, "    {} = call_method {}({})", d, method, all_args.join(", "))
                } else {
                    write!(f, "    call_method {}({})", method, all_args.join(", "))
                }
            }
            IROp::SpawnTask { dest, task_label } => {
                write!(f, "    {} = spawn_coroutine @{}", dest, task_label)
            }
            IROp::AwaitTask { dest, task_var } => {
                write!(f, "    {} = await_task {}", dest, task_var)
            }
            IROp::DropVar { var_name, perceus_reuse } => {
                if let Some(reuse) = perceus_reuse {
                    write!(f, "    perceus_drop({}, reuse_for={})", var_name, reuse)
                } else {
                    write!(f, "    drop({})", var_name)
                }
            }
            IROp::ErrorPropagate { dest, source } => {
                write!(f, "    {} = unwrap_or_propagate {}", dest, source)
            }
            IROp::CatchError { dest, err_var, handler_label } => {
                write!(f, "    {} = catch_error {} -> @{}", dest, err_var, handler_label)
            }
            IROp::Return { value } => {
                if let Some(v) = value {
                    write!(f, "    return {}", v)
                } else {
                    write!(f, "    return ()")
                }
            }
            IROp::Fail { value } => write!(f, "    fail_err {}", value),
            IROp::BranchIf { cond, then_label, else_label } => {
                if let Some(el) = else_label {
                    write!(f, "    br_if {} then @{} else @{}", cond, then_label, el)
                } else {
                    write!(f, "    br_if {} then @{}", cond, then_label)
                }
            }
            IROp::Goto(lbl) => write!(f, "    jump @{}", lbl),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IRFunction {
    pub name: String,
    pub params: Vec<(String, bool)>, // (param_name, is_borrowed)
    pub ops: Vec<IROp>,
}

impl fmt::Display for IRFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn {} (", self.name)?;
        for (i, (p, is_b)) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            let b_prefix = if *is_b { "&" } else { "" };
            write!(f, "{}{}", b_prefix, p)?;
        }
        writeln!(f, ") {{")?;
        for op in &self.ops {
            writeln!(f, "{}", op)?;
        }
        writeln!(f, "}}")
    }
}

#[derive(Debug, Clone, Default)]
pub struct IRProgram {
    pub functions: Vec<IRFunction>,
}

impl fmt::Display for IRProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "; --- DAPCODE INTERMEDIATE REPRESENTATION (WITH PERCEUS DROPS) ---")?;
        for fn_ir in &self.functions {
            writeln!(f, "\n{}", fn_ir)?;
        }
        Ok(())
    }
}

pub struct IRBuilder<'a> {
    ownership: &'a OwnershipReport,
    tmp_counter: usize,
}

impl<'a> IRBuilder<'a> {
    pub fn new(ownership: &'a OwnershipReport) -> Self {
        IRBuilder {
            ownership,
            tmp_counter: 0,
        }
    }

    fn fresh_tmp(&mut self) -> String {
        let name = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    pub fn build_program(&mut self, program: &Program) -> IRProgram {
        let mut ir_prog = IRProgram::default();

        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    let ir_fn = self.build_function(&f.name, &f.params, &f.body);
                    ir_prog.functions.push(ir_fn);
                }
                Decl::Impl(i) => {
                    for m in &i.methods {
                        let full_name = format!("{}::{}", i.target_struct, m.name);
                        let ir_fn = self.build_function(&full_name, &m.params, &m.body);
                        ir_prog.functions.push(ir_fn);
                    }
                }
                _ => {}
            }
        }

        ir_prog
    }

    fn build_function(&mut self, name: &str, params: &[ParamDecl], body: &Block) -> IRFunction {
        let fa = self.ownership.functions.get(name);
        let mut ir_params = Vec::new();

        for p in params {
            let is_borrowed = fa
                .and_then(|a| a.var_usages.get(&p.name))
                .map(|u| *u == crate::ownership::UsageKind::Borrow)
                .unwrap_or(true);
            ir_params.push((p.name.clone(), is_borrowed));
        }

        let mut ops = Vec::new();

        for (stmt_idx, stmt) in body.stmts.iter().enumerate() {
            self.lower_stmt(stmt, &mut ops);

            // Inject deterministic drops for this statement
            if let Some(fa) = fa {
                if let Some(drops) = fa.drop_points.get(&stmt_idx) {
                    for d in drops {
                        ops.push(IROp::DropVar {
                            var_name: d.var_name.clone(),
                            perceus_reuse: d.reuse_candidate.clone(),
                        });
                    }
                }
            }
        }

        // Lower tail expression if any
        if let Some(tail) = &body.expr {
            let val = self.lower_expr(tail, &mut ops);
            ops.push(IROp::Return { value: Some(val) });
        } else {
            ops.push(IROp::Return { value: None });
        }

        // Add exit drops
        if let Some(fa) = fa {
            for d in &fa.exit_drops {
                ops.push(IROp::DropVar {
                    var_name: d.var_name.clone(),
                    perceus_reuse: None,
                });
            }
        }

        IRFunction {
            name: name.to_string(),
            params: ir_params,
            ops,
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt, ops: &mut Vec<IROp>) {
        match stmt {
            Stmt::VarDecl { name, init, .. } => {
                let init_val = self.lower_expr(init, ops);
                ops.push(IROp::VarDecl {
                    name: name.clone(),
                    init: init_val,
                    is_borrow: false,
                });
            }
            Stmt::Assign { target, value, .. } => {
                let val = self.lower_expr(value, ops);
                let target_name = match target {
                    Expr::Ident(n, _) => n.clone(),
                    _ => self.lower_expr(target, ops),
                };
                ops.push(IROp::Assign {
                    target: target_name,
                    value: val,
                });
            }
            Stmt::If { cond, then_branch, else_branch, .. } => {
                let cond_val = self.lower_expr(cond, ops);
                let then_lbl = format!("then_{}", self.tmp_counter);
                let else_lbl = format!("else_{}", self.tmp_counter);
                let end_lbl = format!("endif_{}", self.tmp_counter);
                self.tmp_counter += 1;

                ops.push(IROp::BranchIf {
                    cond: cond_val,
                    then_label: then_lbl.clone(),
                    else_label: if else_branch.is_some() { Some(else_lbl.clone()) } else { Some(end_lbl.clone()) },
                });

                ops.push(IROp::Label(then_lbl));
                for s in &then_branch.stmts {
                    self.lower_stmt(s, ops);
                }
                ops.push(IROp::Goto(end_lbl.clone()));

                if let Some(eb) = else_branch {
                    ops.push(IROp::Label(else_lbl));
                    for s in &eb.stmts {
                        self.lower_stmt(s, ops);
                    }
                    ops.push(IROp::Goto(end_lbl.clone()));
                }

                ops.push(IROp::Label(end_lbl));
            }
            Stmt::While { cond, body, .. } => {
                let start_lbl = format!("loop_{}", self.tmp_counter);
                let body_lbl = format!("loop_body_{}", self.tmp_counter);
                let end_lbl = format!("loop_end_{}", self.tmp_counter);
                self.tmp_counter += 1;

                ops.push(IROp::Label(start_lbl.clone()));
                let cond_val = self.lower_expr(cond, ops);
                ops.push(IROp::BranchIf {
                    cond: cond_val,
                    then_label: body_lbl.clone(),
                    else_label: Some(end_lbl.clone()),
                });

                ops.push(IROp::Label(body_lbl));
                for s in &body.stmts {
                    self.lower_stmt(s, ops);
                }
                ops.push(IROp::Goto(start_lbl));

                ops.push(IROp::Label(end_lbl));
            }
            Stmt::ForIn { var_name, iterable, body, .. } => {
                let iter_val = self.lower_expr(iterable, ops);
                let start_lbl = format!("for_{}", self.tmp_counter);
                let body_lbl = format!("for_body_{}", self.tmp_counter);
                let end_lbl = format!("for_end_{}", self.tmp_counter);
                self.tmp_counter += 1;

                ops.push(IROp::Label(start_lbl.clone()));
                ops.push(IROp::Assign {
                    target: var_name.clone(),
                    value: iter_val,
                });
                ops.push(IROp::Label(body_lbl));
                for s in &body.stmts {
                    self.lower_stmt(s, ops);
                }
                ops.push(IROp::Label(end_lbl));
            }
            Stmt::Return { value, .. } => {
                let val = value.as_ref().map(|v| self.lower_expr(v, ops));
                ops.push(IROp::Return { value: val });
            }
            Stmt::Fail { value, .. } => {
                let val = self.lower_expr(value, ops);
                ops.push(IROp::Fail { value: val });
            }
            Stmt::Expr(e) => {
                self.lower_expr(e, ops);
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr, ops: &mut Vec<IROp>) -> String {
        match expr {
            Expr::Lit(lit, _) => match lit {
                Literal::Int(n) => n.to_string(),
                Literal::Float(f) => format!("{:.1}", f),
                Literal::String(s) => format!("\"{}\"", s),
                Literal::Bool(b) => if *b { "benar".to_string() } else { "salah".to_string() },
                Literal::Unit => "()".to_string(),
            },
            Expr::Ident(name, _) => name.clone(),
            Expr::SelfVar(_) => "ini".to_string(),
            Expr::Binary(left, op, right, _) => {
                let l = self.lower_expr(left, ops);
                let r = self.lower_expr(right, ops);
                let dest = self.fresh_tmp();
                ops.push(IROp::Binary {
                    dest: dest.clone(),
                    op: op.to_string(),
                    left: l,
                    right: r,
                });
                dest
            }
            Expr::Unary(op, operand, _) => {
                let o = self.lower_expr(operand, ops);
                let dest = self.fresh_tmp();
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                ops.push(IROp::Binary {
                    dest: dest.clone(),
                    op: op_str.to_string(),
                    left: "".to_string(),
                    right: o,
                });
                dest
            }
            Expr::Call { callee, args, .. } => {
                // If it's a struct constructor or function call
                if let Expr::Ident(cname, _) = &**callee {
                    if cname.chars().next().map_or(false, |c| c.is_uppercase()) {
                        let dest = self.fresh_tmp();
                        let mut fields = Vec::new();
                        for (idx, arg) in args.iter().enumerate() {
                            let fname = arg.name.clone().unwrap_or_else(|| format!("field_{}", idx));
                            let fval = self.lower_expr(&arg.value, ops);
                            fields.push((fname, fval));
                        }
                        ops.push(IROp::ConstructStruct {
                            dest: dest.clone(),
                            struct_name: cname.clone(),
                            fields,
                        });
                        return dest;
                    }
                }

                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.lower_expr(&a.value, ops));
                }
                let callee_name = match &**callee {
                    Expr::Ident(n, _) => n.clone(),
                    _ => self.lower_expr(callee, ops),
                };
                let dest = self.fresh_tmp();
                ops.push(IROp::Call {
                    dest: Some(dest.clone()),
                    callee: callee_name,
                    args: arg_vals,
                });
                dest
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                let rec_val = self.lower_expr(receiver, ops);
                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.lower_expr(&a.value, ops));
                }
                let dest = self.fresh_tmp();
                ops.push(IROp::MethodCall {
                    dest: Some(dest.clone()),
                    receiver: rec_val,
                    method: method.clone(),
                    args: arg_vals,
                    receiver_borrowed: true,
                });
                dest
            }
            Expr::FieldAccess { receiver, field, .. } => {
                let rec_val = self.lower_expr(receiver, ops);
                let dest = self.fresh_tmp();
                ops.push(IROp::FieldAccess {
                    dest: dest.clone(),
                    receiver: rec_val,
                    field: field.clone(),
                });
                dest
            }
            Expr::InterpolatedString(parts, _) => {
                let mut formatted_args = Vec::new();
                let mut format_str = String::new();
                for p in parts {
                    match p {
                        InterpolatedExprPart::Lit(s) => format_str.push_str(s),
                        InterpolatedExprPart::Expr(e) => {
                            let val = self.lower_expr(e, ops);
                            format_str.push_str("{}");
                            formatted_args.push(val);
                        }
                    }
                }
                let dest = self.fresh_tmp();
                let mut all_args = vec![format!("\"{}\"", format_str)];
                all_args.extend(formatted_args);
                ops.push(IROp::Call {
                    dest: Some(dest.clone()),
                    callee: "format_teks".to_string(),
                    args: all_args,
                });
                dest
            }
            Expr::ErrorPropagate { inner, .. } => {
                let in_val = self.lower_expr(inner, ops);
                let dest = self.fresh_tmp();
                ops.push(IROp::ErrorPropagate {
                    dest: dest.clone(),
                    source: in_val,
                });
                dest
            }
            Expr::Catch { inner, err_var, handler, .. } => {
                let _in_val = self.lower_expr(inner, ops);
                let dest = self.fresh_tmp();
                let handler_lbl = format!("catch_handler_{}", self.tmp_counter);
                let end_lbl = format!("catch_end_{}", self.tmp_counter);
                self.tmp_counter += 1;

                ops.push(IROp::CatchError {
                    dest: dest.clone(),
                    err_var: err_var.clone(),
                    handler_label: handler_lbl.clone(),
                });
                ops.push(IROp::Goto(end_lbl.clone()));

                ops.push(IROp::Label(handler_lbl));
                for s in &handler.stmts {
                    self.lower_stmt(s, ops);
                }
                ops.push(IROp::Label(end_lbl));
                dest
            }
            Expr::Parallel { body, .. } => {
                let task_lbl = format!("task_coroutine_{}", self.tmp_counter);
                let dest = self.fresh_tmp();
                self.tmp_counter += 1;

                ops.push(IROp::SpawnTask {
                    dest: dest.clone(),
                    task_label: task_lbl.clone(),
                });

                let mut task_ops = Vec::new();
                for s in &body.stmts {
                    self.lower_stmt(s, &mut task_ops);
                }
                if let Some(te) = &body.expr {
                    let tv = self.lower_expr(te, &mut task_ops);
                    task_ops.push(IROp::Return { value: Some(tv) });
                }
                // (Task coroutine block instructions are associated with task_label)
                dest
            }
            Expr::Await { task, .. } => {
                let t_val = self.lower_expr(task, ops);
                let dest = self.fresh_tmp();
                ops.push(IROp::AwaitTask {
                    dest: dest.clone(),
                    task_var: t_val,
                });
                dest
            }
            Expr::Comptime { body, .. } => {
                let mut b_ops = Vec::new();
                for s in &body.stmts {
                    self.lower_stmt(s, &mut b_ops);
                }
                let mut val = "()".to_string();
                if let Some(e) = &body.expr {
                    val = self.lower_expr(e, &mut b_ops);
                }
                ops.extend(b_ops);
                val
            }
            Expr::Closure { body, .. } => {
                self.lower_expr(body, ops)
            }
            Expr::Block(b) => {
                for s in &b.stmts {
                    self.lower_stmt(s, ops);
                }
                if let Some(e) = &b.expr {
                    self.lower_expr(e, ops)
                } else {
                    "()".to_string()
                }
            }
        }
    }
}
