use crate::ast::*;
use crate::diag::{Diagnostic, Result};
use crate::lexer::{InterpolatedPart, Lexer, Token, TokenKind};
use crate::span::Span;

pub struct Parser<'a> {
    tokens: Vec<Token>,
    cursor: usize,
    file_id: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest = 0,
    Catch = 2,      // tangkap
    Or = 3,         // ||
    And = 4,        // &&
    Equality = 5,   // == !=
    Comparison = 6, // < <= > >=
    Term = 7,       // + -
    Factor = 8,     // * / %
    Unary = 9,      // - !
    Postfix = 10,   // ? . ()
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, file_id: usize) -> Self {
        Parser {
            tokens,
            cursor: 0,
            file_id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut decls = Vec::new();
        self.skip_newlines();

        let start_span = self.current_span();

        while !self.is_eof() {
            let decl = self.parse_decl()?;
            decls.push(decl);
            self.skip_newlines();
        }

        let end_span = self.current_span();
        Ok(Program {
            decls,
            span: start_span.merge(end_span),
        })
    }

    // --- Declarations ---

    fn parse_decl(&mut self) -> Result<Decl> {
        self.skip_newlines();
        match self.peek_kind() {
            TokenKind::Impor => self.parse_import_decl().map(Decl::Import),
            TokenKind::Uji => self.parse_test_decl().map(Decl::Test),
            TokenKind::Struktur => self.parse_struct_decl().map(Decl::Struct),
            TokenKind::Sifat => self.parse_trait_decl().map(Decl::Trait),
            TokenKind::Terapkan => self.parse_impl_decl().map(Decl::Impl),
            TokenKind::Fungsi => self.parse_fn_decl(false).map(Decl::Fn),
            TokenKind::Comptime if self.peek_offset_kind(1) == TokenKind::Fungsi => {
                self.advance(); // consume comptime
                self.parse_fn_decl(true).map(Decl::Fn)
            }
            _ => {
                let tok = self.peek();
                Err(Diagnostic::error(format!(
                    "Diharapkan deklarasi ('impor', 'uji', 'struktur', 'sifat', 'terapkan', atau 'fungsi'), tetapi menemukan '{}'",
                    tok.kind
                ))
                .with_span(tok.span))
            }
        }
    }

    fn parse_import_decl(&mut self) -> Result<ImportDecl> {
        let start = self.expect(TokenKind::Impor)?.span;
        let (path, str_span) = match self.peek_kind() {
            TokenKind::StringLit(s) => {
                let s = s.clone();
                let span = self.advance().span;
                (s, span)
            }
            _ => {
                let tok = self.peek();
                return Err(Diagnostic::error("Diharapkan string jalur berkas setelah 'impor'")
                    .with_span(tok.span));
            }
        };
        self.consume_statement_separator();
        Ok(ImportDecl {
            path,
            span: start.merge(str_span),
        })
    }

    fn parse_test_decl(&mut self) -> Result<TestDecl> {
        let start = self.expect(TokenKind::Uji)?.span;
        self.skip_newlines();

        let name = match self.peek_kind() {
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                s
            }
            TokenKind::Ident(s) => {
                let s = s.clone();
                self.advance();
                s
            }
            _ => {
                let tok = self.peek();
                return Err(Diagnostic::error(format!(
                    "Diharapkan nama pengujian berupa string teks atau identifier setelah 'uji', tetapi menemukan '{}'",
                    tok.kind
                ))
                .with_span(tok.span));
            }
        };

        self.skip_newlines();
        let body = self.parse_block()?;
        let span = start.merge(body.span);

        Ok(TestDecl { name, body, span })
    }

    fn parse_struct_decl(&mut self) -> Result<StructDecl> {
        let start = self.expect(TokenKind::Struktur)?.span;
        let name_tok = self.expect_ident()?;
        let name = name_tok.0;

        self.skip_newlines();
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            let field_name_tok = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            let field_span = field_name_tok.1.merge(ty.span());
            fields.push(FieldDecl {
                name: field_name_tok.0,
                ty,
                span: field_span,
            });

            // Optional comma or newline
            if self.check(TokenKind::Comma) {
                self.advance();
            }
            self.skip_newlines();
        }

        let end = self.expect(TokenKind::RBrace)?.span;
        Ok(StructDecl {
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_trait_decl(&mut self) -> Result<TraitDecl> {
        let start = self.expect(TokenKind::Sifat)?.span;
        let (name, _) = self.expect_ident()?;

        self.skip_newlines();
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            self.expect(TokenKind::Fungsi)?;
            let (method_name, m_start) = self.expect_ident()?;
            self.expect(TokenKind::LParen)?;

            let mut params = Vec::new();
            while !self.check(TokenKind::RParen) && !self.is_eof() {
                if self.check(TokenKind::Ini) {
                    let ini_span = self.advance().span;
                    params.push(ParamDecl {
                        name: "ini".to_string(),
                        ty: None,
                        is_self: true,
                        span: ini_span,
                    });
                } else {
                    let (p_name, p_start) = self.expect_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type_expr()?;
                    let p_span = p_start.merge(ty.span());
                    params.push(ParamDecl {
                        name: p_name,
                        ty: Some(ty),
                        is_self: false,
                        span: p_span,
                    });
                }

                if self.check(TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(TokenKind::RParen)?;

            let ret_ty = if self.check(TokenKind::Arrow) {
                self.advance();
                Some(self.parse_type_expr()?)
            } else {
                None
            };

            let m_end = ret_ty
                .as_ref()
                .map(|t| t.span())
                .unwrap_or(self.prev_span());
            methods.push(TraitMethodDecl {
                name: method_name,
                params,
                ret_ty,
                span: m_start.merge(m_end),
            });

            self.skip_newlines();
        }

        let end = self.expect(TokenKind::RBrace)?.span;
        Ok(TraitDecl {
            name,
            methods,
            span: start.merge(end),
        })
    }

    fn parse_impl_decl(&mut self) -> Result<ImplDecl> {
        let start = self.expect(TokenKind::Terapkan)?.span;
        let (first_name, _) = self.expect_ident()?;

        let (trait_name, target_struct) = if self.check(TokenKind::Untuk) {
            self.advance();
            let (struct_name, _) = self.expect_ident()?;
            (Some(first_name), struct_name)
        } else {
            (None, first_name)
        };

        self.skip_newlines();
        self.expect(TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            let m = self.parse_fn_decl(false)?;
            methods.push(m);
            self.skip_newlines();
        }

        let end = self.expect(TokenKind::RBrace)?.span;
        Ok(ImplDecl {
            trait_name,
            target_struct,
            methods,
            span: start.merge(end),
        })
    }

    fn parse_fn_decl(&mut self, is_comptime: bool) -> Result<FnDecl> {
        let start = self.expect(TokenKind::Fungsi)?.span;
        let (name, _) = self.expect_ident()?;

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_eof() {
            if self.check(TokenKind::Ini) {
                let ini_span = self.advance().span;
                params.push(ParamDecl {
                    name: "ini".to_string(),
                    ty: None,
                    is_self: true,
                    span: ini_span,
                });
            } else {
                let (p_name, p_start) = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type_expr()?;
                let p_span = p_start.merge(ty.span());
                params.push(ParamDecl {
                    name: p_name,
                    ty: Some(ty),
                    is_self: false,
                    span: p_span,
                });
            }

            if self.check(TokenKind::Comma) {
                self.advance();
            }
        }
        self.expect(TokenKind::RParen)?;

        let ret_ty = if self.check(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.skip_newlines();
        let body = self.parse_block()?;
        let end = body.span;

        Ok(FnDecl {
            name,
            params,
            ret_ty,
            body,
            is_comptime,
            span: start.merge(end),
        })
    }

    // --- Type Expressions ---

    pub fn parse_type_expr(&mut self) -> Result<TypeExpr> {
        let mut left = if self.check(TokenKind::LParen) {
            let start = self.advance().span;
            if self.check(TokenKind::RParen) {
                let end = self.advance().span;
                TypeExpr::Unit(start.merge(end))
            } else {
                let inner = self.parse_type_expr()?;
                self.expect(TokenKind::RParen)?;
                inner
            }
        } else {
            let (name, span) = self.expect_ident()?;
            if self.check(TokenKind::Lt) {
                self.advance();
                let inner = self.parse_type_expr()?;
                if name == "Tugas" {
                    let end = self.expect(TokenKind::Gt)?.span;
                    TypeExpr::Task(Box::new(inner), span.merge(end))
                } else if name == "Daftar" {
                    let end = self.expect(TokenKind::Gt)?.span;
                    TypeExpr::List(Box::new(inner), span.merge(end))
                } else if name == "Kamus" {
                    if self.check(TokenKind::Comma) {
                        self.advance();
                        let val_inner = self.parse_type_expr()?;
                        let end = self.expect(TokenKind::Gt)?.span;
                        TypeExpr::Map(Box::new(val_inner), span.merge(end))
                    } else {
                        let end = self.expect(TokenKind::Gt)?.span;
                        TypeExpr::Map(Box::new(inner), span.merge(end))
                    }
                } else {
                    let end = self.expect(TokenKind::Gt)?.span;
                    TypeExpr::Named(name, span.merge(end))
                }
            } else {
                TypeExpr::Named(name, span)
            }
        };

        // Check for ErrorUnion `T ! E`
        if self.check(TokenKind::Exclamation) {
            self.advance();
            let right = self.parse_type_expr()?;
            let span = left.span().merge(right.span());
            left = TypeExpr::ErrorUnion(Box::new(left), Box::new(right), span);
        }

        Ok(left)
    }

    // --- Blocks & Statements ---

    pub fn parse_block(&mut self) -> Result<Block> {
        let start = self.expect(TokenKind::LBrace)?.span;
        self.skip_newlines();

        let mut stmts = Vec::new();
        let mut final_expr = None;

        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            // Check if this could be an expression statement or a tail expression
            let stmt = self.parse_stmt()?;
            self.skip_newlines();

            // If the next token is RBrace and the stmt was an expression, it can serve as final_expr
            if self.check(TokenKind::RBrace) {
                if let Stmt::Expr(expr) = stmt {
                    final_expr = Some(Box::new(expr));
                    break;
                }
            }
            stmts.push(stmt);
        }

        let end = self.expect(TokenKind::RBrace)?.span;
        Ok(Block {
            stmts,
            expr: final_expr,
            span: start.merge(end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        self.skip_newlines();

        // 1. Walrus variable declaration: `ident := expr`
        if self.peek_is_ident() && self.peek_offset_kind(1) == TokenKind::Walrus {
            let (name, start) = self.expect_ident()?;
            self.advance(); // consume :=
            let init = self.parse_expr(Precedence::Lowest)?;
            let span = start.merge(init.span());
            self.consume_statement_separator();
            return Ok(Stmt::VarDecl {
                name,
                explicit_ty: None,
                init,
                span,
            });
        }

        // 2. Explicit type variable declaration: `ident: Type = expr`
        if self.peek_is_ident() && self.peek_offset_kind(1) == TokenKind::Colon {
            // Disambiguate with struct fields or labels: statements in blocks can declare variables
            let (name, start) = self.expect_ident()?;
            self.advance(); // consume :
            let ty = self.parse_type_expr()?;
            self.expect(TokenKind::Eq)?;
            let init = self.parse_expr(Precedence::Lowest)?;
            let span = start.merge(init.span());
            self.consume_statement_separator();
            return Ok(Stmt::VarDecl {
                name,
                explicit_ty: Some(ty),
                init,
                span,
            });
        }

        // 3. Control flow: jika (if)
        if self.check(TokenKind::Jika) {
            let start = self.advance().span;
            let cond = self.parse_expr(Precedence::Lowest)?;
            self.skip_newlines();
            let then_branch = self.parse_block()?;
            self.skip_newlines();

            let else_branch = if self.check(TokenKind::SelainItu) {
                self.advance();
                self.skip_newlines();
                if self.check(TokenKind::Jika) {
                    let nested_if = self.parse_stmt()?;
                    Some(Block {
                        span: nested_if.span(),
                        stmts: vec![nested_if],
                        expr: None,
                    })
                } else {
                    Some(self.parse_block()?)
                }
            } else {
                None
            };

            let end = else_branch
                .as_ref()
                .map(|b| b.span)
                .unwrap_or(then_branch.span);
            return Ok(Stmt::If {
                cond,
                then_branch,
                else_branch,
                span: start.merge(end),
            });
        }

        // 4. Control flow: selama (while)
        if self.check(TokenKind::Selama) {
            let start = self.advance().span;
            let cond = self.parse_expr(Precedence::Lowest)?;
            self.skip_newlines();
            let body = self.parse_block()?;
            let end = body.span;
            return Ok(Stmt::While {
                cond,
                body,
                span: start.merge(end),
            });
        }

        // 5. Control flow: untuk (for-in loop: untuk x di koleksi { ... })
        if self.check(TokenKind::Untuk) {
            let start = self.advance().span;
            let (var_name, _) = self.expect_ident()?;
            if self.check(TokenKind::Di) || self.check(TokenKind::Dalam) {
                self.advance();
            } else {
                return Err(Diagnostic::error("Diharapkan 'di' atau 'dalam' setelah nama variabel pada perulangan 'untuk'")
                    .with_span(self.peek().span));
            }
            let iterable = self.parse_expr(Precedence::Lowest)?;
            self.skip_newlines();
            let body = self.parse_block()?;
            let end = body.span;
            return Ok(Stmt::ForIn {
                var_name,
                iterable,
                body,
                span: start.merge(end),
            });
        }

        // 6. kembali (return)
        if self.check(TokenKind::Kembali) {
            let start = self.advance().span;
            let val = if self.check(TokenKind::Newline)
                || self.check(TokenKind::Semicolon)
                || self.check(TokenKind::RBrace)
                || self.is_eof()
            {
                None
            } else {
                Some(self.parse_expr(Precedence::Lowest)?)
            };
            let end = val.as_ref().map(|e| e.span()).unwrap_or(start);
            self.consume_statement_separator();
            return Ok(Stmt::Return {
                value: val,
                span: start.merge(end),
            });
        }

        // 6. gagal (fail / error throw)
        if self.check(TokenKind::Gagal) {
            let start = self.advance().span;
            let value = self.parse_expr(Precedence::Lowest)?;
            let span = start.merge(value.span());
            self.consume_statement_separator();
            return Ok(Stmt::Fail { value, span });
        }

        // 7. General Expression or Assignment
        let expr = self.parse_expr(Precedence::Lowest)?;

        if self.check(TokenKind::Eq) {
            self.advance();
            let value = self.parse_expr(Precedence::Lowest)?;
            let span = expr.span().merge(value.span());
            self.consume_statement_separator();
            return Ok(Stmt::Assign {
                target: expr,
                value,
                span,
            });
        }

        self.consume_statement_separator();
        Ok(Stmt::Expr(expr))
    }

    // --- Expressions (Pratt Parser) ---

    pub fn parse_expr(&mut self, prec: Precedence) -> Result<Expr> {
        let mut left = self.parse_prefix()?;

        while !self.is_eof() {
            let next_prec = self.current_precedence();
            if prec >= next_prec {
                break;
            }
            left = self.parse_infix(left)?;
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<Expr> {
        let tok = self.peek();
        match &tok.kind {
            TokenKind::IntLit(n) => {
                let n = *n;
                let span = self.advance().span;
                Ok(Expr::Lit(Literal::Int(n), span))
            }
            TokenKind::FloatLit(n) => {
                let n = *n;
                let span = self.advance().span;
                Ok(Expr::Lit(Literal::Float(n), span))
            }
            TokenKind::StringLit(s) => {
                let s = s.clone();
                let span = self.advance().span;
                Ok(Expr::Lit(Literal::String(s), span))
            }
            TokenKind::InterpolatedString(parts) => {
                let parts = parts.clone();
                let span = self.advance().span;
                let mut expr_parts = Vec::new();
                for part in parts {
                    match part {
                        InterpolatedPart::Literal(lit) => {
                            expr_parts.push(InterpolatedExprPart::Lit(lit));
                        }
                        InterpolatedPart::Expression(code) => {
                            let mut inner_lexer = Lexer::new(&code, self.file_id);
                            let inner_tokens = inner_lexer.tokenize()?;
                            let mut inner_parser = Parser::new(inner_tokens, self.file_id);
                            let parsed_expr = inner_parser.parse_expr(Precedence::Lowest)?;
                            expr_parts.push(InterpolatedExprPart::Expr(Box::new(parsed_expr)));
                        }
                    }
                }
                Ok(Expr::InterpolatedString(expr_parts, span))
            }
            TokenKind::Benar => {
                let span = self.advance().span;
                Ok(Expr::Lit(Literal::Bool(true), span))
            }
            TokenKind::Salah => {
                let span = self.advance().span;
                Ok(Expr::Lit(Literal::Bool(false), span))
            }
            TokenKind::Ini => {
                let span = self.advance().span;
                Ok(Expr::SelfVar(span))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = self.advance().span;
                Ok(Expr::Ident(name, span))
            }
            TokenKind::Minus => {
                let span = self.advance().span;
                let operand = self.parse_expr(Precedence::Unary)?;
                let end = operand.span();
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(operand), span.merge(end)))
            }
            TokenKind::Exclamation => {
                let span = self.advance().span;
                let operand = self.parse_expr(Precedence::Unary)?;
                let end = operand.span();
                Ok(Expr::Unary(UnaryOp::Not, Box::new(operand), span.merge(end)))
            }
            TokenKind::LParen => {
                let start = self.advance().span;
                if self.check(TokenKind::RParen) {
                    let end = self.advance().span;
                    Ok(Expr::Lit(Literal::Unit, start.merge(end)))
                } else {
                    let inner = self.parse_expr(Precedence::Lowest)?;
                    self.expect(TokenKind::RParen)?;
                    Ok(inner)
                }
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Expr::Block(block))
            }
            TokenKind::JalanParalel => {
                let start = self.advance().span;
                self.skip_newlines();
                let body = self.parse_block()?;
                let end = body.span;
                Ok(Expr::Parallel {
                    body,
                    span: start.merge(end),
                })
            }
            TokenKind::Tunggu => {
                let start = self.advance().span;
                self.expect(TokenKind::LParen)?;
                let task = self.parse_expr(Precedence::Lowest)?;
                let end = self.expect(TokenKind::RParen)?.span;
                Ok(Expr::Await {
                    task: Box::new(task),
                    span: start.merge(end),
                })
            }
            TokenKind::Comptime => {
                let start = self.advance().span;
                self.skip_newlines();
                let body = self.parse_block()?;
                let end = body.span;
                Ok(Expr::Comptime {
                    body,
                    span: start.merge(end),
                })
            }
            TokenKind::Pipe => {
                let start = self.advance().span;
                let mut params = Vec::new();
                if !self.check(TokenKind::Pipe) {
                    loop {
                        let (param_name, p_span) = self.expect_ident()?;
                        let param_ty = if self.check(TokenKind::Colon) {
                            self.advance();
                            Some(self.parse_type_expr()?)
                        } else {
                            None
                        };
                        params.push(ParamDecl {
                            name: param_name,
                            ty: param_ty,
                            is_self: false,
                            span: p_span,
                        });
                        if self.check(TokenKind::Comma) {
                            self.advance();
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenKind::Pipe)?;
                let ret_ty = if self.check(TokenKind::Arrow) {
                    self.advance();
                    Some(self.parse_type_expr()?)
                } else {
                    None
                };
                let body = self.parse_expr(Precedence::Lowest)?;
                let end = body.span();
                Ok(Expr::Closure {
                    params,
                    ret_ty,
                    body: Box::new(body),
                    span: start.merge(end),
                })
            }
            TokenKind::OrOr => {
                let start = self.advance().span;
                let ret_ty = if self.check(TokenKind::Arrow) {
                    self.advance();
                    Some(self.parse_type_expr()?)
                } else {
                    None
                };
                let body = self.parse_expr(Precedence::Lowest)?;
                let end = body.span();
                Ok(Expr::Closure {
                    params: Vec::new(),
                    ret_ty,
                    body: Box::new(body),
                    span: start.merge(end),
                })
            }
            _ => Err(Diagnostic::error(format!(
                "Ekspresi tidak terduga atau tidak valid dimulai dengan: '{}'",
                tok.kind
            ))
            .with_span(tok.span)),
        }
    }

    fn current_precedence(&self) -> Precedence {
        match self.peek_kind() {
            TokenKind::Tangkap => Precedence::Catch,
            TokenKind::OrOr => Precedence::Or,
            TokenKind::AndAnd => Precedence::And,
            TokenKind::EqEq | TokenKind::NotEq => Precedence::Equality,
            TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => {
                Precedence::Comparison
            }
            TokenKind::Plus | TokenKind::Minus => Precedence::Term,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Precedence::Factor,
            TokenKind::Question | TokenKind::Dot | TokenKind::LParen => Precedence::Postfix,
            _ => Precedence::Lowest,
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr> {
        let left_span = left.span();
        let tok = self.peek();
        match tok.kind {
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::EqEq
            | TokenKind::NotEq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::Gt
            | TokenKind::GtEq
            | TokenKind::AndAnd
            | TokenKind::OrOr => {
                let op = match tok.kind {
                    TokenKind::Plus => BinaryOp::Add,
                    TokenKind::Minus => BinaryOp::Sub,
                    TokenKind::Star => BinaryOp::Mul,
                    TokenKind::Slash => BinaryOp::Div,
                    TokenKind::Percent => BinaryOp::Mod,
                    TokenKind::EqEq => BinaryOp::Eq,
                    TokenKind::NotEq => BinaryOp::NotEq,
                    TokenKind::Lt => BinaryOp::Lt,
                    TokenKind::LtEq => BinaryOp::LtEq,
                    TokenKind::Gt => BinaryOp::Gt,
                    TokenKind::GtEq => BinaryOp::GtEq,
                    TokenKind::AndAnd => BinaryOp::And,
                    TokenKind::OrOr => BinaryOp::Or,
                    _ => unreachable!(),
                };
                let prec = self.current_precedence();
                self.advance();
                let right = self.parse_expr(prec)?;
                let span = left_span.merge(right.span());
                Ok(Expr::Binary(Box::new(left), op, Box::new(right), span))
            }
            TokenKind::Question => {
                let q_span = self.advance().span;

                // Check for inline catch block: `? tangkap err_var { ... }`
                if self.check(TokenKind::Tangkap) {
                    self.advance(); // consume tangkap
                    let (err_var, _) = self.expect_ident()?;
                    self.skip_newlines();
                    let handler = self.parse_block()?;
                    let span = left_span.merge(handler.span);
                    Ok(Expr::Catch {
                        inner: Box::new(left),
                        err_var,
                        handler,
                        span,
                    })
                } else {
                    let propagated = Expr::ErrorPropagate {
                        span: left_span.merge(q_span),
                        inner: Box::new(left),
                    };
                    Ok(propagated)
                }
            }
            TokenKind::Tangkap => {
                self.advance(); // consume tangkap
                let (err_var, _) = self.expect_ident()?;
                self.skip_newlines();
                let handler = self.parse_block()?;
                let span = left_span.merge(handler.span);
                Ok(Expr::Catch {
                    inner: Box::new(left),
                    err_var,
                    handler,
                    span,
                })
            }
            TokenKind::Dot => {
                self.advance(); // consume .
                let (name, name_span) = self.expect_ident()?;

                // Check if this is a method call or a field access
                if self.check(TokenKind::LParen) {
                    self.advance(); // consume (
                    let args = self.parse_call_args()?;
                    let end = self.expect(TokenKind::RParen)?.span;
                    Ok(Expr::MethodCall {
                        receiver: Box::new(left),
                        method: name,
                        args,
                        span: left_span.merge(end),
                    })
                } else {
                    Ok(Expr::FieldAccess {
                        receiver: Box::new(left),
                        field: name,
                        span: left_span.merge(name_span),
                    })
                }
            }
            TokenKind::LParen => {
                self.advance(); // consume (
                let args = self.parse_call_args()?;
                let end = self.expect(TokenKind::RParen)?.span;
                Ok(Expr::Call {
                    callee: Box::new(left),
                    args,
                    span: left_span.merge(end),
                })
            }
            _ => Err(Diagnostic::error(format!(
                "Operator infix tidak terduga: '{}'",
                tok.kind
            ))
            .with_span(tok.span)),
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<CallArg>> {
        let mut args = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_eof() {
            // Check for named argument: `id: expr`
            let arg = if self.peek_is_ident() && self.peek_offset_kind(1) == TokenKind::Colon {
                let (arg_name, start) = self.expect_ident()?;
                self.advance(); // consume :
                let val = self.parse_expr(Precedence::Lowest)?;
                let span = start.merge(val.span());
                CallArg {
                    name: Some(arg_name),
                    value: val,
                    span,
                }
            } else {
                let val = self.parse_expr(Precedence::Lowest)?;
                let span = val.span();
                CallArg {
                    name: None,
                    value: val,
                    span,
                }
            };
            args.push(arg);

            if self.check(TokenKind::Comma) {
                self.advance();
            }
        }
        Ok(args)
    }

    // --- Helpers ---

    fn peek(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            self.tokens.last().unwrap()
        }
    }

    fn peek_kind(&self) -> TokenKind {
        self.peek().kind.clone()
    }

    fn peek_offset_kind(&self, offset: usize) -> TokenKind {
        let idx = self.cursor + offset;
        if idx < self.tokens.len() {
            self.tokens[idx].kind.clone()
        } else {
            TokenKind::Eof
        }
    }

    fn peek_is_ident(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Ident(_))
    }

    fn advance(&mut self) -> &Token {
        if self.cursor < self.tokens.len() {
            let tok = &self.tokens[self.cursor];
            self.cursor += 1;
            tok
        } else {
            self.tokens.last().unwrap()
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek_kind() == kind
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token> {
        let cur = self.peek().clone();
        if cur.kind == kind {
            Ok(self.advance())
        } else {
            Err(Diagnostic::error(format!(
                "Diharapkan token '{}', tetapi menemukan '{}'",
                kind, cur.kind
            ))
            .with_span(cur.span))
        }
    }

    fn expect_ident(&mut self) -> Result<(String, Span)> {
        let cur = self.peek().clone();
        if let TokenKind::Ident(name) = cur.kind {
            self.advance();
            Ok((name, cur.span))
        } else {
            Err(Diagnostic::error(format!(
                "Diharapkan identifier, tetapi menemukan '{}'",
                cur.kind
            ))
            .with_span(cur.span))
        }
    }

    fn skip_newlines(&mut self) {
        while self.check(TokenKind::Newline) || self.check(TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn consume_statement_separator(&mut self) {
        if self.check(TokenKind::Newline) || self.check(TokenKind::Semicolon) {
            self.advance();
        }
        self.skip_newlines();
    }

    fn is_eof(&self) -> bool {
        self.peek_kind() == TokenKind::Eof
    }

    fn current_span(&self) -> Span {
        self.peek().span
    }

    fn prev_span(&self) -> Span {
        if self.cursor > 0 {
            self.tokens[self.cursor - 1].span
        } else {
            Span::DUMMY
        }
    }
}
