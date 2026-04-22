//! 通用 AST 定义（领域无关）

use std::collections::HashMap;

/// 符号引用
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolRef {
    /// 符号名称
    pub name: String,
    /// 命名空间（可选，用于多业务域场景）
    pub namespace: Option<String>,
}

impl SymbolRef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: None,
        }
    }

    pub fn with_namespace(name: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: Some(namespace.into()),
        }
    }

    /// 获取完整名称（带命名空间）
    pub fn full_name(&self) -> String {
        if let Some(ns) = &self.namespace {
            format!("{}::{}", ns, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// 顶层策略（V6.0）
#[derive(Debug, Clone, PartialEq)]
pub struct Strategy {
    pub name: String,
    pub metadata: StrategyMeta,
    pub vars: VarsBlock,
    /// V6.0: 统一的 Monitor 块
    pub monitor: MonitorBlock,
}

/// Monitor 块
///
/// 包含 Monitor 调用和触发后的交易流程（buy + sell + finally）。
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorBlock {
    /// Monitor/Trigger 调用
    pub monitor_call: CallExpr,
    /// 触发后的处理（buy + sell + finally）
    pub on_trigger: TriggerBody,
}

/// 触发后的交易流程：buy → sell → finally
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerBody {
    pub buy: Vec<Statement>,
    /// buy 失败（is_done）时执行，不进入 sell 阶段
    pub buy_else: Vec<Statement>,
    pub sell: Vec<Statement>,
    /// 兜底执行器序列（sell 执行完毕后，不论 Done 还是顺序结束，都执行此块）
    pub sell_finally: Vec<ExecutorItem>,
}

impl From<TriggerBody> for MonitorBlock {
    fn from(tb: TriggerBody) -> Self {
        MonitorBlock {
            monitor_call: CallExpr {
                name: SymbolRef::new(""),
                args: vec![],
            },
            on_trigger: tb,
        }
    }
}

/// 统一的函数调用表达式（V6.0 新增）
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    /// 函数符号
    pub name: SymbolRef,
    /// 命名参数列表
    pub args: Vec<NamedArg>,
}

/// 命名参数（V6.0 新增）
#[derive(Debug, Clone, PartialEq)]
pub struct NamedArg {
    /// 参数名
    pub name: String,
    /// 参数值
    pub value: DataExpr,
}

/// 策略元数据
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyMeta {
    pub version: String,
    pub description: Option<String>,
}

/// 变量声明块
#[derive(Debug, Clone, PartialEq)]
pub struct VarsBlock {
    pub vars: Vec<VarDecl>,
}

/// 变量声明
#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub name: String,
    pub var_type: VarType,
}

/// 变量类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VarType {
    Price,
    Amount,
    Duration,
    TimePoint,
    Percent,
    Count,
    Number,
    /// 链上地址（内部表示为字符串）
    Address,
}

/// 语句（V6.0 重新设计）
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Let 赋值：`let var = expr,`
    /// 强制要求变量必须在 vars 中预先声明
    LetAssign { var_name: String, value: DataExpr },

    /// Let 解构赋値：`let (var1, var2) = expr,`
    /// 支持任意返回元组的表达式（函数调用、元组字面量、变量等）
    LetDestructure {
        /// 目标变量列表，None 表示 _ 占位符
        targets: Vec<Option<String>>,
        /// 返回元组的表达式
        value: DataExpr,
    },

    /// 直接执行器（无条件）：`AnySymbol(...),`
    Executor { call: CallExpr },

    /// 条件执行：`condition => [executors],`
    ConditionExec {
        condition: Condition,
        executors: Vec<ExecutorItem>,
    },

    /// 后台派生：`Spawn[item1, item2, ...],`
    ///
    /// items 与普通执行器列表 `[...]` 完全一致（`ExecutorItem` 已统一支持条件化执行块）。
    /// 共享同一个 `TradeTaskContext`，Done 信号自动传播。
    Spawn { items: Vec<ExecutorItem> },
}

/// 条件
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// 比较运算
    Compare {
        left: DataExpr,
        op: CompareOp,
        right: DataExpr,
    },
    /// 函数调用形式的条件（V6.0）：Timeout(duration: 15s)
    Call(CallExpr),
    /// 条件组合子：All[c1, c2, ...] / OneOf[c1, c2, ...] / 自定义组合子
    /// name 为注册的条件符号名，conditions 为子条件列表
    Combinator {
        name: String,
        conditions: Vec<Condition>,
    },
    /// 序列条件：Do[exec1, exec2, ...] — 顺序执行完成后返回 true
    Seq { items: Vec<ExecutorItem> },
    /// Let 绑定条件：`let x = Cond(...)` 或 `let (a, b) = Cond(...)`
    ///
    /// condition 触发（返回 true）时，将偏值解构绑定到 targets；
    /// 包裹在 OneOf 内时，落败一侧的 targets 会预初始化为 Uninit。
    LetBound {
        targets: Vec<Option<String>>,
        inner: Box<Condition>,
    },
    /// 默认条件 _
    Default,
}

/// 比较运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq, // ==
    Ne, // !=
    Lt, // <
    Le, // <=
    Gt, // >
    Ge, // >=
}

/// 数据表达式（通用）
#[derive(Debug, Clone, PartialEq)]
pub enum DataExpr {
    /// 变量引用
    Var(String),
    /// 字面量值
    Literal(Value),
    /// 符号引用（替代原来的 DataItem）
    Symbol(SymbolRef),
    /// 二元运算
    BinOp {
        left: Box<DataExpr>,
        op: BinOp,
        right: Box<DataExpr>,
    },
    /// 函数调用表达式
    Call(CallExpr),
    /// 元组表达式：(expr1, expr2, ...)
    Tuple(Vec<DataExpr>),
}

/// 二元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    /// Uninit 回退：左侧为 Uninit 时取右侧值，否则取左侧值（类似 `??` 运算符）
    Or, // OR
}

/// 执行器序列项（V6.0 新设计）
///
/// 出现在 `[...]` 执行器列表中。`Done` 不是特殊关键字，
/// 而是一个零参数的已注册 Executor 符号，解释器按名称识别并传播 "done" 信号。
///
/// 统一支持条件化执行（`condition => [execs]`），任何接受 `ExecutorItem`
/// 的地方均可使用，无需额外中间类型（如 `SpawnItem`）。
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorItem {
    /// 执行器调用（包含带参数的，也包含零参数的如 Done）
    Executor(ExecutorCall),
    /// 序列内变量赋值：`let peak = PumpPrice`
    LetAssign { var_name: String, value: DataExpr },
    /// 序列内解构赋值：`let (a, b) = expr`
    LetDestructure {
        targets: Vec<Option<String>>,
        value: DataExpr,
    },
    /// 条件化执行：`condition => [exec1, exec2, ...]`
    ///
    /// 可在任何 `[...]` 执行器列表中使用，condition 为函数调用形式
    /// （含 LetBound：`let x = Cond(...) => [...]`）。
    CondExec {
        condition: Condition,
        executors: Vec<ExecutorItem>,
    },
}

/// 操作符（执行器序列）
#[derive(Debug, Clone, PartialEq)]
pub struct Operator {
    /// 执行器序列：[Executor1, Executor2, Done]
    pub items: Vec<ExecutorItem>,
}

/// 执行器调用（通用）
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutorCall {
    /// 执行器符号
    pub executor: SymbolRef,
    /// 参数
    pub args: HashMap<String, DataExpr>,
}

/// 值类型
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    /// 百分比
    Percent(f64),
    /// 时间段（毫秒）
    Duration(u64),
    /// 带单位的数量（如 0.5 SOL, 100 USDC）
    Amount(f64, std::string::String),
    /// 列表
    List(Vec<Value>),
    /// 映射
    Map(HashMap<String, Value>),
    /// 元组
    Tuple(Vec<Value>),
    /// 未初始化
    Uninit,
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Percent(p) => Some(*p),
            Self::Amount(n, _) => Some(*n),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }
}
