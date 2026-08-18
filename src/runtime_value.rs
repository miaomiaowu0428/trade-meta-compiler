//! 运行时值类型
//!
//! 定义在 checker-core 中，供 interpreter crate 使用，避免跨 crate 重复定义。
//! 每个 variant 与 TypeSpec 一一对应，便于 validate_against 做类型验证。

use std::any::Any;
use std::sync::Arc;

use crate::TypeSpec;

/// 可传递的任务值：携带一个已组装好的待 spawn 任务。
///
/// 内层为 `Arc<dyn Any + Send + Sync>`，由 pipeline 构造为具体的 `PreparedSpawnTask`
/// 类型，由 `Spawn` 执行器 handler 通过 `Arc::downcast` 还原并执行。
/// 这层类型擦除使得 `trade-meta-compiler` 不必依赖 tokio / futures。
#[derive(Clone)]
pub struct TaskValue(pub Arc<dyn Any + Send + Sync>);

impl std::fmt::Debug for TaskValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task")
    }
}

/// 运行时值：DSL 变量和表达式在解释器中的实际值
#[derive(Debug, Clone)]
pub enum RuntimeValue {
    Price(f64),
    /// 带单位的数量值（value, unit）。unit 为空字符串表示无单位。
    Amount(f64, String),
    Duration(f64),
    TimePoint(f64),
    Percent(f64),
    Count(f64),
    Number(f64),
    Bool(bool),
    Str(String),
    Tuple(Vec<RuntimeValue>),
    /// 列表值
    List(Vec<RuntimeValue>),
    Unit,
    /// 未初始化（buy 失败时解构变量的默认值）
    Uninit,
    /// 已组装好的后台任务（由 `Spawn[...]` 语法构造，交给 Spawn 执行器 handler 派发）
    Task(TaskValue),
}

impl Default for RuntimeValue {
    /// 参数缺失时的占位值（required param 找不到对应 arg 时使用）
    fn default() -> Self {
        Self::Uninit
    }
}

impl RuntimeValue {
    /// 强制转为 f64（适用于所有数值 variant）
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Price(v)
            | Self::Amount(v, _)
            | Self::Duration(v)
            | Self::TimePoint(v)
            | Self::Percent(v)
            | Self::Count(v)
            | Self::Number(v) => *v,
            _ => 0.0,
        }
    }

    /// 是否为 Uninit
    pub fn is_uninit(&self) -> bool {
        matches!(self, Self::Uninit)
    }

    /// 返回该值对应的 TypeSpec（用于 validate_against 返回类型验证）
    pub fn type_spec(&self) -> TypeSpec {
        match self {
            Self::Price(_) => TypeSpec::Price,
            Self::Amount(_, _) => TypeSpec::Amount,
            Self::Duration(_) => TypeSpec::Duration,
            Self::TimePoint(_) => TypeSpec::TimePoint,
            Self::Percent(_) => TypeSpec::Percent,
            Self::Count(_) => TypeSpec::Count,
            Self::Number(_) => TypeSpec::Number,
            Self::Bool(_) => TypeSpec::Bool,
            Self::Str(_) => TypeSpec::String,
            Self::Tuple(vals) => TypeSpec::Tuple(vals.iter().map(|v| v.type_spec()).collect()),
            Self::List(vals) => {
                let elem = vals.first().map(|v| v.type_spec()).unwrap_or(TypeSpec::Any);
                TypeSpec::List(Box::new(elem))
            }
            Self::Unit => TypeSpec::Any,
            Self::Uninit => TypeSpec::Any,
            Self::Task(_) => TypeSpec::Any,
        }
    }
}
