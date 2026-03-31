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

use crate::ast::AstNode;
use crate::errors::JsonFormulaError;
use crate::runtime::Runtime;
use crate::types::{get_type, DataType, JfValue};
use crate::utils::{
    debug_available, get_property, get_value_of, strict_deep_equal, to_boolean,
};

pub struct Interpreter {
    runtime: *mut Runtime,
    globals: Option<JfValue>,
    pub language: String,
    debug: *mut Vec<String>,
    debug_chain_start: Option<String>,
}

impl Interpreter {
    pub fn new(
        runtime: &mut Runtime,
        globals: Option<JfValue>,
        language: &str,
        debug: &mut Vec<String>,
    ) -> Self {
        Self {
            runtime,
            globals,
            language: language.to_string(),
            debug,
            debug_chain_start: None,
        }
    }

    pub fn search(&mut self, node: &AstNode, value: &JfValue) -> Result<JfValue, JsonFormulaError> {
        self.visit(node, value)
    }

    pub fn visit(&mut self, node: &AstNode, value: &JfValue) -> Result<JfValue, JsonFormulaError> {
        match node {
            AstNode::Identifier(name) | AstNode::QuotedIdentifier(name) => {
                self.field(name, value)
            }
            AstNode::ChainedExpression(children) => {
                let mut result = self.visit(&children[0], value)?;
                if let AstNode::Identifier(name) = &children[0] {
                    self.debug_chain_start = Some(name.clone());
                }
                let mut projecting = false;
                for idx in 1..children.len() {
                    if matches!(result, JfValue::Null)
                        && !matches!(
                            children[idx],
                            AstNode::Function { .. }
                                | AstNode::ObjectExpression(_)
                                | AstNode::ArrayExpression(_)
                        )
                    {
                        return Ok(JfValue::Null);
                    }
                    let child = &children[idx];
                    let previous = &children[idx - 1];
                    let is_projection = matches!(
                        previous,
                        AstNode::Projection { .. }
                            | AstNode::ValueProjection { .. }
                            | AstNode::FilterProjection { .. }
                    ) || matches!(
                        previous,
                        AstNode::BracketExpression(left, _)
                            if matches!(**left, AstNode::Projection { .. })
                    );
                    let should_project = matches!(
                        child,
                        AstNode::Identifier(_)
                            | AstNode::QuotedIdentifier(_)
                            | AstNode::Function { .. }
                            | AstNode::ObjectExpression(_)
                            | AstNode::ArrayExpression(_)
                            | AstNode::Projection { .. }
                            | AstNode::ValueProjection { .. }
                            | AstNode::FilterProjection { .. }
                    ) && (is_projection || projecting);
                    if should_project {
                        if let JfValue::Array(items) = result {
                            let mut projected = Vec::new();
                            for item in items {
                                projected.push(self.visit(child, &item)?);
                            }
                            result = JfValue::Array(projected);
                            projecting = true;
                        } else {
                            result = self.visit(child, &result)?;
                            projecting = false;
                        }
                    } else {
                        result = self.visit(child, &result)?;
                        projecting = false;
                    }
                    if matches!(result, JfValue::Null) {
                        let next_allows_null = children
                            .get(idx + 1)
                            .map(|next| {
                                matches!(
                                    next,
                                    AstNode::Function { .. }
                                        | AstNode::ObjectExpression(_)
                                        | AstNode::ArrayExpression(_)
                                )
                            })
                            .unwrap_or(false);
                        if !next_allows_null {
                            return Ok(JfValue::Null);
                        }
                    }
                }
                Ok(result)
            }
            AstNode::BracketExpression(left, right) => {
                let base = self.visit(left, value)?;
                self.visit(right, &base)
            }
            AstNode::Index(expr) => {
                let index = index_value(expr)?;
                if let JfValue::Array(items) = value {
                    let mut idx = index;
                    if idx < 0 {
                        idx = items.len() as i64 + idx;
                    }
                    if idx < 0 || idx as usize >= items.len() {
                        self.debug_mut().push(format!(
                            "Index: {} out of range for array size: {}",
                            idx,
                            items.len()
                        ));
                        return Ok(JfValue::Null);
                    }
                    Ok(items[idx as usize].clone())
                } else {
                    self.debug_mut()
                        .push("Left side of index expression must be an array".to_string());
                    self.debug_mut().push(format!(
                        "Did you intend a single-element array? if so, use a JSON literal: `[{}]`",
                        index
                    ));
                    Ok(JfValue::Null)
                }
            }
            AstNode::Slice { start, stop, step } => {
                if let JfValue::Array(items) = value {
                    let (start, stop, step) =
                        self.compute_slice_params(items.len() as i64, *start, *stop, *step)?;
                    let mut result = Vec::new();
                    if step > 0 {
                        let mut i = start;
                        while i < stop {
                            if let Some(item) = items.get(i as usize) {
                                result.push(item.clone());
                            }
                            i += step;
                        }
                    } else {
                        let mut i = start;
                        while i > stop {
                            if let Some(item) = items.get(i as usize) {
                                result.push(item.clone());
                            }
                            i += step;
                        }
                    }
                    Ok(JfValue::Array(result))
                } else {
                    self.debug_mut().push("Slices apply to arrays only".to_string());
                    Ok(JfValue::Null)
                }
            }
            AstNode::Projection { left, right, debug } => {
                let base = self.visit(left, value)?;
                if let JfValue::Array(items) = base {
                    let mut collected = Vec::new();
                    for item in items {
                        collected.push(self.visit(right, &item)?);
                    }
                    Ok(JfValue::Array(collected))
                } else {
                    if debug.as_deref() == Some("Wildcard") {
                        self.debug_mut()
                            .push("Bracketed wildcards apply to arrays only".to_string());
                    }
                    Ok(JfValue::Null)
                }
            }
            AstNode::ValueProjection { left, right } => {
                let projection = self.visit(left, value)?;
                let proj_value = get_value_of(&projection);
                match proj_value {
                    JfValue::Object(map) => {
                        let mut collected = Vec::new();
                        for val in map.values() {
                            collected.push(self.visit(right, val)?);
                        }
                        Ok(JfValue::Array(collected))
                    }
                    _ => {
                        self.debug_mut()
                            .push("Chained wildcards apply to objects only".to_string());
                        Ok(JfValue::Null)
                    }
                }
            }
            AstNode::FilterProjection {
                left,
                right,
                condition,
            } => {
                let base = self.visit(left, value)?;
                if let JfValue::Array(items) = base {
                    if matches!(&**left, AstNode::ValueProjection { .. }) {
                        let mut projected = Vec::with_capacity(items.len());
                        for item in items {
                            if let JfValue::Array(inner) = item {
                                let mut filtered = Vec::new();
                                for inner_item in inner {
                                    let matched = self.visit(condition, &inner_item)?;
                                    if to_boolean(&matched) {
                                        filtered.push(inner_item);
                                    }
                                }
                                let mut final_results = Vec::new();
                                for inner_item in filtered {
                                    final_results.push(self.visit(right, &inner_item)?);
                                }
                                projected.push(JfValue::Array(final_results));
                            } else {
                                projected.push(JfValue::Null);
                            }
                        }
                        return Ok(JfValue::Array(projected));
                    }
                    let mut filtered = Vec::new();
                    for item in items {
                        let matched = self.visit(condition, &item)?;
                        if to_boolean(&matched) {
                            filtered.push(item);
                        }
                    }
                    let mut final_results = Vec::new();
                    for item in filtered {
                        final_results.push(self.visit(right, &item)?);
                    }
                    Ok(JfValue::Array(final_results))
                } else {
                    self.debug_mut()
                        .push("Filter expressions apply to arrays only".to_string());
                    Ok(JfValue::Null)
                }
            }
            AstNode::Comparator { op, left, right } => {
                let first = get_value_of(&self.visit(left, value)?);
                let second = get_value_of(&self.visit(right, value)?);

                if op == "==" {
                    return Ok(JfValue::Bool(strict_deep_equal(&first, &second)));
                }
                if op == "!=" {
                    return Ok(JfValue::Bool(!strict_deep_equal(&first, &second)));
                }

                if matches!(first, JfValue::Object(_) | JfValue::Array(_)) {
                    self.debug_mut()
                        .push(format!("Cannot use comparators with {}", type_name(&first)));
                    return Ok(JfValue::Bool(false));
                }
                if matches!(second, JfValue::Object(_) | JfValue::Array(_)) {
                    self.debug_mut()
                        .push(format!("Cannot use comparators with {}", type_name(&second)));
                    return Ok(JfValue::Bool(false));
                }
                let type1 = get_type(&first);
                let type2 = get_type(&second);
                let cmp = if matches!(type1, DataType::String) && matches!(type2, DataType::String) {
                    if let (JfValue::String(a), JfValue::String(b)) = (first, second) {
                        compare_str(&a, &b, op)
                    } else {
                        false
                    }
                } else {
                    let n1 = unsafe { (&mut *self.runtime).to_number(&first) };
                    let n2 = unsafe { (&mut *self.runtime).to_number(&second) };
                    if n1.is_err() || n2.is_err() {
                        return Ok(JfValue::Bool(false));
                    }
                    compare_f64(n1?, n2?, op)
                };
                Ok(JfValue::Bool(cmp))
            }
            AstNode::Flatten(inner) => {
                let original = self.visit(inner, value)?;
                if let JfValue::Array(items) = original {
                    let mut merged = Vec::new();
                    for current in items {
                        if let JfValue::Array(nested) = current {
                            merged.extend(nested);
                        } else {
                            merged.push(current);
                        }
                    }
                    Ok(JfValue::Array(merged))
                } else {
                    self.debug_mut().push("Flatten expressions apply to arrays only. If you want an empty array, use a JSON literal: `[]`".to_string());
                    Ok(JfValue::Null)
                }
            }
            AstNode::Identity => Ok(value.clone()),
            AstNode::ArrayExpression(children) => {
                let mut out = Vec::new();
                for child in children {
                    out.push(self.visit(child, value)?);
                }
                Ok(JfValue::Array(out))
            }
            AstNode::ObjectExpression(pairs) => {
                let mut out = indexmap::IndexMap::new();
                for pair in pairs {
                    if out.contains_key(&pair.key) {
                        self.debug_mut()
                            .push(format!("Duplicate key: '{}'", pair.key));
                    }
                    out.insert(pair.key.clone(), self.visit(&pair.value, value)?);
                }
                Ok(JfValue::Object(out))
            }
            AstNode::OrExpression(left, right) => {
                let first = self.visit(left, value)?;
                if !to_boolean(&first) {
                    return self.visit(right, value);
                }
                Ok(first)
            }
            AstNode::AndExpression(left, right) => {
                let first = self.visit(left, value)?;
                if !to_boolean(&first) {
                    return Ok(first);
                }
                self.visit(right, value)
            }
            AstNode::AddExpression(left, right) => {
                let first = self.visit(left, value)?;
                let second = self.visit(right, value)?;
                balance_array_operands(&first, &second);
                self.apply_operator(first, second, "+")
            }
            AstNode::SubtractExpression(left, right) => {
                let first = self.visit(left, value)?;
                let second = self.visit(right, value)?;
                balance_array_operands(&first, &second);
                self.apply_operator(first, second, "-")
            }
            AstNode::MultiplyExpression(left, right) => {
                let first = self.visit(left, value)?;
                let second = self.visit(right, value)?;
                balance_array_operands(&first, &second);
                self.apply_operator(first, second, "*")
            }
            AstNode::DivideExpression(left, right) => {
                let first = self.visit(left, value)?;
                let second = self.visit(right, value)?;
                balance_array_operands(&first, &second);
                self.apply_operator(first, second, "/")
            }
            AstNode::ConcatenateExpression(left, right) => {
                let first = self.visit(left, value)?;
                let second = self.visit(right, value)?;
                balance_array_operands(&first, &second);
                self.apply_operator(first, second, "&")
            }
            AstNode::UnionExpression(left, right) => {
                let mut first = self.visit(left, value)?;
                let mut second = self.visit(right, value)?;
                if matches!(first, JfValue::Null) {
                    first = JfValue::Array(vec![JfValue::Null]);
                }
                if matches!(second, JfValue::Null) {
                    second = JfValue::Array(vec![JfValue::Null]);
                }
                let first = crate::types::match_type(
                    &[DataType::Array],
                    first,
                    "union",
                    |v| unsafe { (&mut *self.runtime).to_number(&v) },
                    |v| unsafe { (&mut *self.runtime).to_string(&v) },
                )?;
                let second = crate::types::match_type(
                    &[DataType::Array],
                    second,
                    "union",
                    |v| unsafe { (&mut *self.runtime).to_number(&v) },
                    |v| unsafe { (&mut *self.runtime).to_string(&v) },
                )?;
                if let (JfValue::Array(mut a), JfValue::Array(b)) = (first, second) {
                    a.extend(b);
                    Ok(JfValue::Array(a))
                } else {
                    Ok(JfValue::Null)
                }
            }
            AstNode::NotExpression(inner) => {
                let first = self.visit(inner, value)?;
                Ok(JfValue::Bool(!to_boolean(&first)))
            }
            AstNode::UnaryMinusExpression(inner) => {
                let first = self.visit(inner, value)?;
                let number = match get_value_of(&first) {
                    JfValue::Number(n) => n,
                    JfValue::String(s) => s.parse::<f64>().unwrap_or(f64::NAN),
                    JfValue::Bool(b) => if b { 1.0 } else { 0.0 },
                    JfValue::Null => 0.0,
                    _ => f64::NAN,
                };
                let minus = number * -1.0;
                if minus.is_nan() {
                    return Err(JsonFormulaError::ty(format!(
                        "Failed to convert \"{}\" to number",
                        value_to_string(&first)
                    )));
                }
                Ok(JfValue::Number(minus))
            }
            AstNode::String(value) => Ok(JfValue::String(value.clone())),
            AstNode::Literal(value) => Ok(JfValue::from_json(value)),
            AstNode::Number(value) => Ok(JfValue::Number(*value)),
            AstNode::Integer(value) => Ok(JfValue::Number(*value as f64)),
            AstNode::Pipe(left, right) => {
                let left_val = self.visit(left, value)?;
                self.visit(right, &left_val)
            }
            AstNode::Current => Ok(value.clone()),
            AstNode::Global(name) => {
                if let Some(JfValue::Object(map)) = &self.globals {
                    Ok(map.get(name).cloned().unwrap_or(JfValue::Null))
                } else {
                    Ok(JfValue::Null)
                }
            }
            AstNode::Function { name, args } => {
                if name == "if" {
                    if args.len() != 3 {
                        return Err(JsonFormulaError::function(
                            "if() takes 3 arguments".to_string(),
                        ));
                    }
                    let condition = self.visit(&args[0], value)?;
                    if matches!(condition, JfValue::Expref(_)) {
                        return Err(JsonFormulaError::ty(
                            "if() does not accept an expression reference argument.".to_string(),
                        ));
                    }
                    if to_boolean(&condition) {
                        return self.visit(&args[1], value);
                    }
                    return self.visit(&args[2], value);
                }
                let mut resolved_args = Vec::new();
                for child in args {
                    resolved_args.push(self.visit(child, value)?);
                }
                unsafe { (&mut *self.runtime).call_function(name, resolved_args, value, self, true) }
            }
            AstNode::ExpressionReference(expr) => Ok(JfValue::Expref(Box::new((**expr).clone()))),
            AstNode::KeyValuePair { .. } => Err(JsonFormulaError::syntax(
                "Unexpected key-value pair".to_string(),
            )),
        }
    }

    fn field(&mut self, name: &str, value: &JfValue) -> Result<JfValue, JsonFormulaError> {
        match value {
            JfValue::Null => {
                let chain = self.debug_chain_start.as_deref().map(|s| s.to_string());
                debug_available(
                    self.debug_mut(),
                    value,
                    name,
                    chain.as_deref(),
                );
                Ok(JfValue::Null)
            }
            JfValue::Object(_) | JfValue::Array(_) | JfValue::Field { .. } => {
                if let Some(field) = get_property(value, name) {
                    Ok(field)
                } else {
                    let chain = self.debug_chain_start.as_deref().map(|s| s.to_string());
                    debug_available(self.debug_mut(), value, name, chain.as_deref());
                    Ok(JfValue::Null)
                }
            }
            _ => {
                let chain = self.debug_chain_start.as_deref().map(|s| s.to_string());
                debug_available(self.debug_mut(), value, name, chain.as_deref());
                Ok(JfValue::Null)
            }
        }
    }

    fn compute_slice_params(
        &self,
        array_length: i64,
        start: Option<i64>,
        stop: Option<i64>,
        step: Option<i64>,
    ) -> Result<(i64, i64, i64), JsonFormulaError> {
        let step_val = step.unwrap_or(1);
        if step_val == 0 {
            return Err(JsonFormulaError::evaluation(
                "Invalid slice, step cannot be 0".to_string(),
            ));
        }
        let step_negative = step_val < 0;
        let (start_val, stop_val) = if step_negative {
            let mut start_val = start.unwrap_or(array_length - 1);
            if start_val < 0 {
                start_val += array_length;
                if start_val < 0 {
                    start_val = -1;
                }
            } else if start_val >= array_length {
                start_val = array_length - 1;
            }
            let mut stop_val = if let Some(value) = stop {
                value
            } else {
                -1
            };
            if let Some(mut value) = stop {
                if value < 0 {
                    value += array_length;
                    if value < 0 {
                        value = -1;
                    }
                } else if value >= array_length {
                    value = array_length - 1;
                }
                stop_val = value;
            }
            (start_val, stop_val)
        } else {
            let mut start_val = start.unwrap_or(0);
            if start_val < 0 {
                start_val += array_length;
                if start_val < 0 {
                    start_val = 0;
                }
            } else if start_val > array_length {
                start_val = array_length;
            }
            let mut stop_val = stop.unwrap_or(array_length);
            if stop_val < 0 {
                stop_val += array_length;
                if stop_val < 0 {
                    stop_val = 0;
                }
            } else if stop_val > array_length {
                stop_val = array_length;
            }
            (start_val, stop_val)
        };
        Ok((start_val, stop_val, step_val))
    }

    fn apply_operator(
        &mut self,
        first: JfValue,
        second: JfValue,
        operator: &str,
    ) -> Result<JfValue, JsonFormulaError> {
        if let (JfValue::Array(a), JfValue::Array(b)) = (&first, &second) {
            let max_len = a.len().max(b.len());
            let mut out = Vec::with_capacity(max_len);
            for i in 0..max_len {
                let left = a.get(i).cloned().unwrap_or(JfValue::Null);
                let right = b.get(i).cloned().unwrap_or(JfValue::Null);
                out.push(self.apply_operator(left, right, operator)?);
            }
            return Ok(JfValue::Array(out));
        }

        if let JfValue::Array(items) = first {
            let mut out = Vec::new();
            for item in items {
                out.push(self.apply_operator(item, second.clone(), operator)?);
            }
            return Ok(JfValue::Array(out));
        }
        if let JfValue::Array(items) = second {
            let mut out = Vec::new();
            for item in items {
                out.push(self.apply_operator(first.clone(), item, operator)?);
            }
            return Ok(JfValue::Array(out));
        }

        if operator == "&" {
            let left = unsafe { (&mut *self.runtime).to_string(&first)? };
            let right = unsafe { (&mut *self.runtime).to_string(&second)? };
            return Ok(JfValue::String(format!("{}{}", left, right)));
        }
        if operator == "*" {
            let n1 = unsafe { (&mut *self.runtime).to_number(&first)? };
            let n2 = unsafe { (&mut *self.runtime).to_number(&second)? };
            return Ok(JfValue::Number(n1 * n2));
        }

        let n1 = unsafe { (&mut *self.runtime).to_number(&first)? };
        let n2 = unsafe { (&mut *self.runtime).to_number(&second)? };
        let result = match operator {
            "+" => n1 + n2,
            "-" => n1 - n2,
            "/" => {
                let res = n1 / n2;
                if !res.is_finite() {
                    return Err(JsonFormulaError::evaluation(format!(
                        "Division by zero {}/{}",
                        n1, n2
                    )));
                }
                res
            }
            _ => n1,
        };
        Ok(JfValue::Number(result))
    }

    pub(crate) fn debug_mut(&mut self) -> &mut Vec<String> {
        unsafe { &mut *self.debug }
    }
}

fn balance_array_operands(_left: &JfValue, _right: &JfValue) {}

fn index_value(node: &AstNode) -> Result<i64, JsonFormulaError> {
    match node {
        AstNode::Integer(v) => Ok(*v),
        AstNode::Number(v) => Ok(*v as i64),
        AstNode::UnaryMinusExpression(inner) => match &**inner {
            AstNode::Integer(v) => Ok(-*v),
            AstNode::Number(v) => Ok(-(*v as i64)),
            _ => Err(JsonFormulaError::syntax(
                "Slice expressions must be integers".to_string(),
            )),
        },
        _ => Err(JsonFormulaError::syntax(
            "Slice expressions must be integers".to_string(),
        )),
    }
}

fn compare_f64(a: f64, b: f64, op: &str) -> bool {
    match op {
        ">" => a > b,
        ">=" => a >= b,
        "<" => a < b,
        "<=" => a <= b,
        _ => false,
    }
}

fn compare_str(a: &str, b: &str, op: &str) -> bool {
    match op {
        ">" => a > b,
        ">=" => a >= b,
        "<" => a < b,
        "<=" => a <= b,
        _ => false,
    }
}

fn type_name(value: &JfValue) -> &'static str {
    crate::types::type_name(get_type(value))
}

fn value_to_string(value: &JfValue) -> String {
    match value {
        JfValue::String(s) => s.clone(),
        JfValue::Number(n) => n.to_string(),
        JfValue::Bool(b) => b.to_string(),
        JfValue::Null => "null".to_string(),
        _ => "<value>".to_string(),
    }
}
