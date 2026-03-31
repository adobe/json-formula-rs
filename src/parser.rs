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

use crate::ast::{AstNode, KeyValuePair};
use crate::errors::JsonFormulaError;
use crate::lexer::{Lexer, Token, TokenKind, TokenValue};

const BP_EOF: i32 = 0;
const BP_IDENTIFIER: i32 = 0;
const BP_QUOTED_IDENTIFIER: i32 = 0;
const BP_RBRACKET: i32 = 0;
const BP_RPAREN: i32 = 0;
const BP_COMMA: i32 = 0;
const BP_RBRACE: i32 = 0;
const BP_NUMBER: i32 = 0;
const BP_INT: i32 = 0;
const BP_COLON: i32 = 0;
const BP_CURRENT: i32 = 0;
const BP_GLOBAL: i32 = 0;
const BP_EXPREF: i32 = 0;
const BP_PIPE: i32 = 1;
const BP_OR: i32 = 2;
const BP_AND: i32 = 3;
const BP_COMPARATOR: i32 = 4;
const BP_CONCATENATE: i32 = 5;
const BP_ADD: i32 = 6;
const BP_SUBTRACT: i32 = 6;
const BP_UNION: i32 = 6;
const BP_MULTIPLY: i32 = 7;
const BP_DIVIDE: i32 = 7;
const BP_NOT: i32 = 8;
const BP_UNARY_MINUS: i32 = 8;
const BP_FLATTEN: i32 = 10;
const BP_STAR: i32 = 20;
const BP_FILTER: i32 = 21;
const BP_DOT: i32 = 40;
const BP_LBRACE: i32 = 50;
const BP_LBRACKET: i32 = 55;
const BP_LPAREN: i32 = 60;

fn binding_power(kind: &TokenKind) -> i32 {
    match kind {
        TokenKind::Eof => BP_EOF,
        TokenKind::Identifier => BP_IDENTIFIER,
        TokenKind::QuotedIdentifier => BP_QUOTED_IDENTIFIER,
        TokenKind::Rbracket => BP_RBRACKET,
        TokenKind::Rparen => BP_RPAREN,
        TokenKind::Comma => BP_COMMA,
        TokenKind::Rbrace => BP_RBRACE,
        TokenKind::Number => BP_NUMBER,
        TokenKind::Int => BP_INT,
        TokenKind::Current => BP_CURRENT,
        TokenKind::Global => BP_GLOBAL,
        TokenKind::Expref => BP_EXPREF,
        TokenKind::Pipe => BP_PIPE,
        TokenKind::Or => BP_OR,
        TokenKind::And => BP_AND,
        TokenKind::Comparator => BP_COMPARATOR,
        TokenKind::Concatenate => BP_CONCATENATE,
        TokenKind::Add => BP_ADD,
        TokenKind::Subtract => BP_SUBTRACT,
        TokenKind::Union => BP_UNION,
        TokenKind::Multiply => BP_MULTIPLY,
        TokenKind::Divide => BP_DIVIDE,
        TokenKind::Not => BP_NOT,
        TokenKind::UnaryMinus => BP_UNARY_MINUS,
        TokenKind::Flatten => BP_FLATTEN,
        TokenKind::Star => BP_STAR,
        TokenKind::Filter => BP_FILTER,
        TokenKind::Dot => BP_DOT,
        TokenKind::Lbrace => BP_LBRACE,
        TokenKind::Lbracket => BP_LBRACKET,
        TokenKind::Lparen => BP_LPAREN,
        TokenKind::Colon => BP_COLON,
        TokenKind::String => 0,
        TokenKind::Json => 0,
    }
}

pub struct Parser<'a> {
    allowed_globals: &'a [String],
    tokens: Vec<Token>,
    index: usize,
    debug: &'a mut Vec<String>,
}

impl<'a> Parser<'a> {
    pub fn new(allowed_globals: &'a [String], debug: &'a mut Vec<String>) -> Self {
        Self {
            allowed_globals,
            tokens: Vec::new(),
            index: 0,
            debug,
        }
    }

    pub fn parse(&mut self, expression: &str) -> Result<AstNode, JsonFormulaError> {
        self.load_tokens(expression)?;
        self.index = 0;
        let ast = self.expression(0)?;
        if self.lookahead(0) != TokenKind::Eof {
            let t = self.lookahead_token(0)?;
            return Err(JsonFormulaError::syntax(format!(
                "Unexpected token type: {:?}",
                t.kind
            )));
        }
        Ok(ast)
    }

    fn load_tokens(&mut self, expression: &str) -> Result<(), JsonFormulaError> {
        let mut lexer = Lexer::new(self.allowed_globals, self.debug);
        let mut tokens = lexer.tokenize(expression)?;
        tokens.push(Token {
            kind: TokenKind::Eof,
            value: Some(TokenValue::String(String::new())),
            start: expression.len(),
            name: None,
        });
        self.tokens = tokens;
        Ok(())
    }

    fn expression(&mut self, rbp: i32) -> Result<AstNode, JsonFormulaError> {
        let left_token = self.lookahead_token(0)?;
        self.advance();
        let mut left = self.nud(left_token)?;
        let mut current_token = self.lookahead_token_with_prev(0, Some(&left))?;
        while rbp < binding_power(&current_token.kind) {
            self.advance();
            left = self.led(current_token, left)?;
            current_token = self.lookahead_token_with_prev(0, Some(&left))?;
        }
        Ok(left)
    }

    fn lookahead(&self, number: usize) -> TokenKind {
        self.tokens[self.index + number].kind.clone()
    }

    fn lookahead_token(&self, number: usize) -> Result<Token, JsonFormulaError> {
        Ok(self.tokens[self.index + number].clone())
    }

    fn lookahead_token_with_prev(
        &self,
        number: usize,
        previous: Option<&AstNode>,
    ) -> Result<Token, JsonFormulaError> {
        let mut next = self.tokens[self.index + number].clone();
        if next.kind == TokenKind::Star {
            if matches!(
                previous,
                Some(AstNode::ArrayExpression(_)) | Some(AstNode::ObjectExpression(_))
            ) {
                next.kind = TokenKind::Multiply;
                return Ok(next);
            }
            let prev_kind = previous.map(node_kind);
            if !matches!(
                prev_kind,
                None
                    | Some(TokenKind::Lbracket)
                    | Some(TokenKind::Dot)
                    | Some(TokenKind::Pipe)
                    | Some(TokenKind::And)
                    | Some(TokenKind::Or)
                    | Some(TokenKind::Comma)
                    | Some(TokenKind::Not)
                    | Some(TokenKind::Lparen)
            ) {
                next.kind = TokenKind::Multiply;
            }
        }
        Ok(next)
    }

    fn advance(&mut self) {
        self.index += 1;
    }

    fn lookahead_index(&self) -> bool {
        let mut idx = 0;
        if self.lookahead(idx) == TokenKind::UnaryMinus {
            idx += 1;
        }
        if self.lookahead(idx) == TokenKind::Int {
            idx += 1;
        }
        matches!(self.lookahead(idx), TokenKind::Rbracket | TokenKind::Colon)
    }

    fn nud(&mut self, token: Token) -> Result<AstNode, JsonFormulaError> {
        match token.kind {
            TokenKind::String => match token.value {
                Some(TokenValue::String(value)) => Ok(AstNode::String(value)),
                _ => Err(JsonFormulaError::syntax("Invalid string token")),
            },
            TokenKind::Json => match token.value {
                Some(TokenValue::Json(value)) => Ok(AstNode::Literal(value)),
                _ => Err(JsonFormulaError::syntax("Invalid literal token")),
            },
            TokenKind::Number => match token.value {
                Some(TokenValue::Number(value)) => Ok(AstNode::Number(value)),
                _ => Err(JsonFormulaError::syntax("Invalid number token")),
            },
            TokenKind::Int => match token.value {
                Some(TokenValue::Integer(value)) => Ok(AstNode::Integer(value)),
                _ => Err(JsonFormulaError::syntax("Invalid integer token")),
            },
            TokenKind::Identifier => match token.value {
                Some(TokenValue::String(value)) => Ok(AstNode::Identifier(value)),
                _ => Err(JsonFormulaError::syntax("Invalid identifier token")),
            },
            TokenKind::QuotedIdentifier => match token.value {
                Some(TokenValue::String(value)) => Ok(AstNode::QuotedIdentifier(value)),
                _ => Err(JsonFormulaError::syntax("Invalid quoted identifier token")),
            },
            TokenKind::Not => {
                let right = self.expression(BP_NOT)?;
                Ok(AstNode::NotExpression(Box::new(right)))
            }
            TokenKind::UnaryMinus => {
                let right = self.expression(BP_UNARY_MINUS)?;
                Ok(AstNode::UnaryMinusExpression(Box::new(right)))
            }
            TokenKind::Star => {
                let left = AstNode::Identity;
                let next = self.lookahead(0);
                let right = if matches!(
                    next,
                    TokenKind::Rbracket
                        | TokenKind::Rparen
                        | TokenKind::Rbrace
                        | TokenKind::Eof
                        | TokenKind::Comma
                        | TokenKind::Pipe
                        | TokenKind::Or
                        | TokenKind::And
                        | TokenKind::Comparator
                        | TokenKind::Concatenate
                        | TokenKind::Add
                        | TokenKind::Subtract
                        | TokenKind::Star
                        | TokenKind::Multiply
                        | TokenKind::Divide
                        | TokenKind::Union
                        | TokenKind::Flatten
                        | TokenKind::Filter
                ) {
                    AstNode::Identity
                } else if matches!(next, TokenKind::Dot | TokenKind::Lbracket) {
                    self.parse_projection_rhs(BP_STAR)?
                } else {
                    return Err(JsonFormulaError::syntax(
                        "Invalid wildcard expression".to_string(),
                    ));
                };
                Ok(AstNode::ValueProjection {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            TokenKind::Filter => self.led(token, AstNode::Identity),
            TokenKind::Lbrace => self.parse_object_expression(),
            TokenKind::Flatten => {
                let left = AstNode::Flatten(Box::new(AstNode::Identity));
                let right = self.parse_projection_rhs(BP_FLATTEN)?;
                Ok(AstNode::Projection {
                    left: Box::new(left),
                    right: Box::new(right),
                    debug: None,
                })
            }
            TokenKind::Lbracket => {
                if self.lookahead_index() {
                    let right = self.parse_index_expression()?;
                    return self.project_if_slice(AstNode::Identity, right);
                }
                if self.lookahead(0) == TokenKind::Star && self.lookahead(1) == TokenKind::Rbracket
                {
                    self.advance();
                    self.advance();
                    let right = self.parse_projection_rhs(BP_STAR)?;
                    return Ok(AstNode::Projection {
                        left: Box::new(AstNode::Identity),
                        right: Box::new(right),
                        debug: Some("Wildcard".to_string()),
                    });
                }
                self.parse_array_expression()
            }
            TokenKind::Current => Ok(AstNode::Current),
            TokenKind::Global => Ok(AstNode::Global(token.name.unwrap_or_default())),
            TokenKind::Expref => {
                let expression = self.expression(BP_EXPREF)?;
                Ok(AstNode::ExpressionReference(Box::new(expression)))
            }
            TokenKind::Lparen => {
                let mut args = Vec::new();
                while self.lookahead(0) != TokenKind::Rparen {
                    let expr = self.expression(0)?;
                    args.push(expr);
                }
                self.match_kind(TokenKind::Rparen)?;
                Ok(args
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| AstNode::Identity))
            }
            _ => self.error_token(&token),
        }
    }

    fn led(&mut self, token: Token, left: AstNode) -> Result<AstNode, JsonFormulaError> {
        match token.kind {
            TokenKind::Concatenate => {
                let right = self.expression(BP_CONCATENATE)?;
                Ok(AstNode::ConcatenateExpression(
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::Dot => {
                let rbp = BP_DOT;
                if self.lookahead(0) != TokenKind::Star {
                    let right = self.parse_dot_rhs(rbp)?;
                    if let AstNode::ChainedExpression(mut nodes) = left {
                        nodes.push(right);
                        return Ok(AstNode::ChainedExpression(nodes));
                    }
                    return Ok(AstNode::ChainedExpression(vec![left, right]));
                }
                self.advance();
                let right = self.parse_projection_rhs(rbp)?;
                Ok(AstNode::ValueProjection {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            TokenKind::Pipe => {
                let right = self.expression(BP_PIPE)?;
                Ok(AstNode::Pipe(Box::new(left), Box::new(right)))
            }
            TokenKind::Or => {
                let right = self.expression(BP_OR)?;
                Ok(AstNode::OrExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::And => {
                let right = self.expression(BP_AND)?;
                Ok(AstNode::AndExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::Add => {
                let right = self.expression(BP_ADD)?;
                Ok(AstNode::AddExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::Subtract => {
                let right = self.expression(BP_SUBTRACT)?;
                Ok(AstNode::SubtractExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::Multiply => {
                let right = self.expression(BP_MULTIPLY)?;
                Ok(AstNode::MultiplyExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::Divide => {
                let right = self.expression(BP_DIVIDE)?;
                Ok(AstNode::DivideExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::Union => {
                let right = self.expression(BP_UNION)?;
                Ok(AstNode::UnionExpression(Box::new(left), Box::new(right)))
            }
            TokenKind::Lparen => {
                let name = match left {
                    AstNode::Identifier(name) => name,
                    _ => {
                        return Err(JsonFormulaError::syntax(
                            "Bad function syntax. Parenthesis must be preceded by an unquoted identifier",
                        ))
                    }
                };
                let args = self.parse_function_args()?;
                Ok(AstNode::Function { name, args })
            }
            TokenKind::Filter => {
                let condition = self.expression(0)?;
                self.match_kind(TokenKind::Rbracket)?;
                let right = self.parse_projection_rhs(BP_FILTER)?;
                Ok(AstNode::FilterProjection {
                    left: Box::new(left),
                    right: Box::new(right),
                    condition: Box::new(condition),
                })
            }
            TokenKind::Flatten => {
                let left_node = AstNode::Flatten(Box::new(left));
                let right_node = self.parse_projection_rhs(BP_FLATTEN)?;
                Ok(AstNode::Projection {
                    left: Box::new(left_node),
                    right: Box::new(right_node),
                    debug: None,
                })
            }
            TokenKind::Comparator => self.parse_comparator(left, token),
            TokenKind::Lbracket => {
                if self.lookahead(0) == TokenKind::Star && self.lookahead(1) == TokenKind::Rbracket
                {
                    self.advance();
                    self.advance();
                    let right = self.parse_projection_rhs(BP_STAR)?;
                    return Ok(AstNode::Projection {
                        left: Box::new(left),
                        right: Box::new(right),
                        debug: Some("Wildcard".to_string()),
                    });
                }
                let right = self.parse_index_expression()?;
                self.project_if_slice(left, right)
            }
            _ => self.error_token(&token),
        }
    }

    fn parse_function_args(&mut self) -> Result<Vec<AstNode>, JsonFormulaError> {
        let mut args = Vec::new();
        if self.lookahead(0) == TokenKind::Rparen {
            self.advance();
            return Ok(args);
        }
        loop {
            let expr = self.expression(0)?;
            args.push(expr);
            if self.lookahead(0) == TokenKind::Comma {
                self.advance();
                continue;
            }
            break;
        }
        self.match_kind(TokenKind::Rparen)?;
        Ok(args)
    }

    fn parse_index_expression(&mut self) -> Result<AstNode, JsonFormulaError> {
        if self.lookahead(0) == TokenKind::Colon || self.lookahead(0) == TokenKind::Rbracket {
            return self.parse_slice_expression();
        }

        let expression = self.expression(0)?;
        if self.lookahead(0) == TokenKind::Colon {
            self.advance();
            let start = match expression {
                AstNode::Integer(value) => Some(value),
                AstNode::UnaryMinusExpression(inner) => match *inner {
                    AstNode::Integer(value) => Some(-value),
                    _ => {
                        return Err(JsonFormulaError::syntax(
                            "Slice expressions must be integers",
                        ))
                    }
                },
                _ => {
                    return Err(JsonFormulaError::syntax(
                        "Slice expressions must be integers",
                    ))
                }
            };
            return self.parse_slice_expression_with_parts(vec![start]);
        }
        self.match_kind(TokenKind::Rbracket)?;
        Ok(AstNode::Index(Box::new(expression)))
    }

    fn parse_slice_expression(&mut self) -> Result<AstNode, JsonFormulaError> {
        self.parse_slice_expression_with_parts(Vec::new())
    }

    fn parse_slice_expression_with_parts(
        &mut self,
        mut parts: Vec<Option<i64>>,
    ) -> Result<AstNode, JsonFormulaError> {
        let mut expect_value = true;
        let mut saw_colon = !parts.is_empty();
        while self.lookahead(0) != TokenKind::Rbracket {
            if self.lookahead(0) == TokenKind::Colon {
                if expect_value {
                    parts.push(None);
                }
                self.advance();
                expect_value = true;
                saw_colon = true;
                continue;
            } else {
                let expr = self.expression(0)?;
                if let AstNode::Integer(value) = expr {
                    parts.push(Some(value));
                } else if let AstNode::UnaryMinusExpression(inner) = expr {
                    if let AstNode::Integer(value) = *inner {
                        parts.push(Some(-value));
                    } else {
                        return Err(JsonFormulaError::syntax(
                            "Slice expressions must be integers",
                        ));
                    }
                } else {
                    return Err(JsonFormulaError::syntax(
                        "Slice expressions must be integers",
                    ));
                }
            }
            expect_value = false;
        }
        if expect_value && saw_colon {
            parts.push(None);
        }
        self.match_kind(TokenKind::Rbracket)?;
        if parts.len() > 3 {
            return Err(JsonFormulaError::syntax(
                "Slice expressions must have at most 3 parts".to_string(),
            ));
        }

        let start = parts.get(0).copied().flatten();
        let stop = parts.get(1).copied().flatten();
        let step = parts.get(2).copied().flatten();
        Ok(AstNode::Slice { start, stop, step })
    }

    fn parse_comparator(&mut self, left: AstNode, token: Token) -> Result<AstNode, JsonFormulaError> {
        let right = self.expression(BP_COMPARATOR)?;
        let op = match token.value {
            Some(TokenValue::Comparator(value)) => value,
            Some(TokenValue::String(value)) => value,
            _ => "==".to_string(),
        };
        Ok(AstNode::Comparator {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_projection_rhs(&mut self, rbp: i32) -> Result<AstNode, JsonFormulaError> {
        let next = self.lookahead_token_with_prev(0, None)?;
        let right = if matches!(
            next.kind,
            TokenKind::Rbracket
                | TokenKind::Rparen
                | TokenKind::Eof
                | TokenKind::Comma
                | TokenKind::Pipe
                | TokenKind::Or
                | TokenKind::And
                | TokenKind::Comparator
                | TokenKind::Concatenate
                | TokenKind::Add
                | TokenKind::Subtract
                | TokenKind::Multiply
                | TokenKind::Divide
                | TokenKind::Union
                | TokenKind::Flatten
        ) {
            AstNode::Identity
        } else if matches!(
            next.kind,
            TokenKind::Identifier
                | TokenKind::QuotedIdentifier
                | TokenKind::String
                | TokenKind::Number
                | TokenKind::Int
                | TokenKind::Json
                | TokenKind::UnaryMinus
                | TokenKind::Star
        ) {
            return Err(JsonFormulaError::syntax(
                "Projection expressions must be followed by a dot or bracket".to_string(),
            ));
        } else if next.kind == TokenKind::Dot {
            self.advance();
            let rhs = self.parse_dot_rhs(rbp)?;
            AstNode::ChainedExpression(vec![AstNode::Identity, rhs])
        } else {
            self.expression(rbp)?
        };
        Ok(right)
    }

    fn parse_dot_rhs(&mut self, rbp: i32) -> Result<AstNode, JsonFormulaError> {
        if self.lookahead(0) == TokenKind::Lbrace {
            return self.parse_multi_select_hash();
        }
        if self.lookahead(0) == TokenKind::Star {
            self.advance();
            let right = self.parse_projection_rhs(rbp)?;
            return Ok(AstNode::ValueProjection {
                left: Box::new(AstNode::Identity),
                right: Box::new(right),
            });
        }
        if self.lookahead(0) == TokenKind::Lbracket {
            return self.parse_multi_select_list();
        }
        let expression = self.expression(rbp)?;
        if matches!(
            expression,
            AstNode::Literal(_)
                | AstNode::String(_)
                | AstNode::Number(_)
                | AstNode::Integer(_)
                | AstNode::UnaryMinusExpression(_)
        ) {
            return Err(JsonFormulaError::syntax(
                "Literal expressions are not allowed on the right side of a dot".to_string(),
            ));
        }
        Ok(expression)
    }

    fn parse_multi_select_list(&mut self) -> Result<AstNode, JsonFormulaError> {
        self.match_kind(TokenKind::Lbracket)?;
        let mut expressions = Vec::new();
        while self.lookahead(0) != TokenKind::Rbracket {
            expressions.push(self.expression(0)?);
            if self.lookahead(0) == TokenKind::Comma {
                self.advance();
                if self.lookahead(0) == TokenKind::Rbracket {
                    return Err(JsonFormulaError::syntax(
                        "Trailing commas are not allowed in list expressions".to_string(),
                    ));
                }
            }
        }
        self.match_kind(TokenKind::Rbracket)?;
        Ok(AstNode::ArrayExpression(expressions))
    }

    fn parse_multi_select_hash(&mut self) -> Result<AstNode, JsonFormulaError> {
        self.match_kind(TokenKind::Lbrace)?;
        if self.lookahead(0) == TokenKind::Rbrace {
            return Err(JsonFormulaError::syntax(
                "Empty object expressions are not allowed".to_string(),
            ));
        }
        let mut pairs = Vec::new();
        while self.lookahead(0) != TokenKind::Rbrace {
            let key_token = self.lookahead_token(0)?;
            let key = match key_token.kind {
                TokenKind::Identifier | TokenKind::QuotedIdentifier => {
                    self.advance();
                    match key_token.value {
                        Some(TokenValue::String(value)) => value,
                        _ => {
                            return Err(JsonFormulaError::syntax(
                                "Invalid key in object expression",
                            ))
                        }
                    }
                }
                _ => {
                    return Err(JsonFormulaError::syntax(
                        "Object key must be an identifier or quoted identifier",
                    ))
                }
            };
            self.match_kind(TokenKind::Colon)?;
            let value = self.expression(0)?;
            pairs.push(KeyValuePair { key, value });
            if self.lookahead(0) == TokenKind::Comma {
                self.advance();
                if self.lookahead(0) == TokenKind::Rbrace {
                    return Err(JsonFormulaError::syntax(
                        "Trailing commas are not allowed in object expressions".to_string(),
                    ));
                }
            }
        }
        self.match_kind(TokenKind::Rbrace)?;
        Ok(AstNode::ObjectExpression(pairs))
    }

    fn parse_array_expression(&mut self) -> Result<AstNode, JsonFormulaError> {
        let mut expressions = Vec::new();
        while self.lookahead(0) != TokenKind::Rbracket {
            expressions.push(self.expression(0)?);
            if self.lookahead(0) == TokenKind::Comma {
                self.advance();
                if self.lookahead(0) == TokenKind::Rbracket {
                    return Err(JsonFormulaError::syntax(
                        "Trailing commas are not allowed in array expressions".to_string(),
                    ));
                }
            }
        }
        self.match_kind(TokenKind::Rbracket)?;
        Ok(AstNode::ArrayExpression(expressions))
    }

    fn parse_object_expression(&mut self) -> Result<AstNode, JsonFormulaError> {
        if self.lookahead(0) == TokenKind::Rbrace {
            return Err(JsonFormulaError::syntax(
                "Empty object expressions are not allowed".to_string(),
            ));
        }
        let mut pairs = Vec::new();
        while self.lookahead(0) != TokenKind::Rbrace {
            let key_token = self.lookahead_token(0)?;
            let key = match key_token.kind {
                TokenKind::Identifier | TokenKind::QuotedIdentifier => {
                    self.advance();
                    match key_token.value {
                        Some(TokenValue::String(value)) => value,
                        _ => {
                            return Err(JsonFormulaError::syntax(
                                "Invalid key in object expression",
                            ))
                        }
                    }
                }
                _ => {
                    return Err(JsonFormulaError::syntax(
                        "Object key must be an identifier or quoted identifier",
                    ))
                }
            };
            self.match_kind(TokenKind::Colon)?;
            let value = self.expression(0)?;
            pairs.push(KeyValuePair { key, value });
            if self.lookahead(0) == TokenKind::Comma {
                self.advance();
                if self.lookahead(0) == TokenKind::Rbrace {
                    return Err(JsonFormulaError::syntax(
                        "Trailing commas are not allowed in object expressions".to_string(),
                    ));
                }
            }
        }
        self.match_kind(TokenKind::Rbrace)?;
        Ok(AstNode::ObjectExpression(pairs))
    }

    fn project_if_slice(&mut self, left: AstNode, right: AstNode) -> Result<AstNode, JsonFormulaError> {
        if matches!(right, AstNode::Slice { .. }) {
            let projection = AstNode::Projection {
                left: Box::new(left),
                right: Box::new(AstNode::Identity),
                debug: None,
            };
            Ok(AstNode::BracketExpression(
                Box::new(projection),
                Box::new(right),
            ))
        } else {
            Ok(AstNode::BracketExpression(Box::new(left), Box::new(right)))
        }
    }

    fn match_kind(&mut self, kind: TokenKind) -> Result<(), JsonFormulaError> {
        if self.lookahead(0) == kind {
            self.advance();
            Ok(())
        } else {
            let t = self.lookahead_token(0)?;
            Err(JsonFormulaError::syntax(format!(
                "Expected token {:?}, got {:?}",
                kind, t.kind
            )))
        }
    }

    fn error_token(&self, token: &Token) -> Result<AstNode, JsonFormulaError> {
        Err(JsonFormulaError::syntax(format!(
            "Unexpected token type: {:?}, value: {:?}",
            token.kind, token.value
        )))
    }
}

fn node_kind(node: &AstNode) -> TokenKind {
    match node {
        AstNode::Identifier(_) => TokenKind::Identifier,
        AstNode::QuotedIdentifier(_) => TokenKind::QuotedIdentifier,
        AstNode::Literal(_) => TokenKind::Json,
        AstNode::String(_) => TokenKind::String,
        AstNode::Number(_) => TokenKind::Number,
        AstNode::Integer(_) => TokenKind::Int,
        AstNode::Current => TokenKind::Current,
        AstNode::Global(_) => TokenKind::Global,
        AstNode::ExpressionReference(_) => TokenKind::Expref,
        AstNode::NotExpression(_) => TokenKind::Not,
        AstNode::UnaryMinusExpression(_) => TokenKind::UnaryMinus,
        AstNode::ConcatenateExpression(_, _) => TokenKind::Concatenate,
        AstNode::OrExpression(_, _) => TokenKind::Or,
        AstNode::AndExpression(_, _) => TokenKind::And,
        AstNode::AddExpression(_, _) => TokenKind::Add,
        AstNode::SubtractExpression(_, _) => TokenKind::Subtract,
        AstNode::MultiplyExpression(_, _) => TokenKind::Multiply,
        AstNode::DivideExpression(_, _) => TokenKind::Divide,
        AstNode::UnionExpression(_, _) => TokenKind::Union,
        AstNode::Comparator { .. } => TokenKind::Comparator,
        AstNode::Pipe(_, _) => TokenKind::Pipe,
        AstNode::ChainedExpression(_) => TokenKind::Dot,
        AstNode::BracketExpression(_, _) => TokenKind::Identifier,
        AstNode::Index(_) => TokenKind::Identifier,
        AstNode::Slice { .. } => TokenKind::Identifier,
        AstNode::Projection { .. } => TokenKind::Star,
        AstNode::ValueProjection { .. } => TokenKind::Star,
        AstNode::FilterProjection { .. } => TokenKind::Filter,
        AstNode::Flatten(_) => TokenKind::Flatten,
        AstNode::Function { .. } => TokenKind::Lparen,
        AstNode::ArrayExpression(_) => TokenKind::Lbracket,
        AstNode::ObjectExpression(_) => TokenKind::Lbrace,
        AstNode::KeyValuePair { .. } => TokenKind::Lbrace,
        AstNode::Identity => TokenKind::Identifier,
    }
}
