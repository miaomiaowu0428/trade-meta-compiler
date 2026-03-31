//! 符号表抽象

use crate::types::TypeSpec;
use std::fmt;

// ── 货币/单位定义 ────────────────────────────────────────────────────────────

/// 领域可扩展的单位定义（如 SOL, USDC, lamports）\n/// 通过 inventory 注册，Checker 在编译期验证 DSL 中出现的单位名是否合法。
pub struct UnitDef {
    /// 单位名称（对应 DSL 中的后缀，如 \"SOL\"）
    pub name: &'static str,
    /// 基础类型（通常是 Amount）
    pub base_type: TypeSpec,
}
inventory::collect!(UnitDef);

impl UnitDef {
    /// 查询 inventory 中是否存在指定名称的 UnitDef
    pub fn lookup(name: &str) -> Option<&'static UnitDef> {
        inventory::iter::<UnitDef>
            .into_iter()
            .find(|u| u.name == name)
    }
}

// ── 参数/符号定义 ────────────────────────────────────────────────────────────
// ── 类型别名定义 ──────────────────────────────────────────────────────────────────────

/// 类型别名工厂 — 用于 inventory 自动收集
///
/// 包装一个函数指针，因为 `TypeAliasDef` 包含 Vec 字段，无法在 const 中创建。
pub struct TypeAliasFactory(pub fn() -> TypeAliasDef);
inventory::collect!(TypeAliasFactory);

/// 类型别名定义 — 将命名类型映射到原始类型组合
///
/// 用于 `define_symbol!` 宏中：`param slippage: Slippage;`
/// 其中 `Slippage` 是通过 inventory 注册的别名。
pub struct TypeAliasDef {
    /// 别名名称（对应 define_symbol! 中的类型名）
    pub name: &'static str,
    /// 允许的原始类型列表
    pub types: Vec<TypeSpec>,
}

impl TypeAliasDef {
    /// 通过 inventory 查找命名别名，返回允许的类型列表
    pub fn lookup(name: &str) -> Option<Vec<TypeSpec>> {
        for factory in inventory::iter::<TypeAliasFactory> {
            let def = (factory.0)();
            if def.name == name {
                return Some(def.types);
            }
        }
        None
    }
}
/// 参数规范
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// 参数名称
    pub name: &'static str,
    /// 允许的参数类型（支持多类型参数）
    pub allowed_types: Vec<TypeSpec>,
    /// 是否必需
    pub required: bool,
}

impl ParamSpec {
    /// 创建必需参数（单一类型）
    pub fn required(name: &'static str, ty: TypeSpec) -> Self {
        Self {
            name,
            allowed_types: vec![ty],
            required: true,
        }
    }

    /// 创建必需参数（多种类型）
    pub fn required_multi(name: &'static str, types: Vec<TypeSpec>) -> Self {
        Self {
            name,
            allowed_types: types,
            required: true,
        }
    }

    /// 创建可选参数（单一类型）
    pub fn optional(name: &'static str, ty: TypeSpec) -> Self {
        Self {
            name,
            allowed_types: vec![ty],
            required: false,
        }
    }

    /// 创建可选参数（多种类型）
    pub fn optional_multi(name: &'static str, types: Vec<TypeSpec>) -> Self {
        Self {
            name,
            allowed_types: types,
            required: false,
        }
    }

    /// 检查类型是否被允许（使用 TypeChecker::is_compatible 支持隐式转换）
    pub fn accepts_type(&self, ty: &TypeSpec) -> bool {
        use crate::types::TypeChecker;
        self.allowed_types
            .iter()
            .any(|allowed| TypeChecker::is_compatible(allowed, ty))
    }
}

/// 上下文操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextOp {
    /// 产出新的上下文实例
    Produce,
    /// 只读使用
    Need,
    /// 消费（使用后移除）
    Consume,
}

impl fmt::Display for ContextOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Produce => write!(f, "Produce"),
            Self::Need => write!(f, "Need"),
            Self::Consume => write!(f, "Consume"),
        }
    }
}

/// 符号对隐式上下文的交互声明
#[derive(Debug, Clone, PartialEq)]
pub struct ContextInteraction {
    pub protocol: &'static str,
    pub op: ContextOp,
}

impl ContextInteraction {
    pub fn produce(protocol: &'static str) -> Self {
        Self {
            protocol,
            op: ContextOp::Produce,
        }
    }
    pub fn need(protocol: &'static str) -> Self {
        Self {
            protocol,
            op: ContextOp::Need,
        }
    }
    pub fn consume(protocol: &'static str) -> Self {
        Self {
            protocol,
            op: ContextOp::Consume,
        }
    }
}

/// 符号元数据
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolMetadata {
    /// 符号名称
    pub name: &'static str,
    /// 返回类型（对 DataItem 有效）
    pub returns: Option<TypeSpec>,
    /// 参数列表（对 Executor/Monitor 有效）
    pub params: Vec<ParamSpec>,
    /// 符号类别
    pub category: SymbolCategory,
    /// 隐式上下文交互声明（空 = 无上下文需求）
    pub contexts: Vec<ContextInteraction>,
}

inventory::collect!(SymbolMetadata);

/// 包装函数指针，用于 inventory 自动收集（函数指针是 const，可以在 static 中使用）
pub struct SymbolFactory(pub fn() -> SymbolMetadata);
inventory::collect!(SymbolFactory);

/// 符号类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolCategory {
    /// 数据项（返回值）
    DataItem,
    /// 执行器（副作用）
    Executor,
    /// 监视器 / 事件触发器（改称 Monitor）
    Monitor,
    /// 条件
    Condition,
}

impl fmt::Display for SymbolCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataItem => write!(f, "DataItem"),
            Self::Executor => write!(f, "Executor"),
            Self::Monitor => write!(f, "Monitor"),
            Self::Condition => write!(f, "Condition"),
        }
    }
}

/// 符号注册表（用于运行时查找）
///
/// 将多个符号表整合到一个统一的查找接口
#[derive(Clone)]
pub struct SymbolRegistry {
    symbols: std::collections::HashMap<(String, SymbolCategory), SymbolMetadata>,
}

impl SymbolRegistry {
    pub fn new() -> Self {
        Self {
            symbols: std::collections::HashMap::new(),
        }
    }

    /// 注册符号
    pub fn register(&mut self, metadata: SymbolMetadata) {
        let key = (metadata.name.to_string(), metadata.category);
        self.symbols.insert(key, metadata);
    }

    /// 查找符号
    pub fn lookup(&self, name: &str, category: SymbolCategory) -> Option<&SymbolMetadata> {
        self.symbols.get(&(name.to_string(), category))
    }

    /// 获取所有符号
    pub fn all_symbols(&self, category: SymbolCategory) -> Vec<&SymbolMetadata> {
        self.symbols
            .iter()
            .filter(|((_, cat), _)| *cat == category)
            .map(|(_, meta)| meta)
            .collect()
    }

    /// 合并另一个注册表的所有符号到当前注册表
    pub fn merge(&mut self, other: SymbolRegistry) {
        for (key, meta) in other.symbols {
            self.symbols.insert(key, meta);
        }
    }

    /// 从 inventory 自动收集所有通过 `define_symbol!` 注册的符号
    pub fn collect_from_inventory(&mut self) {
        for factory in inventory::iter::<SymbolFactory> {
            self.register((factory.0)());
        }
    }
}

impl Default for SymbolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
