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
    pub buy: BuySpec,
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

/// 买入规范（支持解构赋值和列表模式）
#[derive(Debug, Clone, PartialEq)]
pub enum BuySpec {
    /// 直接调用：buy: PumpBuy(...)
    Direct(CallExpr),
    /// 解构赋值：buy: let (var1, var2) = expr
    Destructure {
        targets: Vec<Option<String>>,
        value: DataExpr,
    },
    /// 列表模式：buy: [item1, item2, ...]
    /// 顺序执行，失败项解构变量设为 Uninit，至少一项成功则进 sell
    List(Vec<BuyItem>),
}

/// buy 列表中的单个项
#[derive(Debug, Clone, PartialEq)]
pub enum BuyItem {
    /// 直接调用：PumpBuy(...)
    Direct(CallExpr),
    /// 解构赋值：let (var1, var2) = expr
    Destructure {
        targets: Vec<Option<String>>,
        value: DataExpr,
    },
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

    /// 控制流调用：`SymbolName[cond => [execs], ...],`
    ///
    /// `Spawn`、`OneOf` 等都是用此语法注册的插件符号，
    /// 元编译器不硬编码其语义——解释器按名称分派行为。
    ///
    /// `All[cond1, cond2] => [execs]` 也归入此变体，
    /// 解析时将共享的 executor 列表复制到每个 (cond, execs) 分支中。
    ControlFlow {
        /// 符号名（如 OneOf、Spawn、All）
        name: String,
        /// 分支列表：每项为 (Condition, 执行器列表)
        branches: Vec<(Condition, Vec<ExecutorItem>)>,
    },
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
    /// 函数调用形式的条件（V6.0）
    Call(CallExpr),
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
}

/// 执行器序列项（V6.0 新设计）
///
/// 出现在 `[...]` 执行器列表中。`Done` 不是特殊关键字，
/// 而是一个零参数的已注册 Executor 符号，解释器按名称识别并传播 "done" 信号。
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorItem {
    /// 执行器调用（包含带参数的，也包含零参数的如 Done）
    Executor(ExecutorCall),
    /// 序列内变量赋値：`let peak = PumpPrice`
    LetAssign { var_name: String, value: DataExpr },
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
    /// 时间段（秒）
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
