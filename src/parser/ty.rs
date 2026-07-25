//! Stage 6.12 (TD-022): Type parsing sub-module.
//!
//! Per 02-grammar.md §3.3 (Type). Extracted from `parser.rs`
//! per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `parse_ty` (type expression parsing — primitive / ref / ptr / tuple /
//!   array / slice / fn-ptr / trait-object / impl-trait / path)
//!
//! Also includes the file-level `ty_to_path` helper used by `parse_impl`.

use crate::ast;
use crate::ast::*;
use crate::lexer::token::*;

use super::parser::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_ty(&mut self) -> Ty {
        let span = self.current_span();
        match self.peek().clone() {
            // Basic primitive types
            TokenKind::Ident(sym) => {
                let name = self.interner.resolve(&sym).to_string();
                match name.as_str() {
                    "bool" => {
                        self.bump();
                        return Ty::Bool(span);
                    }
                    "char" => {
                        self.bump();
                        return Ty::Char(span);
                    }
                    "i8" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I8, span);
                    }
                    "i16" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I16, span);
                    }
                    "i32" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I32, span);
                    }
                    "i64" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I64, span);
                    }
                    "i128" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I128, span);
                    }
                    "isize" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::Isize, span);
                    }
                    "u8" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U8, span);
                    }
                    "u16" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U16, span);
                    }
                    "u32" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U32, span);
                    }
                    "u64" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U64, span);
                    }
                    "u128" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U128, span);
                    }
                    "usize" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::Usize, span);
                    }
                    "f32" => {
                        self.bump();
                        return Ty::Float(ast::FloatTy::F32, span);
                    }
                    "f64" => {
                        self.bump();
                        return Ty::Float(ast::FloatTy::F64, span);
                    }
                    _ => {} // Fall through to path type
                }
                let path = self.parse_path();
                Ty::Path(QSelf::default(), path, span)
            }
            TokenKind::Not => {
                self.bump();
                Ty::Never(span)
            }
            TokenKind::LParen => {
                self.bump();
                let mut tys = Vec::new();
                if *self.peek() != TokenKind::RParen {
                    tys.push(self.parse_ty());
                    if *self.peek() == TokenKind::Comma {
                        self.bump();
                        if *self.peek() != TokenKind::RParen {
                            tys.push(self.parse_ty());
                            while self.eat(&TokenKind::Comma) {
                                if *self.peek() == TokenKind::RParen {
                                    break;
                                }
                                tys.push(self.parse_ty());
                            }
                        }
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                Ty::Tuple(tys, span)
            }
            TokenKind::LBracket => {
                self.bump();
                let ty = self.parse_ty();
                if *self.peek() == TokenKind::Semicolon {
                    self.bump();
                    let count = self.parse_expr();
                    self.expect(&TokenKind::RBracket, "`]`");
                    Ty::Array(Box::new(ty), Box::new(count), span)
                } else {
                    self.expect(&TokenKind::RBracket, "`]`");
                    Ty::Slice(Box::new(ty), span)
                }
            }
            TokenKind::And => {
                self.bump();
                // Preserve lifetime in `&'a T` — Stage 1 lifetime elision needs this.
                let lifetime: Option<Lifetime> =
                    if let TokenKind::Lifetime(sym) = self.peek().clone() {
                        let l_span = self.current_span();
                        self.bump();
                        Some(Lifetime {
                            ident: Ident::new(sym, l_span),
                            span: l_span,
                        })
                    } else {
                        None
                    };
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let ty = self.parse_ty();
                Ty::Ref(lifetime, mutability, Box::new(ty), span)
            }
            TokenKind::Star => {
                self.bump();
                // *const or *mut
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    self.bump(); // const
                    Mutability::Immutable
                };
                let ty = self.parse_ty();
                Ty::Ptr(mutability, Box::new(ty), span)
            }
            TokenKind::KwFn => {
                self.bump();
                let is_unsafe = false; // `unsafe fn` type handled by KwUnsafe arm below
                let _ = is_unsafe;
                self.expect(&TokenKind::LParen, "`(`");
                let mut inputs = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    // Fn pointer params are types only (no patterns)
                    inputs.push(self.parse_ty());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                let output = if *self.peek() == TokenKind::Arrow {
                    self.bump();
                    Box::new(self.parse_ty())
                } else {
                    Box::new(Ty::Tuple(Vec::new(), span))
                };
                Ty::FnPtr {
                    inputs,
                    output,
                    abi: Abi::Landin,
                    is_unsafe: false,
                    span,
                }
            }
            // `unsafe fn` as a type (fn pointer that is unsafe)
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwFn) => {
                self.bump(); // unsafe
                self.bump(); // fn
                self.expect(&TokenKind::LParen, "`(`");
                let mut inputs = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    inputs.push(self.parse_ty());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                let output = if *self.peek() == TokenKind::Arrow {
                    self.bump();
                    Box::new(self.parse_ty())
                } else {
                    Box::new(Ty::Tuple(Vec::new(), span))
                };
                Ty::FnPtr {
                    inputs,
                    output,
                    abi: Abi::Landin,
                    is_unsafe: true,
                    span,
                }
            }
            // impl Trait — `impl Clone + Default`
            TokenKind::KwImpl => {
                self.bump();
                let bounds = self.parse_type_bounds();
                Ty::ImplTrait(bounds, span)
            }
            // dyn Trait — `dyn Display + Send`
            TokenKind::KwDyn => {
                self.bump();
                let bounds = self.parse_type_bounds();
                // Optional lifetime before `dyn`: `'a dyn Trait` (rare; usually `&(dyn Trait + 'a)`)
                Ty::TraitObject {
                    bounds,
                    lifetime: None,
                    span,
                }
            }
            TokenKind::Underscore => {
                self.bump();
                Ty::Infer(span)
            }
            _ => {
                // Path type (including KwSelf_ / KwSelfType as the type itself)
                let path = self.parse_path();
                Ty::Path(QSelf::default(), path, span)
            }
        }
    }
}
