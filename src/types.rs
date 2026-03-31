//! 类型系统定义

use std::fmt;

/// 类型规范
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeSpec {
    /// 价格类型
    Price,
    /// 数量类型
    Amount,
    /// 时间段类型
    Duration,
    /// 时间点类型
    TimePoint,
    /// 百分比类型
    Percent,
    /// 计数类型
    Count,
    /// 数字类型
    Number,
    /// 字符串类型
    String,
    /// 布尔类型
    Bool,
    /// 任意类型（用于泛型）
    Any,
    /// 元组类型（支持多返回值）
    Tuple(Vec<TypeSpec>),
    /// 链上地址类型（链无关，内部表示为字符串，解释器层可转换为具体类型如 Pubkey）
    Address,
    /// 列表类型（元素类型一致）
    List(Box<TypeSpec>),
    /// 条件表达式类型（用于接受条件作为参数的符号，如 WaitFor(condition: ...)）
    Condition,
}

impl fmt::Display for TypeSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Price => write!(f, "Price"),
            Self::Amount => write!(f, "Amount"),
            Self::Duration => write!(f, "Duration"),
            Self::TimePoint => write!(f, "TimePoint"),
            Self::Percent => write!(f, "Percent"),
            Self::Count => write!(f, "Count"),
            Self::Number => write!(f, "Number"),
            Self::String => write!(f, "String"),
            Self::Bool => write!(f, "Bool"),
            Self::Any => write!(f, "Any"),
            Self::Address => write!(f, "Address"),
            Self::Condition => write!(f, "Condition"),
            Self::List(elem) => write!(f, "[{}]", elem),
            Self::Tuple(types) => {
                write!(f, "(")?;
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                write!(f, ")")
            }
        }
    }
}

/// 类型检查器
pub struct TypeChecker;

impl TypeChecker {
    /// 检查类型是否兼容
    pub fn is_compatible(expected: &TypeSpec, actual: &TypeSpec) -> bool {
        if expected == actual {
            return true;
        }

        // Any 类型兼容所有类型
        if matches!(expected, TypeSpec::Any) || matches!(actual, TypeSpec::Any) {
            return true;
        }

        // Number 可以隐式转换为其他数值类型
        if matches!(actual, TypeSpec::Number) {
            matches!(
                expected,
                TypeSpec::Price | TypeSpec::Amount | TypeSpec::Percent | TypeSpec::Count
            )
        }
        // Percent 可以用于 Amount（如卖出 100% 持仓）
        else if matches!(actual, TypeSpec::Percent) && matches!(expected, TypeSpec::Amount) {
            true
        }
        // List 元素类型兼容即可
        else if let (TypeSpec::List(exp_elem), TypeSpec::List(act_elem)) = (expected, actual) {
            Self::is_compatible(exp_elem, act_elem)
        } else {
            false
        }
    }

    /// 检查二元运算的类型
    pub fn check_binary_op(left: &TypeSpec, op: BinOp, right: &TypeSpec) -> Option<TypeSpec> {
        use BinOp::*;

        match op {
            Add | Sub => {
                // 同类型相加减
                if left == right {
                    Some(left.clone())
                } else {
                    None
                }
            }
            Mul | Div => {
                // 类型 * Number = 类型
                if matches!(right, TypeSpec::Number) {
                    Some(left.clone())
                } else if matches!(left, TypeSpec::Number) {
                    Some(right.clone())
                } else if left == right {
                    // 同类型相乘除得到 Number
                    Some(TypeSpec::Number)
                } else {
                    None
                }
            }
        }
    }

    /// 检查比较运算的类型
    pub fn check_compare_op(left: &TypeSpec, right: &TypeSpec) -> bool {
        // 同类型可以比较
        if left == right {
            return true;
        }

        // Number 可以和数值类型比较
        if matches!(left, TypeSpec::Number) {
            matches!(
                right,
                TypeSpec::Price | TypeSpec::Amount | TypeSpec::Percent | TypeSpec::Count
            )
        } else if matches!(right, TypeSpec::Number) {
            matches!(
                left,
                TypeSpec::Price | TypeSpec::Amount | TypeSpec::Percent | TypeSpec::Count
            )
        } else {
            false
        }
    }
}

/// 二元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_compatibility() {
        assert!(TypeChecker::is_compatible(
            &TypeSpec::Price,
            &TypeSpec::Price
        ));
        assert!(TypeChecker::is_compatible(&TypeSpec::Any, &TypeSpec::Price));
        assert!(TypeChecker::is_compatible(
            &TypeSpec::Price,
            &TypeSpec::Number
        ));
        assert!(!TypeChecker::is_compatible(
            &TypeSpec::Price,
            &TypeSpec::Duration
        ));
    }

    #[test]
    fn test_binary_op() {
        // Price * Number = Price
        assert_eq!(
            TypeChecker::check_binary_op(&TypeSpec::Price, BinOp::Mul, &TypeSpec::Number),
            Some(TypeSpec::Price)
        );

        // Price + Price = Price
        assert_eq!(
            TypeChecker::check_binary_op(&TypeSpec::Price, BinOp::Add, &TypeSpec::Price),
            Some(TypeSpec::Price)
        );

        // Price + Duration = None (不兼容)
        assert_eq!(
            TypeChecker::check_binary_op(&TypeSpec::Price, BinOp::Add, &TypeSpec::Duration),
            None
        );
    }

    #[test]
    fn test_compare_op() {
        assert!(TypeChecker::check_compare_op(
            &TypeSpec::Price,
            &TypeSpec::Price
        ));
        assert!(TypeChecker::check_compare_op(
            &TypeSpec::Price,
            &TypeSpec::Number
        ));
        assert!(!TypeChecker::check_compare_op(
            &TypeSpec::Price,
            &TypeSpec::Duration
        ));
    }
}
