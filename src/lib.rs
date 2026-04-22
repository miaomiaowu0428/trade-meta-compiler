//! Meta-Compiler
//!
//! 编译器生成器层：包含语言 AST、lalrpop 语法解析器、语义检查器和符号表抽象。
//! 领域无关设计——任何业务域（Solana、EVM 等）都可在此之上注册符号。

#![allow(deprecated)]

// ── 语言核心模块 ─────────────────────────────────────────────────────────────
pub mod ast;
pub mod builtin_symbols;
pub mod checker;
pub mod error;
pub mod runtime_value;
pub mod symbol_table;
pub mod types;

// ── Parser（由 lalrpop 生成）─────────────────────────────────────────────────
#[allow(
    unused_imports,
    dead_code,
    unused_mut,
    unused_variables,
    non_snake_case,
    non_camel_case_types,
    clippy::all
)]
mod trade_v6 {
    include!(concat!(env!("OUT_DIR"), "/trade_v6.rs"));
}
pub use trade_v6::StrategyParser;

// ── Re-exports ────────────────────────────────────────────────────────────────
pub use ast::*;
pub use builtin_symbols::builtin_symbol_registry;
pub use checker::Checker;
pub use error::{CheckError, CheckResult};
pub use inventory;
pub use runtime_value::{RuntimeValue, TaskValue};
pub use symbol_table::{
    ContextInteraction, ContextOp, ParamSpec, SymbolCategory, SymbolFactory, SymbolMetadata,
    SymbolRegistry, TypeAliasDef, TypeAliasFactory, UnitDef,
};
pub use types::{TypeChecker, TypeSpec};
