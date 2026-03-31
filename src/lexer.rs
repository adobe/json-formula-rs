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

use crate::errors::JsonFormulaError;
use crate::errors::JsonFormulaErrorKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Eof,
    Identifier,
    QuotedIdentifier,
    Rbracket,
    Rparen,
    Comma,
    Colon,
    Concatenate,
    Rbrace,
    Number,
    Current,
    Global,
    Expref,
    Pipe,
    Or,
    And,
    Add,
    Subtract,
    UnaryMinus,
    Multiply,
    Union,
    Divide,
    Comparator,
    Flatten,
    Star,
    Filter,
    Dot,
    Not,
    Lbrace,
    Lbracket,
    Lparen,
    Json,
    String,
    Int,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub value: Option<TokenValue>,
    pub start: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    String(String),
    Number(f64),
    Integer(i64),
    Json(serde_json::Value),
    Comparator(String),
}

pub struct Lexer<'a> {
    allowed_globals: &'a [String],
    debug: &'a mut Vec<String>,
    current: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(allowed_globals: &'a [String], debug: &'a mut Vec<String>) -> Self {
        Self {
            allowed_globals,
            debug,
            current: 0,
        }
    }

    pub fn tokenize(&mut self, stream: &str) -> Result<Vec<Token>, JsonFormulaError> {
        let mut tokens = Vec::new();
        self.current = 0;
        let bytes = stream.as_bytes();
        while self.current < bytes.len() {
            let prev: Option<TokenKind> = tokens.last().map(|t: &Token| t.kind.clone());
            if self.is_global(prev.as_ref(), stream, self.current) {
                tokens.push(self.consume_global(stream));
                continue;
            }

            let ch = bytes[self.current] as char;
            if is_identifier(stream, self.current) {
                let start = self.current;
                let identifier = self.consume_unquoted_identifier(stream);
                tokens.push(Token {
                    kind: TokenKind::Identifier,
                    value: Some(TokenValue::String(identifier)),
                    start,
                    name: None,
                });
                continue;
            }

            if self.is_number(stream) {
                tokens.push(self.consume_number(stream)?);
                continue;
            }

            if let Some(kind) = basic_token_kind(ch) {
                tokens.push(Token {
                    kind,
                    value: Some(TokenValue::String(ch.to_string())),
                    start: self.current,
                    name: None,
                });
                self.current += 1;
                continue;
            }

            if ch == '-' && !matches!(
                prev,
                Some(TokenKind::Global)
                    | Some(TokenKind::Current)
                    | Some(TokenKind::Number)
                    | Some(TokenKind::Int)
                    | Some(TokenKind::Rparen)
                    | Some(TokenKind::Identifier)
                    | Some(TokenKind::QuotedIdentifier)
                    | Some(TokenKind::Rbracket)
                    | Some(TokenKind::Json)
                    | Some(TokenKind::String)
            ) {
                tokens.push(self.consume_unary_minus());
                continue;
            }

            if ch == '[' {
                tokens.push(self.consume_lbracket(stream));
                continue;
            }

            if ch == '\'' {
                let start = self.current;
                let identifier = self.consume_quoted_identifier(stream)?;
                tokens.push(Token {
                    kind: TokenKind::QuotedIdentifier,
                    value: Some(TokenValue::String(identifier)),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '"' {
                let start = self.current;
                let literal = self.consume_raw_string_literal(stream)?;
                tokens.push(Token {
                    kind: TokenKind::String,
                    value: Some(TokenValue::String(literal)),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '`' {
                let start = self.current;
                let json = self.consume_json(stream)?;
                tokens.push(Token {
                    kind: TokenKind::Json,
                    value: Some(TokenValue::Json(json)),
                    start,
                    name: None,
                });
                continue;
            }

            if is_operator_start(ch) {
                tokens.push(self.consume_operator(stream)?);
                continue;
            }

            if is_skip_char(ch) {
                self.current += 1;
                continue;
            }

            if ch == '&' {
                let start = self.current;
                self.current += 1;
                if self.current < bytes.len() && bytes[self.current] as char == '&' {
                    self.current += 1;
                    tokens.push(Token {
                        kind: TokenKind::And,
                        value: Some(TokenValue::String("&&".to_string())),
                        start,
                        name: None,
                    });
                } else if matches!(prev, Some(TokenKind::Comma) | Some(TokenKind::Lparen)) {
                    tokens.push(Token {
                        kind: TokenKind::Expref,
                        value: Some(TokenValue::String("&".to_string())),
                        start,
                        name: None,
                    });
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Concatenate,
                        value: Some(TokenValue::String("&".to_string())),
                        start,
                        name: None,
                    });
                }
                continue;
            }

            if ch == '~' {
                let start = self.current;
                self.current += 1;
                tokens.push(Token {
                    kind: TokenKind::Union,
                    value: Some(TokenValue::String("~".to_string())),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '+' {
                let start = self.current;
                self.current += 1;
                tokens.push(Token {
                    kind: TokenKind::Add,
                    value: Some(TokenValue::String("+".to_string())),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '-' {
                let start = self.current;
                self.current += 1;
                tokens.push(Token {
                    kind: TokenKind::Subtract,
                    value: Some(TokenValue::String("-".to_string())),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '*' {
                let start = self.current;
                self.current += 1;
                tokens.push(Token {
                    kind: TokenKind::Star,
                    value: Some(TokenValue::String("*".to_string())),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '/' {
                let start = self.current;
                self.current += 1;
                tokens.push(Token {
                    kind: TokenKind::Divide,
                    value: Some(TokenValue::String("/".to_string())),
                    start,
                    name: None,
                });
                continue;
            }

            if ch == '|' {
                let start = self.current;
                self.current += 1;
                if self.current < bytes.len() && bytes[self.current] as char == '|' {
                    self.current += 1;
                    tokens.push(Token {
                        kind: TokenKind::Or,
                        value: Some(TokenValue::String("||".to_string())),
                        start,
                        name: None,
                    });
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Pipe,
                        value: Some(TokenValue::String("|".to_string())),
                        start,
                        name: None,
                    });
                }
                continue;
            }

            throw_syntax(&format!("Unknown character:{}", ch))?;
        }

        Ok(tokens)
    }

    fn consume_unquoted_identifier(&mut self, stream: &str) -> String {
        let start = self.current;
        self.current += 1;
        while self.current < stream.len()
            && (stream.as_bytes()[self.current] as char == '$'
                || is_alnum(stream.as_bytes()[self.current] as char))
        {
            self.current += 1;
        }
        stream[start..self.current].to_string()
    }

    fn consume_quoted_identifier(&mut self, stream: &str) -> Result<String, JsonFormulaError> {
        let start = self.current;
        self.current += 1;
        let max = stream.len();
        let mut found_non_alpha = !is_identifier(stream, start + 1);
        while self.current < max && stream.as_bytes()[self.current] as char != '\'' {
            let mut current = self.current;
            let ch = stream.as_bytes()[current] as char;
            if !is_alnum(ch) {
                found_non_alpha = true;
            }
            if ch == '\\' {
                let next = stream.as_bytes().get(current + 1).copied().unwrap_or(b'\0') as char;
                if next == '\\' || next == '\'' {
                    current += 2;
                } else {
                    current += 1;
                }
            } else {
                current += 1;
            }
            self.current = current;
        }
        self.current += 1;
        let val = stream[start..self.current].to_string();

        if !found_non_alpha {
            self.debug.push(format!("Suspicious quotes: {}", val));
            self.debug.push(format!(
                "Did you intend a literal? \"{}\"?",
                val.replace('\'', "")
            ));
        }

        let inner = &val[1..val.len().saturating_sub(1)];
        let replaced = inner.replace("\\'", "'");
        let json = format!("\"{}\"", replaced);
        let parsed: String = serde_json::from_str(&json).map_err(|_| {
            JsonFormulaError::syntax(format!("Invalid quoted identifier: {}", val))
        })?;
        Ok(parsed)
    }

    fn consume_raw_string_literal(&mut self, stream: &str) -> Result<String, JsonFormulaError> {
        let start = self.current;
        self.current += 1;
        let max = stream.len();
        while self.current < max && stream.as_bytes()[self.current] as char != '"' {
            let mut current = self.current;
            let ch = stream.as_bytes()[current] as char;
            if ch == '\\' {
                let next = stream.as_bytes().get(current + 1).copied().unwrap_or(b'\0') as char;
                if next == '\\' || next == '"' {
                    current += 2;
                } else {
                    current += 1;
                }
            } else {
                current += 1;
            }
            self.current = current;
        }
        self.current += 1;
        let literal = stream[start + 1..self.current - 1].to_string();
        if self.current > max {
            return Err(JsonFormulaError::syntax(format!(
                "Unterminated string literal at {}, \"{}",
                start, literal
            )));
        }
        let json = format!("\"{}\"", literal);
        let parsed: String = serde_json::from_str(&json)
            .map_err(|_| JsonFormulaError::syntax(format!("Invalid string literal: {}", literal)))?;
        Ok(parsed)
    }

    fn is_number(&self, stream: &str) -> bool {
        let bytes = stream.as_bytes();
        let ch = bytes[self.current] as char;
        if ch.is_ascii_digit() {
            return true;
        }
        if ch != '.' {
            return false;
        }
        if self.current == bytes.len() - 1 {
            return false;
        }
        let next = bytes[self.current + 1] as char;
        next.is_ascii_digit()
    }

    fn consume_number(&mut self, stream: &str) -> Result<Token, JsonFormulaError> {
        let start = self.current;
        let slice = &stream[start..];
        let re = regex::Regex::new(r"^[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?")
            .expect("regex should compile");
        let caps = re
            .find(slice)
            .ok_or_else(|| JsonFormulaError::syntax(format!("Invalid number: {}", slice)))?;
        let n = caps.as_str();
        self.current += n.len();
        if n.contains('.') || n.to_ascii_lowercase().contains('e') {
            let value: f64 = n.parse().map_err(|_| {
                JsonFormulaError::syntax(format!("Invalid number: {}", n))
            })?;
            Ok(Token {
                kind: TokenKind::Number,
                value: Some(TokenValue::Number(value)),
                start,
                name: None,
            })
        } else {
            let value: i64 = n.parse().map_err(|_| {
                JsonFormulaError::syntax(format!("Invalid number: {}", n))
            })?;
            Ok(Token {
                kind: TokenKind::Int,
                value: Some(TokenValue::Integer(value)),
                start,
                name: None,
            })
        }
    }

    fn consume_unary_minus(&mut self) -> Token {
        let start = self.current;
        self.current += 1;
        Token {
            kind: TokenKind::UnaryMinus,
            value: Some(TokenValue::String("-".to_string())),
            start,
            name: None,
        }
    }

    fn consume_lbracket(&mut self, stream: &str) -> Token {
        let start = self.current;
        self.current += 1;
        let next = stream.as_bytes().get(self.current).copied().unwrap_or(b'\0') as char;
        if next == '?' {
            self.current += 1;
            return Token {
                kind: TokenKind::Filter,
                value: Some(TokenValue::String("[?".to_string())),
                start,
                name: None,
            };
        }
        if next == ']' {
            self.current += 1;
            return Token {
                kind: TokenKind::Flatten,
                value: Some(TokenValue::String("[]".to_string())),
                start,
                name: None,
            };
        }
        Token {
            kind: TokenKind::Lbracket,
            value: Some(TokenValue::String("[".to_string())),
            start,
            name: None,
        }
    }

    fn is_global(&self, prev: Option<&TokenKind>, stream: &str, pos: usize) -> bool {
        if matches!(prev, Some(TokenKind::Dot)) {
            return false;
        }
        let ch = stream.as_bytes()[pos] as char;
        if ch != '$' {
            return false;
        }
        let mut i = pos + 1;
        while i < stream.len() {
            let ch = stream.as_bytes()[i] as char;
            if ch == '$' || is_alnum(ch) {
                i += 1;
            } else {
                break;
            }
        }
        let global = &stream[pos..i];
        if self.allowed_globals.is_empty() {
            return true;
        }
        self.allowed_globals.iter().any(|g| g == global)
    }

    fn consume_global(&mut self, stream: &str) -> Token {
        let start = self.current;
        self.current += 1;
        while self.current < stream.len() {
            let ch = stream.as_bytes()[self.current] as char;
            if ch == '$' || is_alnum(ch) {
                self.current += 1;
            } else {
                break;
            }
        }
        let global = stream[start..self.current].to_string();
        Token {
            kind: TokenKind::Global,
            value: None,
            start,
            name: Some(global),
        }
    }

    fn consume_operator(&mut self, stream: &str) -> Result<Token, JsonFormulaError> {
        let start = self.current;
        let starting_char = stream.as_bytes()[start] as char;
        self.current += 1;
        let next = stream.as_bytes().get(self.current).copied().unwrap_or(b'\0') as char;
        let token = match starting_char {
            '!' => {
                if next == '=' {
                    self.current += 1;
                    TokenValue::Comparator("!=".to_string())
                } else {
                    return Ok(Token {
                        kind: TokenKind::Not,
                        value: Some(TokenValue::String("!".to_string())),
                        start,
                        name: None,
                    });
                }
            }
            '<' => {
                if next == '=' {
                    self.current += 1;
                    TokenValue::Comparator("<=".to_string())
                } else if next == '>' {
                    self.current += 1;
                    TokenValue::Comparator("!=".to_string())
                } else {
                    TokenValue::Comparator("<".to_string())
                }
            }
            '>' => {
                if next == '=' {
                    self.current += 1;
                    TokenValue::Comparator(">=".to_string())
                } else {
                    TokenValue::Comparator(">".to_string())
                }
            }
            '=' => {
                if next == '=' {
                    self.current += 1;
                }
                TokenValue::Comparator("==".to_string())
            }
            _ => {
                return Err(JsonFormulaError::syntax(format!(
                    "Unknown operator: {}",
                    starting_char
                )))
            }
        };

        Ok(Token {
            kind: TokenKind::Comparator,
            value: Some(token),
            start,
            name: None,
        })
    }

    fn consume_json(&mut self, stream: &str) -> Result<serde_json::Value, JsonFormulaError> {
        self.current += 1;
        let start = self.current;
        let max = stream.len();
        while self.current < max && stream.as_bytes()[self.current] as char != '`' {
            let mut current = self.current;
            let ch = stream.as_bytes()[current] as char;
            if ch == '\\' {
                let next = stream.as_bytes().get(current + 1).copied().unwrap_or(b'\0') as char;
                if next == '`' {
                    current += 2;
                } else {
                    current += 1;
                }
            } else {
                current += 1;
            }
            self.current = current;
        }
        let mut literal = stream[start..self.current].trim_start().to_string();
        literal = literal.replace("\\`", "`");
        self.current += 1;
        if self.current > max {
            return Err(JsonFormulaError::syntax(format!(
                "Unterminated JSON literal at {}: `{}`",
                start, literal
            )));
        }
        serde_json::from_str(&literal)
            .map_err(|_| JsonFormulaError::syntax(format!("Invalid JSON literal: {}", literal)))
    }
}

fn is_operator_start(ch: char) -> bool {
    matches!(ch, '<' | '>' | '=' | '!')
}

fn is_skip_char(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n')
}

fn is_alnum(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_identifier(stream: &str, pos: usize) -> bool {
    let ch = stream.as_bytes()[pos] as char;
    ch == '$' || ch.is_ascii_alphabetic() || ch == '_'
}

fn basic_token_kind(ch: char) -> Option<TokenKind> {
    match ch {
        '.' => Some(TokenKind::Dot),
        ',' => Some(TokenKind::Comma),
        ':' => Some(TokenKind::Colon),
        '{' => Some(TokenKind::Lbrace),
        '}' => Some(TokenKind::Rbrace),
        ']' => Some(TokenKind::Rbracket),
        '(' => Some(TokenKind::Lparen),
        ')' => Some(TokenKind::Rparen),
        '@' => Some(TokenKind::Current),
        _ => None,
    }
}

fn throw_syntax(msg: &str) -> Result<(), JsonFormulaError> {
    Err(JsonFormulaError {
        kind: JsonFormulaErrorKind::SyntaxError,
        message: msg.to_string(),
    })
}
