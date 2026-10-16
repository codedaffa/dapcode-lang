use crate::ast::*;
use crate::span::Span;
use crate::types::Type;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageKind {
    Borrow,    // &T
    MutBorrow, // &mut T
    Move,      // T (transferred/consumed)
}

impl fmt::Display for UsageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsageKind::Borrow => write!(f, "& (borrow)"),
            UsageKind::MutBorrow => write!(f, "&mut (mut-borrow)"),
            UsageKind::Move => write!(f, "move (konsumsi)"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DropAction {
    pub var_name: String,
    pub var_type: Option<Type>,
    pub span: Span,
    pub reuse_candidate: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FunctionOwnershipAnalysis {
    pub fn_name: String,
    // Variable -> Inferred usage mode
    pub var_usages: HashMap<String, UsageKind>,
    // Statement index -> variables to drop immediately after this statement
    pub drop_points: HashMap<usize, Vec<DropAction>>,
    // Drop actions at function exit (for unconsumed parameters or remaining locals)
    pub exit_drops: Vec<DropAction>,
    // In-place Perceus reuse opportunities: (dropped_var, new_allocated_var)
    pub reuse_opportunities: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct OwnershipReport {
    pub functions: HashMap<String, FunctionOwnershipAnalysis>,
}

impl fmt::Display for OwnershipReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== LAPORAN AUTO-OWNERSHIP & PERCEUS (COMPILE-TIME) ===")?;
        for (fn_name, fa) in &self.functions {
            writeln!(f, "\nFungsi: {}", fn_name)?;
            writeln!(f, "  Inferensi Pinjaman (Borrow Inference):")?;
            for (var, usage) in &fa.var_usages {
                writeln!(f, "    - variable '{}' -> {}", var, usage)?;
            }
            writeln!(f, "  Injeksi Drop Deterministik (Last-Use Drops):")?;
            for (stmt_idx, drops) in &fa.drop_points {
                for d in drops {
                    if let Some(reuse) = &d.reuse_candidate {
                        writeln!(
                            f,
                            "    - setelah baris/pernyataan #{} -> drop('{}') [Perceus Reuse Buffer -> '{}']",
                            stmt_idx + 1,
                            d.var_name,
                            reuse
                        )?;
                    } else {
                        writeln!(
                            f,
                            "    - setelah baris/pernyataan #{} -> drop('{}')",
                            stmt_idx + 1,
                            d.var_name
                        )?;
                    }
                }
            }
            if !fa.exit_drops.is_empty() {
                writeln!(f, "  Drop saat keluar fungsi (Scope Exit):")?;
                for d in &fa.exit_drops {
                    writeln!(f, "    - drop('{}')", d.var_name)?;
                }
            }
        }
        writeln!(f, "\n=======================================================")?;
        Ok(())
    }
}

pub struct OwnershipAnalyzer {
    report: OwnershipReport,
}

impl OwnershipAnalyzer {
    pub fn new() -> Self {
        OwnershipAnalyzer {
            report: OwnershipReport::default(),
        }
    }

    pub fn analyze_program(mut self, program: &Program) -> OwnershipReport {
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    let fa = self.analyze_fn(&f.name, &f.params, &f.body);
                    self.report.functions.insert(f.name.clone(), fa);
                }
                Decl::Impl(i) => {
                    for m in &i.methods {
                        let full_name = format!("{}::{}", i.target_struct, m.name);
                        let fa = self.analyze_fn(&full_name, &m.params, &m.body);
                        self.report.functions.insert(full_name, fa);
                    }
                }
                Decl::Test(t) => {
                    let fa = self.analyze_fn(&format!("uji::{}", t.name), &[], &t.body);
                    self.report.functions.insert(format!("uji::{}", t.name), fa);
                }
                _ => {}
            }
        }
        self.report
    }

    fn analyze_fn(&self, name: &str, params: &[ParamDecl], body: &Block) -> FunctionOwnershipAnalysis {
        let mut fa = FunctionOwnershipAnalysis {
            fn_name: name.to_string(),
            var_usages: HashMap::new(),
            drop_points: HashMap::new(),
            exit_drops: Vec::new(),
            reuse_opportunities: Vec::new(),
        };

        // All local declared variables & parameters
        let mut declared_vars = HashSet::new();
        for p in params {
            declared_vars.insert(p.name.clone());
            // By default, parameters that are only read are borrowed
            fa.var_usages.insert(p.name.clone(), UsageKind::Borrow);
        }

        // Collect variable mentions and their last occurrences in the statement list
        // stmt_idx -> set of variables read/written
        let mut var_last_stmt = HashMap::new();
        let mut var_mutated = HashSet::new();
        let mut var_allocated = Vec::new(); // (stmt_idx, var_name)

        for (stmt_idx, stmt) in body.stmts.iter().enumerate() {
            let (vars_used, vars_modified, new_var) = self.collect_stmt_vars(stmt);
            if let Some(new_v) = new_var {
                declared_vars.insert(new_v.clone());
                var_allocated.push((stmt_idx, new_v.clone()));
                fa.var_usages.insert(new_v, UsageKind::Borrow);
            }
            for v in vars_modified {
                var_mutated.insert(v);
            }
            for v in vars_used {
                var_last_stmt.insert(v, stmt_idx);
            }
        }

        // If block has a trailing expr, record occurrences in it
        if let Some(trailing) = &body.expr {
            let vars_used = self.collect_expr_vars(trailing);
            for v in vars_used {
                var_last_stmt.insert(v, body.stmts.len());
            }
        }

        // Determine UsageKind: Borrow, MutBorrow, or Move
        for var in &declared_vars {
            if var_mutated.contains(var) {
                fa.var_usages.insert(var.clone(), UsageKind::MutBorrow);
            } else if let Some(last_idx) = var_last_stmt.get(var) {
                // If the variable is used at the very end of the function (returned), it's moved
                if *last_idx == body.stmts.len() && body.expr.is_some() {
                    fa.var_usages.insert(var.clone(), UsageKind::Move);
                } else {
                    fa.var_usages.insert(var.clone(), UsageKind::Borrow);
                }
            }
        }

        // Inject deterministic drops at last use
        let mut dropped_vars = HashSet::new();
        for (var, last_idx) in &var_last_stmt {
            if declared_vars.contains(var) && *last_idx < body.stmts.len() {
                // Check if this variable is a candidate for Perceus buffer reuse
                let reuse_candidate = var_allocated.iter().find_map(|(idx, new_v)| {
                    if *idx > *last_idx && new_v != var {
                        Some(new_v.clone())
                    } else {
                        None
                    }
                });

                if let Some(ref r) = reuse_candidate {
                    fa.reuse_opportunities.push((var.clone(), r.clone()));
                }

                fa.drop_points.entry(*last_idx).or_default().push(DropAction {
                    var_name: var.clone(),
                    var_type: None,
                    span: body.stmts[*last_idx].span(),
                    reuse_candidate,
                });
                dropped_vars.insert(var.clone());
            }
        }

        // Variables declared but never dropped (e.g. unused parameters or locals) are dropped at exit
        for var in &declared_vars {
            if !dropped_vars.contains(var) {
                // If it's moved into return expression, don't drop at exit
                let is_moved_return = body.expr.as_ref().map_or(false, |e| {
                    if let Expr::Ident(name, _) = e.as_ref() {
                        name == var
                    } else {
                        false
                    }
                });
                if !is_moved_return {
                    fa.exit_drops.push(DropAction {
                        var_name: var.clone(),
                        var_type: None,
                        span: body.span,
                        reuse_candidate: None,
                    });
                }
            }
        }

        fa
    }

    fn collect_stmt_vars(&self, stmt: &Stmt) -> (HashSet<String>, HashSet<String>, Option<String>) {
        let mut used = HashSet::new();
        let mut modified = HashSet::new();
        let mut declared = None;

        match stmt {
            Stmt::VarDecl { name, init, .. } => {
                declared = Some(name.clone());
                for v in self.collect_expr_vars(init) {
                    used.insert(v);
                }
            }
            Stmt::Assign { target, value, .. } => {
                if let Expr::Ident(name, _) = target {
                    modified.insert(name.clone());
                } else {
                    for v in self.collect_expr_vars(target) {
                        used.insert(v);
                    }
                }
                for v in self.collect_expr_vars(value) {
                    used.insert(v);
                }
            }
            Stmt::If { cond, then_branch, else_branch, .. } => {
                for v in self.collect_expr_vars(cond) {
                    used.insert(v);
                }
                for s in &then_branch.stmts {
                    let (u, m, _) = self.collect_stmt_vars(s);
                    used.extend(u);
                    modified.extend(m);
                }
                if let Some(eb) = else_branch {
                    for s in &eb.stmts {
                        let (u, m, _) = self.collect_stmt_vars(s);
                        used.extend(u);
                        modified.extend(m);
                    }
                }
            }
            Stmt::While { cond, body, .. } => {
                for v in self.collect_expr_vars(cond) {
                    used.insert(v);
                }
                for s in &body.stmts {
                    let (u, m, _) = self.collect_stmt_vars(s);
                    used.extend(u);
                    modified.extend(m);
                }
            }
            Stmt::ForIn { var_name, iterable, body, .. } => {
                for v in self.collect_expr_vars(iterable) {
                    used.insert(v);
                }
                declared = Some(var_name.clone());
                for s in &body.stmts {
                    let (u, m, _) = self.collect_stmt_vars(s);
                    used.extend(u);
                    modified.extend(m);
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    for uv in self.collect_expr_vars(v) {
                        used.insert(uv);
                    }
                }
            }
            Stmt::Fail { value, .. } => {
                for uv in self.collect_expr_vars(value) {
                    used.insert(uv);
                }
            }
            Stmt::Expr(e) => {
                for uv in self.collect_expr_vars(e) {
                    used.insert(uv);
                }
            }
        }

        (used, modified, declared)
    }

    fn collect_expr_vars(&self, expr: &Expr) -> HashSet<String> {
        let mut vars = HashSet::new();
        match expr {
            Expr::Ident(name, _) => {
                vars.insert(name.clone());
            }
            Expr::SelfVar(_) => {
                vars.insert("ini".to_string());
            }
            Expr::Binary(left, _, right, _) => {
                vars.extend(self.collect_expr_vars(left));
                vars.extend(self.collect_expr_vars(right));
            }
            Expr::Unary(_, operand, _) => {
                vars.extend(self.collect_expr_vars(operand));
            }
            Expr::Call { callee, args, .. } => {
                vars.extend(self.collect_expr_vars(callee));
                for arg in args {
                    vars.extend(self.collect_expr_vars(&arg.value));
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                vars.extend(self.collect_expr_vars(receiver));
                for arg in args {
                    vars.extend(self.collect_expr_vars(&arg.value));
                }
            }
            Expr::FieldAccess { receiver, .. } => {
                vars.extend(self.collect_expr_vars(receiver));
            }
            Expr::InterpolatedString(parts, _) => {
                for part in parts {
                    if let InterpolatedExprPart::Expr(e) = part {
                        vars.extend(self.collect_expr_vars(e));
                    }
                }
            }
            Expr::ErrorPropagate { inner, .. } => {
                vars.extend(self.collect_expr_vars(inner));
            }
            Expr::Catch { inner, err_var, handler, .. } => {
                vars.extend(self.collect_expr_vars(inner));
                for s in &handler.stmts {
                    let (u, _, _) = self.collect_stmt_vars(s);
                    for uv in u {
                        if &uv != err_var {
                            vars.insert(uv);
                        }
                    }
                }
            }
            Expr::Parallel { body, .. } => {
                for s in &body.stmts {
                    let (u, _, _) = self.collect_stmt_vars(s);
                    vars.extend(u);
                }
                if let Some(e) = &body.expr {
                    vars.extend(self.collect_expr_vars(e));
                }
            }
            Expr::Await { task, .. } => {
                vars.extend(self.collect_expr_vars(task));
            }
            Expr::Comptime { body, .. } => {
                for s in &body.stmts {
                    let (u, _, _) = self.collect_stmt_vars(s);
                    vars.extend(u);
                }
                if let Some(e) = &body.expr {
                    vars.extend(self.collect_expr_vars(e));
                }
            }
            Expr::Block(b) => {
                for s in &b.stmts {
                    let (u, _, _) = self.collect_stmt_vars(s);
                    vars.extend(u);
                }
                if let Some(e) = &b.expr {
                    vars.extend(self.collect_expr_vars(e));
                }
            }
            Expr::Closure { body, .. } => {
                vars.extend(self.collect_expr_vars(body));
            }
            Expr::Lit(_, _) => {}
        }
        vars
    }
}
