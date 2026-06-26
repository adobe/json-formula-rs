/*
Copyright 2025 Adobe. All rights reserved.
This file is licensed to you under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License. You may obtain a copy
of the License at http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed under
the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR REPRESENTATIONS
OF ANY KIND, either express or implied. See the License for the specific language
governing permissions and limitations under the License.
*/

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    Identity,
    Identifier(String),
    QuotedIdentifier(String),
    Literal(Value),
    String(String),
    Number(f64),
    Integer(i64),
    Current,
    Global(String),
    ExpressionReference(Box<AstNode>),
    NotExpression(Box<AstNode>),
    UnaryMinusExpression(Box<AstNode>),
    ConcatenateExpression(Box<AstNode>, Box<AstNode>),
    OrExpression(Box<AstNode>, Box<AstNode>),
    AndExpression(Box<AstNode>, Box<AstNode>),
    AddExpression(Box<AstNode>, Box<AstNode>),
    SubtractExpression(Box<AstNode>, Box<AstNode>),
    MultiplyExpression(Box<AstNode>, Box<AstNode>),
    DivideExpression(Box<AstNode>, Box<AstNode>),
    UnionExpression(Box<AstNode>, Box<AstNode>),
    Comparator {
        op: String,
        left: Box<AstNode>,
        right: Box<AstNode>,
    },
    Pipe(Box<AstNode>, Box<AstNode>),
    ChainedExpression(Vec<AstNode>),
    BracketExpression(Box<AstNode>, Box<AstNode>),
    Index(Box<AstNode>),
    Slice {
        start: Option<i64>,
        stop: Option<i64>,
        step: Option<i64>,
    },
    Projection {
        left: Box<AstNode>,
        right: Box<AstNode>,
        debug: Option<String>,
    },
    ValueProjection {
        left: Box<AstNode>,
        right: Box<AstNode>,
    },
    FilterProjection {
        left: Box<AstNode>,
        right: Box<AstNode>,
        condition: Box<AstNode>,
    },
    Flatten(Box<AstNode>),
    Function {
        name: String,
        args: Vec<AstNode>,
    },
    ArrayExpression(Vec<AstNode>),
    ObjectExpression(Vec<KeyValuePair>),
    KeyValuePair {
        key: String,
        value: Box<AstNode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyValuePair {
    pub key: String,
    pub value: AstNode,
}

impl AstNode {
    fn is_binary(&self) -> bool {
        matches!(
            self,
            AstNode::AndExpression(..)
                | AstNode::OrExpression(..)
                | AstNode::AddExpression(..)
                | AstNode::SubtractExpression(..)
                | AstNode::MultiplyExpression(..)
                | AstNode::DivideExpression(..)
                | AstNode::ConcatenateExpression(..)
                | AstNode::UnionExpression(..)
                | AstNode::Comparator { .. }
        )
    }

    fn expr_str_paren(&self) -> String {
        if self.is_binary() {
            format!("({})", self.to_expr_string())
        } else {
            self.to_expr_string()
        }
    }

    /// Reconstruct a human-readable expression string from this AST node.
    ///
    /// This method is intended for display and debugging only — it is NOT a
    /// lossless round-trip serializer. In particular, `AstNode::Identity` and
    /// `AstNode::Current` both render as `"@"` since they are semantically
    /// equivalent in display context.
    pub fn to_expr_string(&self) -> String {
        match self {
            AstNode::Identity => "@".to_string(),
            AstNode::Current => "@".to_string(),
            AstNode::Identifier(name) | AstNode::QuotedIdentifier(name) => name.clone(),
            AstNode::Literal(v) => v.to_string(),
            AstNode::String(s) => format!("\"{}\"", s),
            AstNode::Number(n) => {
                if n.fract() == 0.0 { format!("{}", *n as i64) } else { format!("{}", n) }
            }
            AstNode::Integer(n) => format!("{}", n),
            AstNode::Global(name) => format!("${}", name),
            AstNode::NotExpression(inner) => format!("!{}", inner.expr_str_paren()),
            AstNode::UnaryMinusExpression(inner) => format!("-{}", inner.expr_str_paren()),
            AstNode::AndExpression(l, r) => {
                format!("{} && {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::OrExpression(l, r) => {
                format!("{} || {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::AddExpression(l, r) => {
                format!("{} + {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::SubtractExpression(l, r) => {
                format!("{} - {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::MultiplyExpression(l, r) => {
                format!("{} * {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::DivideExpression(l, r) => {
                format!("{} / {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::ConcatenateExpression(l, r) => {
                format!("{} & {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::UnionExpression(l, r) => {
                format!("{}, {}", l.to_expr_string(), r.to_expr_string())
            }
            AstNode::Comparator { op, left, right } => {
                format!("{} {} {}", left.to_expr_string(), op, right.to_expr_string())
            }
            AstNode::Pipe(l, r) => {
                format!("{} | {}", l.to_expr_string(), r.to_expr_string())
            }
            AstNode::ChainedExpression(children) => {
                children.iter().map(|c| c.to_expr_string()).collect::<Vec<_>>().join(".")
            }
            AstNode::BracketExpression(left, right) => {
                format!("{}[{}]", left.to_expr_string(), right.to_expr_string())
            }
            AstNode::Index(inner) => inner.to_expr_string(),
            AstNode::Slice { start, stop, step } => {
                let start_s = start.map(|n| n.to_string()).unwrap_or_default();
                let stop_s = stop.map(|n| n.to_string()).unwrap_or_default();
                match step {
                    Some(s) => format!("{}:{}:{}", start_s, stop_s, s),
                    None => format!("{}:{}", start_s, stop_s),
                }
            }
            AstNode::Projection { left, right, .. } => {
                let right_s = if matches!(**right, AstNode::Identity) {
                    String::new()
                } else {
                    format!(".{}", right.to_expr_string())
                };
                format!("{}[*]{}", left.to_expr_string(), right_s)
            }
            AstNode::ValueProjection { left, right } => {
                let right_s = if matches!(**right, AstNode::Identity) {
                    String::new()
                } else {
                    format!(".{}", right.to_expr_string())
                };
                format!("{}.*{}", left.to_expr_string(), right_s)
            }
            AstNode::FilterProjection { left, right, condition } => {
                let right_s = if matches!(**right, AstNode::Identity) {
                    String::new()
                } else {
                    format!(".{}", right.to_expr_string())
                };
                format!("{}[?{}]{}", left.to_expr_string(), condition.to_expr_string(), right_s)
            }
            AstNode::Flatten(inner) => format!("{}[]", inner.to_expr_string()),
            AstNode::Function { name, args } => {
                let args_s = args.iter().map(|a| a.to_expr_string()).collect::<Vec<_>>().join(", ");
                format!("{}({})", name, args_s)
            }
            AstNode::ArrayExpression(items) => {
                let items_s = items.iter().map(|a| a.to_expr_string()).collect::<Vec<_>>().join(", ");
                format!("[{}]", items_s)
            }
            AstNode::ObjectExpression(pairs) => {
                let pairs_s = pairs.iter()
                    .map(|p| format!("{}: {}", p.key, p.value.to_expr_string()))
                    .collect::<Vec<_>>().join(", ");
                format!("{{{}}}", pairs_s)
            }
            AstNode::ExpressionReference(inner) => {
                format!("&{}", inner.to_expr_string())
            }
            AstNode::KeyValuePair { key, value } => {
                format!("{}: {}", key, value.to_expr_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_expr_string_identifier() {
        assert_eq!(AstNode::Identifier("foo".to_string()).to_expr_string(), "foo");
    }

    #[test]
    fn test_to_expr_string_and() {
        let node = AstNode::AndExpression(
            Box::new(AstNode::Identifier("A".to_string())),
            Box::new(AstNode::Identifier("B".to_string())),
        );
        assert_eq!(node.to_expr_string(), "A && B");
    }

    #[test]
    fn test_to_expr_string_nested_binary() {
        let add = AstNode::AddExpression(
            Box::new(AstNode::Identifier("A".to_string())),
            Box::new(AstNode::Identifier("B".to_string())),
        );
        let mul = AstNode::MultiplyExpression(
            Box::new(add),
            Box::new(AstNode::Identifier("C".to_string())),
        );
        assert_eq!(mul.to_expr_string(), "(A + B) * C");
    }

    #[test]
    fn test_to_expr_string_comparator() {
        let node = AstNode::Comparator {
            op: ">".to_string(),
            left: Box::new(AstNode::Identifier("x".to_string())),
            right: Box::new(AstNode::Number(10.0)),
        };
        assert_eq!(node.to_expr_string(), "x > 10");
    }

    #[test]
    fn test_to_expr_string_function() {
        let node = AstNode::Function {
            name: "sum".to_string(),
            args: vec![
                AstNode::Identifier("a".to_string()),
                AstNode::Identifier("b".to_string()),
            ],
        };
        assert_eq!(node.to_expr_string(), "sum(a, b)");
    }
}
