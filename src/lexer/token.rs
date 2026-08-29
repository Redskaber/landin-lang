//! Token definitions for Landin.
//!
//! Based on 02-grammar.md §1.2-1.8.

use crate::session::Span;
use lasso::Spur;

/// Interned string identifier.
pub type Symbol = Spur;

/// A single token from the lexer.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// All token kinds in Landin.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // --- Literals ---
    /// Integer literal: value + optional suffix.
    IntLit(u128, Option<IntTy>),
    /// Float literal: value + optional suffix.
    FloatLit(f64, Option<FloatTy>),
    /// Character literal.
    CharLit(char),
    /// String literal (interned).
    StrLit(Symbol),
    /// Byte literal.
    ByteLit(u8),
    /// Byte string literal (interned as raw bytes, stored as symbol for simplicity).
    ByteStrLit(Symbol),
    /// Raw string literal with hash count.
    RawStrLit(Symbol, usize),

    // --- Identifiers ---
    /// Identifier (interned).
    Ident(Symbol),
    /// Raw identifier: r#name
    RawIdent(Symbol),
    /// Lifetime: 'name
    Lifetime(Symbol),

    // --- Keywords ---
    // Strictly reserved (02-grammar.md §1.3)
    KwAs,
    KwBreak,
    KwConst,
    KwContinue,
    KwCrate,
    KwDyn,
    KwElse,
    KwEnum,
    KwExtern,
    KwFalse,
    KwFn,
    KwFor,
    KwIf,
    KwImpl,
    KwIn,
    KwLet,
    KwLoop,
    KwMatch,
    KwMod,
    KwMove,
    KwMut,
    KwPub,
    KwRef,
    KwReturn,
    KwSelf_,
    KwSelfType, // `Self`
    KwStatic,
    KwStruct,
    KwSuper,
    KwTrait,
    KwTrue,
    KwType,
    KwUnsafe,
    KwUse,
    KwWhere,
    KwWhile,
    // Weakly reserved (future use)
    KwAsync,
    KwAwait,

    // --- Operators ---
    // Arithmetic
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %
    // Comparison
    EqEq,  // ==
    NotEq, // !=
    Lt,    // <
    Gt,    // >
    LtEq,  // <=
    GtEq,  // >=
    // Logical
    AndAnd, // &&
    OrOr,   // ||
    Not,    // !
    // Bitwise
    And,   // &
    Or,    // |
    Caret, // ^
    Shl,   // <<
    Shr,   // >>
    // Assignment
    Eq,        // =
    PlusEq,    // +=
    MinusEq,   // -=
    StarEq,    // *=
    SlashEq,   // /=
    PercentEq, // %=
    AndEq,     // &=
    OrEq,      // |=
    CaretEq,   // ^=
    ShlEq,     // <<=
    ShrEq,     // >>=
    // Range
    DotDot,   // ..
    DotDotEq, // ..=
    // Arrow / fat arrow
    Arrow,    // ->
    FatArrow, // =>
    // Path
    PathSep, // ::
    // Other
    Question, // ?

    // --- Punctuation ---
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Semicolon, // ;
    Colon,     // :
    Dot,       // .
    /// `#` for attributes
    Hash,
    /// Stage 18.02: `$` — used in macro_rules! patterns (`$name:fragment`).
    Dollar,
    /// `@` for pattern binding (`ident @ pat`)
    At,

    // --- Special ---
    /// Doc comment: /// or //!
    DocComment(Symbol, bool /* true = inner */),
    /// Underscore wildcard
    Underscore,
    /// End of file.
    Eof,
}

/// Integer type suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

/// Float type suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatTy {
    F32,
    F64,
}

impl TokenKind {
    /// Check if this token is a keyword.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::KwAs
                | TokenKind::KwBreak
                | TokenKind::KwConst
                | TokenKind::KwContinue
                | TokenKind::KwCrate
                | TokenKind::KwDyn
                | TokenKind::KwElse
                | TokenKind::KwEnum
                | TokenKind::KwExtern
                | TokenKind::KwFalse
                | TokenKind::KwFn
                | TokenKind::KwFor
                | TokenKind::KwIf
                | TokenKind::KwImpl
                | TokenKind::KwIn
                | TokenKind::KwLet
                | TokenKind::KwLoop
                | TokenKind::KwMatch
                | TokenKind::KwMod
                | TokenKind::KwMove
                | TokenKind::KwMut
                | TokenKind::KwPub
                | TokenKind::KwRef
                | TokenKind::KwReturn
                | TokenKind::KwSelf_
                | TokenKind::KwSelfType
                | TokenKind::KwStatic
                | TokenKind::KwStruct
                | TokenKind::KwSuper
                | TokenKind::KwTrait
                | TokenKind::KwTrue
                | TokenKind::KwType
                | TokenKind::KwUnsafe
                | TokenKind::KwUse
                | TokenKind::KwWhere
                | TokenKind::KwWhile
                | TokenKind::KwAsync
                | TokenKind::KwAwait
        )
    }

    /// Get keyword string for error messages.
    pub fn keyword_str(&self) -> Option<&'static str> {
        Some(match self {
            TokenKind::KwAs => "as",
            TokenKind::KwBreak => "break",
            TokenKind::KwConst => "const",
            TokenKind::KwContinue => "continue",
            TokenKind::KwCrate => "crate",
            TokenKind::KwDyn => "dyn",
            TokenKind::KwElse => "else",
            TokenKind::KwEnum => "enum",
            TokenKind::KwExtern => "extern",
            TokenKind::KwFalse => "false",
            TokenKind::KwFn => "fn",
            TokenKind::KwFor => "for",
            TokenKind::KwIf => "if",
            TokenKind::KwImpl => "impl",
            TokenKind::KwIn => "in",
            TokenKind::KwLet => "let",
            TokenKind::KwLoop => "loop",
            TokenKind::KwMatch => "match",
            TokenKind::KwMod => "mod",
            TokenKind::KwMove => "move",
            TokenKind::KwMut => "mut",
            TokenKind::KwPub => "pub",
            TokenKind::KwRef => "ref",
            TokenKind::KwReturn => "return",
            TokenKind::KwSelf_ => "self",
            TokenKind::KwSelfType => "Self",
            TokenKind::KwStatic => "static",
            TokenKind::KwStruct => "struct",
            TokenKind::KwSuper => "super",
            TokenKind::KwTrait => "trait",
            TokenKind::KwTrue => "true",
            TokenKind::KwType => "type",
            TokenKind::KwUnsafe => "unsafe",
            TokenKind::KwUse => "use",
            TokenKind::KwWhere => "where",
            TokenKind::KwWhile => "while",
            TokenKind::KwAsync => "async",
            TokenKind::KwAwait => "await",
            _ => return None,
        })
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Ident(_) => write!(f, "identifier"),
            TokenKind::IntLit(_, _) => write!(f, "integer literal"),
            TokenKind::FloatLit(_, _) => write!(f, "float literal"),
            TokenKind::CharLit(_) => write!(f, "char literal"),
            TokenKind::StrLit(_) => write!(f, "string literal"),
            TokenKind::ByteLit(_) => write!(f, "byte literal"),
            TokenKind::ByteStrLit(_) => write!(f, "byte string literal"),
            TokenKind::RawStrLit(_, _) => write!(f, "raw string literal"),
            TokenKind::Lifetime(_) => write!(f, "lifetime"),
            TokenKind::RawIdent(_) => write!(f, "raw identifier"),
            TokenKind::DocComment(_, _) => write!(f, "doc comment"),
            TokenKind::Eof => write!(f, "end of file"),
            kw if kw.is_keyword() => {
                // Guarded by `is_keyword()`: only true keywords have keyword_str.
                write!(
                    f,
                    "`{}`",
                    kw.keyword_str().expect("is_keyword => keyword_str is Some")
                )
            }
            TokenKind::Plus => write!(f, "`+`"),
            TokenKind::Minus => write!(f, "`-`"),
            TokenKind::Star => write!(f, "`*`"),
            TokenKind::Slash => write!(f, "`/`"),
            TokenKind::Percent => write!(f, "`%`"),
            TokenKind::EqEq => write!(f, "`==`"),
            TokenKind::NotEq => write!(f, "`!=`"),
            TokenKind::Lt => write!(f, "`<`"),
            TokenKind::Gt => write!(f, "`>`"),
            TokenKind::LtEq => write!(f, "`<=`"),
            TokenKind::GtEq => write!(f, "`>=`"),
            TokenKind::AndAnd => write!(f, "`&&`"),
            TokenKind::OrOr => write!(f, "`||`"),
            TokenKind::Not => write!(f, "`!`"),
            TokenKind::And => write!(f, "`&`"),
            TokenKind::Or => write!(f, "`|`"),
            TokenKind::Caret => write!(f, "`^`"),
            TokenKind::Shl => write!(f, "`<<`"),
            TokenKind::Shr => write!(f, "`>>`"),
            TokenKind::Eq => write!(f, "`=`"),
            TokenKind::PlusEq => write!(f, "`+=`"),
            TokenKind::MinusEq => write!(f, "`-=`"),
            TokenKind::StarEq => write!(f, "`*=`"),
            TokenKind::SlashEq => write!(f, "`/=`"),
            TokenKind::PercentEq => write!(f, "`%=`"),
            TokenKind::AndEq => write!(f, "`&=`"),
            TokenKind::OrEq => write!(f, "`|=`"),
            TokenKind::CaretEq => write!(f, "`^=`"),
            TokenKind::ShlEq => write!(f, "`<<=`"),
            TokenKind::ShrEq => write!(f, "`>>=`"),
            TokenKind::DotDot => write!(f, "`..`"),
            TokenKind::DotDotEq => write!(f, "`..=`"),
            TokenKind::Arrow => write!(f, "`->`"),
            TokenKind::FatArrow => write!(f, "`=>`"),
            TokenKind::PathSep => write!(f, "`::`"),
            TokenKind::Question => write!(f, "`?`"),
            TokenKind::LParen => write!(f, "`(`"),
            TokenKind::RParen => write!(f, "`)`"),
            TokenKind::LBrace => write!(f, "`{{`"),
            TokenKind::RBrace => write!(f, "`}}`"),
            TokenKind::LBracket => write!(f, "`[`"),
            TokenKind::RBracket => write!(f, "`]`"),
            TokenKind::Comma => write!(f, "`,`"),
            TokenKind::Semicolon => write!(f, "`;`"),
            TokenKind::Colon => write!(f, "`:`"),
            TokenKind::Dot => write!(f, "`.`"),
            TokenKind::Hash => write!(f, "`#`"),
            TokenKind::Dollar => write!(f, "`$`"),
            TokenKind::At => write!(f, "`@`"),
            TokenKind::Underscore => write!(f, "`_`"),
            _ => write!(f, "token"),
        }
    }
}

/// Lookup table for keywords.
pub fn keyword_from_str(s: &str) -> Option<TokenKind> {
    Some(match s {
        "as" => TokenKind::KwAs,
        "break" => TokenKind::KwBreak,
        "const" => TokenKind::KwConst,
        "continue" => TokenKind::KwContinue,
        "crate" => TokenKind::KwCrate,
        "dyn" => TokenKind::KwDyn,
        "else" => TokenKind::KwElse,
        "enum" => TokenKind::KwEnum,
        "extern" => TokenKind::KwExtern,
        "false" => TokenKind::KwFalse,
        "fn" => TokenKind::KwFn,
        "for" => TokenKind::KwFor,
        "if" => TokenKind::KwIf,
        "impl" => TokenKind::KwImpl,
        "in" => TokenKind::KwIn,
        "let" => TokenKind::KwLet,
        "loop" => TokenKind::KwLoop,
        "match" => TokenKind::KwMatch,
        "mod" => TokenKind::KwMod,
        "move" => TokenKind::KwMove,
        "mut" => TokenKind::KwMut,
        "pub" => TokenKind::KwPub,
        "ref" => TokenKind::KwRef,
        "return" => TokenKind::KwReturn,
        "self" => TokenKind::KwSelf_,
        "Self" => TokenKind::KwSelfType,
        "static" => TokenKind::KwStatic,
        "struct" => TokenKind::KwStruct,
        "super" => TokenKind::KwSuper,
        "trait" => TokenKind::KwTrait,
        "true" => TokenKind::KwTrue,
        "type" => TokenKind::KwType,
        "unsafe" => TokenKind::KwUnsafe,
        "use" => TokenKind::KwUse,
        "where" => TokenKind::KwWhere,
        "while" => TokenKind::KwWhile,
        "async" => TokenKind::KwAsync,
        "await" => TokenKind::KwAwait,
        _ => return None,
    })
}
