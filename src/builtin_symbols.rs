//! 内置符号表
//!
//! 由 `trade-meta-compiler` 直接导出的预注册符号注册表。
//! 包含语言框架级别的控制流组合子与核心条件符号，
//! 下游编译器（mock、solana 等）应以此为基础添加自己的业务符号：
//!
//! ```ignore
//! use trade_meta_compiler::builtin_symbols::builtin_symbol_registry;
//!
//! let mut reg = builtin_symbol_registry();
//! reg.register(my_plugin_symbol);  // 追加业务符号
//! ```

use crate::symbol_table::{ParamSpec, SymbolCategory, SymbolMetadata, SymbolRegistry};
use crate::types::TypeSpec;

/// 构建包含所有内置符号的符号注册表
///
/// 内置符号（大驼峰命名规范）：
///
/// | 符号      | 类别      | 说明                                                 |
/// |-----------|-----------|------------------------------------------------------|
/// | `Done`  | Executor  | 终止当前 sell 序列，触发 Done 信号                   |
/// | `Spawn` | Executor  | 后台派生任务；pipeline 将 `Spawn[...]` 括号内的序列  |
/// |         |           | 组装为 `RuntimeValue::Task` 作为 `task` 参数传入     |
/// | `All`   | Condition | 条件组合子：并发评估全部为 true 才通过               |
/// | `OneOf` | Condition | 条件组合子：并发竞争，任意一个 true 即通过           |
///
/// `[...]` 内联序列条件由 grammar 直接映射，不在符号表中。
pub fn builtin_symbol_registry() -> SymbolRegistry {
    let mut r = SymbolRegistry::new();
    for m in builtin_symbols() {
        r.register(m);
    }
    r
}

fn builtin_symbols() -> Vec<SymbolMetadata> {
    vec![
        // ── 内置 Executor ────────────────────────────────────────────────────
        SymbolMetadata {
            name: "Done",
            returns: None,
            category: SymbolCategory::Executor,
            params: vec![],
            contexts: vec![],
        },
        SymbolMetadata {
            name: "Spawn",
            returns: None,
            category: SymbolCategory::Executor,
            // `task` 由 pipeline 为 `Spawn[...]` 自动构造；用户语法不显式写出。
            params: vec![ParamSpec {
                name: "task",
                allowed_types: vec![TypeSpec::Any],
                required: false,
            }],
            contexts: vec![],
        },
        // ── 条件组合子 ───────────────────────────────────────────────────────
        SymbolMetadata {
            name: "All",
            returns: None,
            category: SymbolCategory::Condition,
            params: vec![],
            contexts: vec![],
        },
        SymbolMetadata {
            name: "OneOf",
            returns: None,
            category: SymbolCategory::Condition,
            params: vec![],
            contexts: vec![],
        },
    ]
}
