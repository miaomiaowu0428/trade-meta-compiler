//! 语义检查器

use crate::ast::*;
use crate::error::{CheckError, CheckResult};
use crate::symbol_table::{ContextOp, SymbolCategory, SymbolRegistry};
use crate::types::{TypeChecker, TypeSpec};
use std::collections::HashMap;

/// 检查器
pub struct Checker {
    /// 符号注册表
    registry: SymbolRegistry,
    /// 变量类型表（用于类型检查）
    var_types: HashMap<String, TypeSpec>,
}

impl Checker {
    pub fn new(registry: SymbolRegistry) -> Self {
        Self {
            registry,
            var_types: HashMap::new(),
        }
    }

    /// 检查整个策略（V6.0）
    pub fn check(&mut self, strategy: &Strategy) -> CheckResult<()> {
        // 1. 收集变量类型
        self.collect_var_types(&strategy.vars)?;

        // 2. 检查 Monitor 块
        self.check_monitor_block(&strategy.monitor)?;

        // 3. 检查隐式上下文流
        self.check_context_flow(&strategy.monitor)?;

        Ok(())
    }

    /// 检查 Monitor 块（V6.0 新增）
    fn check_monitor_block(&self, monitor: &MonitorBlock) -> CheckResult<()> {
        // 检查 Monitor 调用
        self.check_call_expr(&monitor.monitor_call, SymbolCategory::Monitor)?;

        // 检查 buy（与 sell 使用完全一致的 Statement 检查）
        for stmt in &monitor.on_trigger.buy {
            self.check_statement(stmt)?;
        }

        // 检查 sell 语句列表
        for stmt in &monitor.on_trigger.sell {
            self.check_statement(stmt)?;
        }

        // 检查 sell_finally 执行器序列（兑底块）
        if !monitor.on_trigger.sell_finally.is_empty() {
            self.check_executor_sequence(&monitor.on_trigger.sell_finally)?;
        }

        Ok(())
    }

    /// 检查解构赋值：验证 RHS 表达式类型是否为元组，以及目标变量类型匹配
    fn check_destructure_targets(
        &self,
        targets: &[Option<String>],
        value: &DataExpr,
    ) -> CheckResult<()> {
        let value_type = self.infer_expr_type(value)?;

        match &value_type {
            TypeSpec::Tuple(tuple_types) => {
                if targets.len() != tuple_types.len() {
                    return Err(CheckError::Custom(format!(
                        "Destructure expects {} values, but expression returns {}",
                        targets.len(),
                        tuple_types.len()
                    )));
                }
                for (target, expected_type) in targets.iter().zip(tuple_types.iter()) {
                    if let Some(var_name) = target {
                        let var_type = self.var_types.get(var_name).ok_or_else(|| {
                            CheckError::UndeclaredVariable {
                                name: var_name.clone(),
                            }
                        })?;
                        if !TypeChecker::is_compatible(var_type, expected_type) {
                            return Err(CheckError::VariableTypeMismatch {
                                name: var_name.clone(),
                                expected: var_type.clone(),
                                actual: expected_type.clone(),
                            });
                        }
                    }
                }
            }
            TypeSpec::Any => {
                // Any 类型跳过类型检查（运行时再验证）
                for target in targets {
                    if let Some(var_name) = target {
                        if !self.var_types.contains_key(var_name) {
                            return Err(CheckError::UndeclaredVariable {
                                name: var_name.clone(),
                            });
                        }
                    }
                }
            }
            scalar_type => {
                // 非元组的单值：仅允许单目标绑定
                if targets.len() > 1 {
                    return Err(CheckError::Custom(format!(
                        "Expression returns a single value ({}), cannot destructure into {} targets",
                        scalar_type,
                        targets.len()
                    )));
                }
                if let Some(Some(var_name)) = targets.first() {
                    let var_type = self.var_types.get(var_name).ok_or_else(|| {
                        CheckError::UndeclaredVariable {
                            name: var_name.clone(),
                        }
                    })?;
                    if !TypeChecker::is_compatible(var_type, scalar_type) {
                        return Err(CheckError::VariableTypeMismatch {
                            name: var_name.clone(),
                            expected: var_type.clone(),
                            actual: scalar_type.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// 检查语句（V6.0 新增）
    fn check_statement(&self, stmt: &Statement) -> CheckResult<()> {
        match stmt {
            Statement::LetAssign { var_name, value } => {
                // 检查变量是否声明
                let var_type =
                    self.var_types
                        .get(var_name)
                        .ok_or_else(|| CheckError::UndeclaredVariable {
                            name: var_name.clone(),
                        })?;

                // 检查值的类型
                let value_type = self.infer_expr_type(value)?;
                if !TypeChecker::is_compatible(var_type, &value_type) {
                    return Err(CheckError::VariableTypeMismatch {
                        name: var_name.clone(),
                        expected: var_type.clone(),
                        actual: value_type,
                    });
                }
            }
            Statement::Executor { call } => {
                // 通用执行器，先尝试 Executor 分类，再 fallback 到 DataItem
                let in_executor = self
                    .registry
                    .lookup(&call.name.name, SymbolCategory::Executor)
                    .is_some();
                if in_executor {
                    self.check_call_expr(call, SymbolCategory::Executor)?;
                } else {
                    // DataItem 作为顶层语句调用（如展示理用） —— 変量存在就通过
                    let in_data = self
                        .registry
                        .lookup(&call.name.name, SymbolCategory::DataItem)
                        .is_some();
                    if !in_data {
                        return Err(CheckError::UndefinedSymbol {
                            name: call.name.name.clone(),
                            category: SymbolCategory::Executor,
                        });
                    }
                }
            }
            Statement::Spawn { items } => {
                self.check_executor_sequence(items)?;
            }
            Statement::ConditionExec {
                condition,
                executors,
            } => {
                self.check_condition(condition)?;
                self.check_executor_sequence(executors)?;
            }

            Statement::LetDestructure { targets, value } => {
                self.check_destructure_targets(targets, value)?;
            }
        }
        Ok(())
    }

    /// 检查函数调用表达式（V6.0 新增）
    fn check_call_expr(
        &self,
        call: &CallExpr,
        expected_category: SymbolCategory,
    ) -> CheckResult<()> {
        // 检查符号是否存在
        let meta = self
            .registry
            .lookup(&call.name.name, expected_category)
            .ok_or_else(|| CheckError::UndefinedSymbol {
                name: call.name.name.clone(),
                category: expected_category,
            })?;

        // 检查必需参数
        let arg_map: HashMap<_, _> = call
            .args
            .iter()
            .map(|arg| (arg.name.as_str(), &arg.value))
            .collect();

        for param_spec in &meta.params {
            if param_spec.required && !arg_map.contains_key(param_spec.name) {
                return Err(CheckError::MissingRequiredParam {
                    executor: call.name.name.clone(),
                    param: param_spec.name.to_string(),
                });
            }
        }

        // 检查参数类型
        for arg in &call.args {
            self.infer_expr_type(&arg.value)?;
        }

        Ok(())
    }

    /// 收集变量类型
    fn collect_var_types(&mut self, vars: &VarsBlock) -> CheckResult<()> {
        for var in &vars.vars {
            let ty = match var.var_type {
                VarType::Price => TypeSpec::Price,
                VarType::Amount => TypeSpec::Amount,
                VarType::Duration => TypeSpec::Duration,
                VarType::TimePoint => TypeSpec::TimePoint,
                VarType::Percent => TypeSpec::Percent,
                VarType::Count => TypeSpec::Count,
                VarType::Number => TypeSpec::Number,
                VarType::Address => TypeSpec::Address,
            };
            self.var_types.insert(var.name.clone(), ty);
        }
        Ok(())
    }

    /// 检查执行器序列
    fn check_executor_sequence(&self, executors: &[ExecutorItem]) -> CheckResult<()> {
        for item in executors {
            match item {
                ExecutorItem::Executor(call) => {
                    self.check_executor_call(call)?;
                }
                ExecutorItem::LetAssign { var_name, value } => {
                    // 序列内赋值，要求变量已在 vars 中声明
                    let var_type = self.var_types.get(var_name).ok_or_else(|| {
                        CheckError::UndeclaredVariable {
                            name: var_name.clone(),
                        }
                    })?;
                    let val_type = self.infer_expr_type(value)?;
                    if !TypeChecker::is_compatible(var_type, &val_type) {
                        return Err(CheckError::VariableTypeMismatch {
                            name: var_name.clone(),
                            expected: var_type.clone(),
                            actual: val_type,
                        });
                    }
                }
                ExecutorItem::LetDestructure { targets, value } => {
                    self.check_destructure_targets(targets, value)?;
                }
            }
        }
        Ok(())
    }

    /// 检查执行器调用
    fn check_executor_call(&self, call: &ExecutorCall) -> CheckResult<()> {
        let meta = self
            .registry
            .lookup(&call.executor.name, SymbolCategory::Executor)
            .ok_or_else(|| CheckError::UndefinedSymbol {
                name: call.executor.name.clone(),
                category: SymbolCategory::Executor,
            })?;

        // 检查必需参数
        for param_spec in &meta.params {
            if param_spec.required && !call.args.contains_key(param_spec.name) {
                return Err(CheckError::MissingRequiredParam {
                    executor: call.executor.name.clone(),
                    param: param_spec.name.to_string(),
                });
            }
        }

        // 检查参数类型
        for (arg_name, arg_value) in &call.args {
            let arg_type = self.infer_expr_type(arg_value)?;

            // 查找参数规范
            if let Some(param_spec) = meta.params.iter().find(|p| p.name == arg_name) {
                // 检查类型是否匹配（支持多类型参数）
                if !param_spec.accepts_type(&arg_type) {
                    return Err(CheckError::TypeMismatch {
                        expected: param_spec.allowed_types[0].clone(), // 显示第一个期望类型
                        actual: arg_type,
                        context: format!("executor {} parameter {}", call.executor.name, arg_name),
                    });
                }
            }
            // 未知参数不报错，允许额外参数
        }

        Ok(())
    }

    /// 检查条件
    fn check_condition(&self, condition: &Condition) -> CheckResult<()> {
        match condition {
            Condition::Compare { left, op: _, right } => {
                let left_ty = self.infer_expr_type(left)?;
                let right_ty = self.infer_expr_type(right)?;

                if !TypeChecker::check_compare_op(&left_ty, &right_ty) {
                    return Err(CheckError::InvalidCompare {
                        left: left_ty,
                        right: right_ty,
                    });
                }
            }
            Condition::Call(call_expr) => {
                // V6.0: 函数调用形式的条件
                self.check_call_expr(call_expr, SymbolCategory::Condition)?;
            }
            Condition::LetBound { targets, inner } => {
                // 检查内部条件
                self.check_condition(inner)?;
                // 目标变量必须在 vars 中声明（_ 除外）
                for target in targets {
                    if let Some(name) = target {
                        if !self.var_types.contains_key(name) {
                            return Err(CheckError::UndeclaredVariable { name: name.clone() });
                        }
                    }
                }
            }
            Condition::Combinator { name, conditions } => {
                // 验证组合子符号已注册
                if self
                    .registry
                    .lookup(name, SymbolCategory::Condition)
                    .is_none()
                {
                    return Err(CheckError::UndefinedSymbol {
                        name: name.clone(),
                        category: SymbolCategory::Condition,
                    });
                }
                for c in conditions {
                    self.check_condition(c)?;
                }
            }
            Condition::Seq { items } => {
                self.check_executor_sequence(items)?;
            }
            Condition::Default => {}
        }
        Ok(())
    }

    /// 推断表达式类型
    fn infer_expr_type(&self, expr: &DataExpr) -> CheckResult<TypeSpec> {
        match expr {
            DataExpr::Var(name) => {
                // 优先从用户声明的 vars 中查找
                if let Some(ty) = self.var_types.get(name) {
                    return Ok(ty.clone());
                }
                // 再查 DataItem 符号注册表（无括号调用的数据符号，如 PumpPrice）
                if let Some(meta) = self.registry.lookup(name, SymbolCategory::DataItem) {
                    return meta.returns.clone().ok_or_else(|| {
                        CheckError::Custom(format!("DataItem '{}' has no return type", name))
                    });
                }
                Err(CheckError::UndeclaredVariable { name: name.clone() })
            }
            DataExpr::Literal(value) => Ok(self.infer_literal_type(value)),
            DataExpr::Symbol(sym) => {
                // 查找符号返回类型
                let meta = self
                    .registry
                    .lookup(&sym.name, SymbolCategory::DataItem)
                    .ok_or_else(|| CheckError::UndefinedSymbol {
                        name: sym.name.clone(),
                        category: SymbolCategory::DataItem,
                    })?;

                meta.returns.clone().ok_or_else(|| {
                    CheckError::Custom(format!("DataItem '{}' has no return type", sym.name))
                })
            }
            DataExpr::BinOp { left, op, right } => {
                let left_ty = self.infer_expr_type(left)?;
                let right_ty = self.infer_expr_type(right)?;

                let bin_op = match op {
                    BinOp::Add => crate::types::BinOp::Add,
                    BinOp::Sub => crate::types::BinOp::Sub,
                    BinOp::Mul => crate::types::BinOp::Mul,
                    BinOp::Div => crate::types::BinOp::Div,
                    BinOp::Or => crate::types::BinOp::Or,
                };

                TypeChecker::check_binary_op(&left_ty, bin_op, &right_ty).ok_or_else(|| {
                    CheckError::InvalidBinaryOp {
                        left: left_ty,
                        op: format!("{:?}", op),
                        right: right_ty,
                    }
                })
            }
            DataExpr::Call(call) => {
                // 查找 Executor 或 Condition 或 DataItem 的返回类型
                let meta = self
                    .registry
                    .lookup(&call.name.name, SymbolCategory::Executor)
                    .or_else(|| {
                        self.registry
                            .lookup(&call.name.name, SymbolCategory::Condition)
                    })
                    .or_else(|| {
                        self.registry
                            .lookup(&call.name.name, SymbolCategory::DataItem)
                    })
                    .ok_or_else(|| CheckError::UndefinedSymbol {
                        name: call.name.name.clone(),
                        category: SymbolCategory::Executor,
                    })?;

                meta.returns.clone().ok_or_else(|| {
                    CheckError::Custom(format!("'{}' has no return type", call.name.name))
                })
            }
            DataExpr::Tuple(exprs) => {
                let types: Vec<TypeSpec> = exprs
                    .iter()
                    .map(|e| self.infer_expr_type(e))
                    .collect::<Result<_, _>>()?;
                Ok(TypeSpec::Tuple(types))
            }
        }
    }

    /// 推断字面量类型
    fn infer_literal_type(&self, value: &Value) -> TypeSpec {
        match value {
            Value::Number(_) => TypeSpec::Number,
            Value::String(_) => TypeSpec::String,
            Value::Bool(_) => TypeSpec::Bool,
            Value::Percent(_) => TypeSpec::Percent,
            Value::Duration(_) => TypeSpec::Duration,
            Value::Amount(_, _) => TypeSpec::Amount,
            Value::List(items) => {
                let elem = items
                    .first()
                    .map(|v| self.infer_literal_type(v))
                    .unwrap_or(TypeSpec::Any);
                TypeSpec::List(Box::new(elem))
            }
            Value::Map(_) => TypeSpec::Any,
            Value::Tuple(items) => {
                let types = items.iter().map(|v| self.infer_literal_type(v)).collect();
                TypeSpec::Tuple(types)
            }
            Value::Uninit => TypeSpec::Any,
        }
    }
}

// ── 隐式上下文流分析 ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ContextStatus {
    Available,
}

#[derive(Clone)]
struct ContextFlowTracker {
    state: HashMap<String, ContextStatus>,
}

impl ContextFlowTracker {
    fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }
}

impl Checker {
    /// 检查整个策略的隐式上下文流
    fn check_context_flow(&self, monitor: &MonitorBlock) -> CheckResult<()> {
        let mut tracker = ContextFlowTracker::new();

        // 1. Monitor 调用（通常 Produce）
        self.apply_symbol_ctx(
            &monitor.monitor_call.name.name,
            SymbolCategory::Monitor,
            &mut tracker,
        )?;

        // 2. Buy 阶段（与 sell 完全一致的 Statement 检查）
        for stmt in &monitor.on_trigger.buy {
            self.check_stmt_ctx(stmt, &mut tracker)?;
        }

        // 3. Sell 语句（顺序）
        for stmt in &monitor.on_trigger.sell {
            self.check_stmt_ctx(stmt, &mut tracker)?;
        }

        // 4. sell_finally
        for item in &monitor.on_trigger.sell_finally {
            self.check_exec_item_ctx(item, &mut tracker)?;
        }

        Ok(())
    }

    fn apply_symbol_ctx(
        &self,
        name: &str,
        category: SymbolCategory,
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        if let Some(meta) = self.registry.lookup(name, category) {
            for ix in &meta.contexts {
                let proto = ix.protocol;
                match ix.op {
                    ContextOp::Produce => {
                        if tracker.state.get(proto) == Some(&ContextStatus::Available) {
                            return Err(CheckError::ContextAlreadyProduced {
                                protocol: proto.to_string(),
                                symbol: name.to_string(),
                            });
                        }
                        tracker
                            .state
                            .insert(proto.to_string(), ContextStatus::Available);
                    }
                    ContextOp::Need => match tracker.state.get(proto) {
                        None => {
                            return Err(CheckError::ContextNotAvailable {
                                protocol: proto.to_string(),
                                symbol: name.to_string(),
                                op: ix.op.to_string(),
                            });
                        }
                        Some(&ContextStatus::Available) => {}
                    },
                    ContextOp::Consume => match tracker.state.get(proto) {
                        None => {
                            return Err(CheckError::ContextNotAvailable {
                                protocol: proto.to_string(),
                                symbol: name.to_string(),
                                op: ix.op.to_string(),
                            });
                        }
                        Some(&ContextStatus::Available) => {
                            tracker.state.remove(proto);
                        }
                    },
                }
            }
        }
        Ok(())
    }

    fn check_call_args_ctx(
        &self,
        args: &[NamedArg],
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        for arg in args {
            self.check_expr_ctx(&arg.value, tracker)?;
        }
        Ok(())
    }

    fn check_stmt_ctx(
        &self,
        stmt: &Statement,
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        match stmt {
            Statement::LetAssign { value, .. } => {
                self.check_expr_ctx(value, tracker)?;
            }
            Statement::Executor { call } => {
                let cat = if self
                    .registry
                    .lookup(&call.name.name, SymbolCategory::Executor)
                    .is_some()
                {
                    SymbolCategory::Executor
                } else {
                    SymbolCategory::DataItem
                };
                self.apply_symbol_ctx(&call.name.name, cat, tracker)?;
                self.check_call_args_ctx(&call.args, tracker)?;
            }
            // ControlFlow 已移除，All/OneOf 通过 Condition::Combinator 处理
            Statement::ConditionExec {
                condition,
                executors,
            } => {
                self.check_condition_ctx(condition, tracker)?;
                let mut branch = tracker.clone();
                for item in executors {
                    self.check_exec_item_ctx(item, &mut branch)?;
                }
            }
            Statement::LetDestructure { value, .. } => {
                self.check_data_expr_ctx(value, tracker)?;
            }
            Statement::Spawn { items } => {
                // Spawn 后台派生，内部 context 变更不传播回父流
                let mut branch = tracker.clone();
                for item in items {
                    self.check_exec_item_ctx(item, &mut branch)?;
                }
            }
        }
        Ok(())
    }

    fn check_exec_item_ctx(
        &self,
        item: &ExecutorItem,
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        match item {
            ExecutorItem::Executor(call) => {
                self.apply_symbol_ctx(&call.executor.name, SymbolCategory::Executor, tracker)?;
                for value in call.args.values() {
                    self.check_expr_ctx(value, tracker)?;
                }
            }
            ExecutorItem::LetAssign { value, .. } => {
                self.check_expr_ctx(value, tracker)?;
            }
            ExecutorItem::LetDestructure { value, .. } => {
                self.check_expr_ctx(value, tracker)?;
            }
        }
        Ok(())
    }

    fn check_condition_ctx(
        &self,
        cond: &Condition,
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        match cond {
            Condition::Compare { left, right, .. } => {
                self.check_expr_ctx(left, tracker)?;
                self.check_expr_ctx(right, tracker)?;
            }
            Condition::Call(call) => {
                self.apply_symbol_ctx(&call.name.name, SymbolCategory::Condition, tracker)?;
                self.check_call_args_ctx(&call.args, tracker)?;
            }
            Condition::Combinator {
                name: _,
                conditions,
            } => {
                // 各子条件独立分析，不传播回父流
                for c in conditions {
                    let mut branch = tracker.clone();
                    self.check_condition_ctx(c, &mut branch)?;
                }
            }
            Condition::Seq { items } => {
                for item in items {
                    self.check_exec_item_ctx(item, tracker)?;
                }
            }
            Condition::LetBound { inner, .. } => {
                self.check_condition_ctx(inner, tracker)?;
            }
            Condition::Default => {}
        }
        Ok(())
    }

    fn check_expr_ctx(&self, expr: &DataExpr, tracker: &mut ContextFlowTracker) -> CheckResult<()> {
        match expr {
            DataExpr::Var(name) => {
                if !self.var_types.contains_key(name) {
                    if let Some(meta) = self.registry.lookup(name, SymbolCategory::DataItem) {
                        for ix in &meta.contexts {
                            let proto = ix.protocol;
                            if matches!(ix.op, ContextOp::Need | ContextOp::Consume)
                                && tracker.state.get(proto).is_none()
                            {
                                return Err(CheckError::ContextNotAvailable {
                                    protocol: proto.to_string(),
                                    symbol: name.clone(),
                                    op: ix.op.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            DataExpr::Symbol(sym) => {
                self.apply_symbol_ctx(&sym.name, SymbolCategory::DataItem, tracker)?;
            }
            DataExpr::BinOp { left, right, .. } => {
                self.check_expr_ctx(left, tracker)?;
                self.check_expr_ctx(right, tracker)?;
            }
            DataExpr::Literal(_) => {}
            DataExpr::Call(call) => {
                self.check_data_expr_call_ctx(call, tracker)?;
            }
            DataExpr::Tuple(exprs) => {
                for e in exprs {
                    self.check_expr_ctx(e, tracker)?;
                }
            }
        }
        Ok(())
    }

    /// 检查函数调用表达式的上下文流（Executor / Condition / DataItem）
    fn check_data_expr_call_ctx(
        &self,
        call: &CallExpr,
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        let cat = if self
            .registry
            .lookup(&call.name.name, SymbolCategory::Executor)
            .is_some()
        {
            SymbolCategory::Executor
        } else if self
            .registry
            .lookup(&call.name.name, SymbolCategory::Condition)
            .is_some()
        {
            SymbolCategory::Condition
        } else {
            SymbolCategory::DataItem
        };
        self.apply_symbol_ctx(&call.name.name, cat, tracker)?;
        self.check_call_args_ctx(&call.args, tracker)?;
        Ok(())
    }

    /// 检查 DataExpr 的上下文流（统一入口，处理 Call / Tuple / 其他表达式）
    fn check_data_expr_ctx(
        &self,
        expr: &DataExpr,
        tracker: &mut ContextFlowTracker,
    ) -> CheckResult<()> {
        self.check_expr_ctx(expr, tracker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checker_basic() {
        let mut registry = SymbolRegistry::new();

        // 注册一个简单的 DataItem
        use crate::symbol_table::SymbolMetadata;
        registry.register(SymbolMetadata {
            name: "TestPrice",
            returns: Some(TypeSpec::Price),
            params: vec![],
            category: SymbolCategory::DataItem,
            contexts: vec![],
        });

        let mut checker = Checker::new(registry);

        // 测试简单的变量类型推断
        let vars = VarsBlock {
            vars: vec![VarDecl {
                name: "my_var".to_string(),
                var_type: VarType::Price,
            }],
        };

        checker.collect_var_types(&vars).unwrap();
        assert_eq!(checker.var_types.get("my_var"), Some(&TypeSpec::Price));
    }
}
