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

/// 构建包含所有内置控制流符号的符号注册表
///
/// 内置符号（大驼峰命名规范）：
///
/// | 符号      | 类别      | 说明                                                 |
/// |-----------|-----------|------------------------------------------------------|
/// | `Done`    | Executor  | 终止当前 sell 序列，触发 Done 信号                    |
/// | `Spawn`   | Executor  | 启动并发后台分支；`Done` 触发后整个 Task 终止         |
/// | `OneOf`   | Executor  | 并发竞争多分支，第一个条件成立的分支的执行器优先执行  |
/// | `All`     | Executor  | 等待所有条件均满足后执行关联执行器                    |
/// | `Timeout` | Condition | 等待指定时长（支持字面量和变量），可持续轮询变量变化  |
pub fn builtin_symbol_registry() -> SymbolRegistry {
    let mut r = SymbolRegistry::new();
    for m in builtin_symbols() {
        r.register(m);
    }
    r
}

fn builtin_symbols() -> Vec<SymbolMetadata> {
    vec![
        // ── 控制流 Executor ──────────────────────────────────────────────────
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
            params: vec![],
            contexts: vec![],
        },
        SymbolMetadata {
            name: "OneOf",
            returns: None,
            category: SymbolCategory::Executor,
            params: vec![],
            contexts: vec![],
        },
        SymbolMetadata {
            name: "All",
            returns: None,
            category: SymbolCategory::Executor,
            params: vec![],
            contexts: vec![],
        },
        // ── 核心 Condition ───────────────────────────────────────────────────
        SymbolMetadata {
            name: "Timeout",
            returns: Some(TypeSpec::Bool),
            category: SymbolCategory::Condition,
            params: vec![ParamSpec::required_multi(
                "duration",
                vec![TypeSpec::Duration, TypeSpec::Any],
            )],
            contexts: vec![],
        },
    ]
}
