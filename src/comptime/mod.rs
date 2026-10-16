use crate::ast::*;
use crate::diag::Result;
use crate::eval::{ControlFlow, Environment, Evaluator, Value};

pub struct ComptimeEvaluator {
    evaluator: Evaluator,
}

impl ComptimeEvaluator {
    pub fn new() -> Self {
        ComptimeEvaluator {
            evaluator: Evaluator::new(),
        }
    }

    pub fn fold_program(&mut self, program: &mut Program) -> Result<()> {
        for decl in &mut program.decls {
            match decl {
                Decl::Fn(f) => {
                    self.fold_block(&mut f.body)?;
                }
                Decl::Impl(i) => {
                    for m in &mut i.methods {
                        self.fold_block(&mut m.body)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn fold_block(&mut self, block: &mut Block) -> Result<()> {
        for stmt in &mut block.stmts {
            self.fold_stmt(stmt)?;
        }
        if let Some(expr) = &mut block.expr {
            self.fold_expr(expr)?;
        }
        Ok(())
    }

    fn fold_stmt(&mut self, stmt: &mut Stmt) -> Result<()> {
        match stmt {
            Stmt::VarDecl { init, .. } => self.fold_expr(init),
            Stmt::Assign { value, .. } => self.fold_expr(value),
            Stmt::If { cond, then_branch, else_branch, .. } => {
                self.fold_expr(cond)?;
                self.fold_block(then_branch)?;
                if let Some(eb) = else_branch {
                    self.fold_block(eb)?;
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                self.fold_expr(cond)?;
                self.fold_block(body)
            }
            Stmt::ForIn { iterable, body, .. } => {
                self.fold_expr(iterable)?;
                self.fold_block(body)
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.fold_expr(v)?;
                }
                Ok(())
            }
            Stmt::Fail { value, .. } => self.fold_expr(value),
            Stmt::Expr(e) => self.fold_expr(e),
        }
    }

    fn fold_expr(&mut self, expr: &mut Expr) -> Result<()> {
        match expr {
            Expr::Comptime { body, span } => {
                let mut env = Environment::new();
                let val = match self.evaluator.eval_block(body, &mut env)? {
                    ControlFlow::Next(v) | ControlFlow::Return(v) => v,
                    ControlFlow::Fail(e) => Value::String(format!("Comptime Error: {}", e)),
                };
                let replacement_lit = match val {
                    Value::Int(n) => Literal::Int(n),
                    Value::Float(f) => Literal::Float(f),
                    Value::String(s) => Literal::String(s),
                    Value::Bool(b) => Literal::Bool(b),
                    Value::Unit => Literal::Unit,
                    Value::Struct { .. } | Value::ErrorUnion(_) | Value::Task(_) | Value::List(_) | Value::Map(_) | Value::Closure(_) => Literal::Unit,
                };
                *expr = Expr::Lit(replacement_lit, *span);
            }
            Expr::Binary(left, _, right, _) => {
                self.fold_expr(left)?;
                self.fold_expr(right)?;
            }
            Expr::Unary(_, operand, _) => {
                self.fold_expr(operand)?;
            }
            Expr::Call { callee, args, .. } => {
                self.fold_expr(callee)?;
                for a in args {
                    self.fold_expr(&mut a.value)?;
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.fold_expr(receiver)?;
                for a in args {
                    self.fold_expr(&mut a.value)?;
                }
            }
            Expr::FieldAccess { receiver, .. } => {
                self.fold_expr(receiver)?;
            }
            Expr::InterpolatedString(parts, _) => {
                for p in parts {
                    if let InterpolatedExprPart::Expr(e) = p {
                        self.fold_expr(e)?;
                    }
                }
            }
            Expr::ErrorPropagate { inner, .. } => self.fold_expr(inner)?,
            Expr::Catch { inner, handler, .. } => {
                self.fold_expr(inner)?;
                self.fold_block(handler)?;
            }
            Expr::Parallel { body, .. } => self.fold_block(body)?,
            Expr::Await { task, .. } => self.fold_expr(task)?,
            Expr::Closure { body, .. } => self.fold_expr(body)?,
            Expr::Block(b) => self.fold_block(b)?,
            Expr::Lit(_, _) | Expr::Ident(_, _) | Expr::SelfVar(_) => {}
        }
        Ok(())
    }
}
