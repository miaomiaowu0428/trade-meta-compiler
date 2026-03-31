//! 错误类型定义

use crate::symbol_table::SymbolCategory;
use crate::types::TypeSpec;
use std::fmt;

pub type CheckResult<T> = Result<T, CheckError>;

/// 检查错误
#[derive(Debug, Clone, PartialEq)]
pub enum CheckError {
    /// 未定义的符号
    UndefinedSymbol {
        name: String,
        category: SymbolCategory,
    },
    /// 类型不匹配
    TypeMismatch {
        expected: TypeSpec,
        actual: TypeSpec,
        context: String,
    },
    /// 缺少必需参数
    MissingRequiredParam { executor: String, param: String },
    /// 未知参数
    UnknownParam { executor: String, param: String },
    /// 二元运算类型错误
    InvalidBinaryOp {
        left: TypeSpec,
        op: String,
        right: TypeSpec,
    },
    /// 比较运算类型错误
    InvalidCompare { left: TypeSpec, right: TypeSpec },
    /// 未声明的变量
    UndeclaredVariable { name: String },
    /// 变量类型不匹配
    VariableTypeMismatch {
        name: String,
        expected: TypeSpec,
        actual: TypeSpec,
    },
    /// 隐式上下文未就绪
    ContextNotAvailable {
        protocol: String,
        symbol: String,
        op: String,
    },
    /// 隐式上下文已被消费
    ContextAlreadyConsumed { protocol: String, symbol: String },
    /// 隐式上下文重复产出
    ContextAlreadyProduced { protocol: String, symbol: String },
    /// 自定义错误
    Custom(String),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedSymbol { name, category } => {
                write!(f, "Undefined {} symbol: '{}'", category, name)
            }
            Self::TypeMismatch {
                expected,
                actual,
                context,
            } => {
                write!(
                    f,
                    "Type mismatch in {}: expected {}, found {}",
                    context, expected, actual
                )
            }
            Self::MissingRequiredParam { executor, param } => {
                write!(
                    f,
                    "Missing required parameter '{}' for executor '{}'",
                    param, executor
                )
            }
            Self::UnknownParam { executor, param } => {
                write!(
                    f,
                    "Unknown parameter '{}' for executor '{}'",
                    param, executor
                )
            }
            Self::InvalidBinaryOp { left, op, right } => {
                write!(f, "Invalid binary operation: {} {} {}", left, op, right)
            }
            Self::InvalidCompare { left, right } => {
                write!(f, "Cannot compare {} with {}", left, right)
            }
            Self::UndeclaredVariable { name } => {
                write!(f, "Undeclared variable: '{}'", name)
            }
            Self::VariableTypeMismatch {
                name,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Variable '{}' type mismatch: expected {}, found {}",
                    name, expected, actual
                )
            }
            Self::ContextNotAvailable {
                protocol,
                symbol,
                op,
            } => {
                write!(
                    f,
                    "Implicit context '{}' not available when '{}' tries to {} it",
                    protocol, symbol, op
                )
            }
            Self::ContextAlreadyConsumed { protocol, symbol } => {
                write!(
                    f,
                    "Implicit context '{}' already consumed when '{}' tries to use it",
                    protocol, symbol
                )
            }
            Self::ContextAlreadyProduced { protocol, symbol } => {
                write!(
                    f,
                    "Implicit context '{}' already available when '{}' tries to produce it again",
                    protocol, symbol
                )
            }
            Self::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for CheckError {}
