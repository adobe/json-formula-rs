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
