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

use indexmap::IndexMap;
use serde_json::Value as JsonValue;

use crate::ast::AstNode;

#[derive(Debug, Clone, PartialEq)]
pub enum JfValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JfValue>),
    Object(IndexMap<String, JfValue>),
    Field {
        name: String,
        value: Box<JfValue>,
        readonly: bool,
        required: bool,
    },
    Expref(Box<AstNode>),
}

impl JfValue {
    pub fn from_json(value: &JsonValue) -> Self {
        match value {
            JsonValue::Null => JfValue::Null,
            JsonValue::Bool(b) => JfValue::Bool(*b),
            JsonValue::Number(n) => JfValue::Number(n.as_f64().unwrap_or(0.0)),
            JsonValue::String(s) => JfValue::String(s.clone()),
            JsonValue::Array(items) => {
                JfValue::Array(items.iter().map(JfValue::from_json).collect())
            }
            JsonValue::Object(map) => {
                let mut out = IndexMap::new();
                for (k, v) in map {
                    out.insert(k.clone(), JfValue::from_json(v));
                }
                JfValue::Object(out)
            }
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            JfValue::Null => JsonValue::Null,
            JfValue::Bool(b) => JsonValue::Bool(*b),
            JfValue::Number(n) => {
                if n.fract() == 0.0
                    && *n >= i64::MIN as f64
                    && *n <= i64::MAX as f64
                {
                    JsonValue::Number(serde_json::Number::from(*n as i64))
                } else {
                    JsonValue::Number(
                        serde_json::Number::from_f64(*n)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    )
                }
            }
            JfValue::String(s) => JsonValue::String(s.clone()),
            JfValue::Array(items) => {
                JsonValue::Array(items.iter().map(|v| v.to_json()).collect())
            }
            JfValue::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, v) in map {
                    out.insert(k.clone(), v.to_json());
                }
                JsonValue::Object(out)
            }
            JfValue::Field { value, .. } => value.to_json(),
            JfValue::Expref(_) => JsonValue::Null,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Number,
    Any,
    String,
    Array,
    Object,
    Boolean,
    Expref,
    Null,
    ArrayNumber,
    ArrayString,
    ArrayArray,
    EmptyArray,
}

pub fn type_name(dt: DataType) -> &'static str {
    match dt {
        DataType::Number => "number",
        DataType::Any => "any",
        DataType::String => "string",
        DataType::Array => "array",
        DataType::Object => "object",
        DataType::Boolean => "boolean",
        DataType::Expref => "expression",
        DataType::Null => "null",
        DataType::ArrayNumber => "Array<number>",
        DataType::ArrayString => "Array<string>",
        DataType::ArrayArray => "Array<array>",
        DataType::EmptyArray => "array",
    }
}

pub fn get_type(value: &JfValue) -> DataType {
    match value {
        JfValue::Null => DataType::Null,
        JfValue::Bool(_) => DataType::Boolean,
        JfValue::Number(_) => DataType::Number,
        JfValue::String(_) => DataType::String,
        JfValue::Array(items) => {
            if items.is_empty() {
                return DataType::EmptyArray;
            }
            if items.iter().all(|v| matches!(get_type(v), DataType::Number)) {
                return DataType::ArrayNumber;
            }
            if items.iter().all(|v| matches!(get_type(v), DataType::String)) {
                return DataType::ArrayString;
            }
            if items.iter().all(|v| is_array_type(get_type(v))) {
                return DataType::ArrayArray;
            }
            DataType::Array
        }
        JfValue::Object(_) => DataType::Object,
        JfValue::Field { value, .. } => get_type(value),
        JfValue::Expref(_) => DataType::Expref,
    }
}

pub fn is_array_type(dt: DataType) -> bool {
    matches!(
        dt,
        DataType::Array
            | DataType::ArrayNumber
            | DataType::ArrayString
            | DataType::ArrayArray
            | DataType::EmptyArray
    )
}

fn supported_conversion(from: DataType, to: DataType) -> bool {
    match from {
        DataType::Number => matches!(
            to,
            DataType::String | DataType::Array | DataType::ArrayNumber | DataType::Boolean
        ),
        DataType::Boolean => matches!(to, DataType::String | DataType::Number | DataType::Array),
        DataType::Array => matches!(
            to,
            DataType::Boolean | DataType::ArrayString | DataType::ArrayNumber
        ),
        DataType::ArrayNumber => matches!(
            to,
            DataType::Boolean | DataType::ArrayString | DataType::Array
        ),
        DataType::ArrayString => matches!(
            to,
            DataType::Boolean | DataType::ArrayNumber | DataType::Array
        ),
        DataType::ArrayArray => matches!(to, DataType::Boolean),
        DataType::EmptyArray => matches!(to, DataType::Boolean),
        DataType::Object => matches!(to, DataType::Boolean),
        DataType::Null => matches!(to, DataType::String | DataType::Number | DataType::Boolean),
        DataType::String => matches!(
            to,
            DataType::Number | DataType::ArrayString | DataType::Array | DataType::Boolean
        ),
        DataType::Any => true,
        DataType::Expref => false,
    }
}

pub fn match_type<FN, FS>(
    expected_list: &[DataType],
    arg_value: JfValue,
    context: &str,
    to_number: FN,
    to_string: FS,
) -> Result<JfValue, crate::errors::JsonFormulaError>
where
    FN: Fn(JfValue) -> Result<f64, crate::errors::JsonFormulaError>,
    FS: Fn(JfValue) -> Result<String, crate::errors::JsonFormulaError>,
{
    let actual = get_type(&arg_value);
    if matches!(arg_value, JfValue::Expref(_)) && !expected_list.contains(&DataType::Expref) {
        return Err(crate::errors::JsonFormulaError::ty(format!(
            "{} does not accept an expression reference argument.",
            context
        )));
    }

    let is_object = |t| t == DataType::Object;
    let matches_type = |expect, found| {
        expect == found
            || expect == DataType::Any
            || (expect == DataType::Array && is_array_type(found))
            || (is_array_type(expect) && found == DataType::EmptyArray)
    };

    if expected_list.iter().any(|t| matches_type(*t, actual)) {
        return Ok(arg_value);
    }

    let filtered: Vec<DataType> = expected_list
        .iter()
        .copied()
        .filter(|t| supported_conversion(actual, *t))
        .collect();
    if filtered.is_empty() {
        return Err(crate::errors::JsonFormulaError::ty(format!(
            "{} expected argument to be type {} but received type {} instead.",
            context,
            type_name(expected_list[0]),
            type_name(actual)
        )));
    }
    let exact_match = filtered.len() > 1;
    let expected = filtered[0];
    let mut wrong_type = false;

    if is_array_type(actual) {
        if matches!(expected, DataType::ArrayNumber | DataType::ArrayString) {
            if let JfValue::Array(items) = &arg_value {
                if items.iter().any(|a| {
                    let t = get_type(a);
                    is_array_type(t) || is_object(t)
                }) {
                    wrong_type = true;
                }
            }
        }
    }
    if exact_match && expected == DataType::Object {
        wrong_type = true;
    }

    if exact_match {
        return Err(crate::errors::JsonFormulaError::ty(format!(
            "{} cannot process type: {}. Must be one of: {}.",
            context,
            type_name(actual),
            expected_list
                .iter()
                .map(|t| type_name(*t))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if wrong_type {
        return Err(crate::errors::JsonFormulaError::ty(format!(
            "{} expected argument to be type {} but received type {} instead.",
            context,
            type_name(expected),
            type_name(actual)
        )));
    }

    if is_object(actual) && expected == DataType::Boolean {
        if let JfValue::Object(map) = arg_value {
            return Ok(JfValue::Bool(map.is_empty()));
        }
    }

    if is_array_type(actual) {
        if let JfValue::Array(ref items) = arg_value {
            if expected == DataType::Boolean {
                return Ok(JfValue::Bool(!items.is_empty()));
            }
            if expected == DataType::ArrayString {
                let mut out = Vec::new();
                for item in items {
                    out.push(JfValue::String(to_string(item.clone())?));
                }
                return Ok(JfValue::Array(out));
            }
            if expected == DataType::ArrayNumber {
                let mut out = Vec::new();
                for item in items {
                    out.push(JfValue::Number(to_number(item.clone())?));
                }
                return Ok(JfValue::Array(out));
            }
            if expected == DataType::ArrayArray {
                let out = items
                    .iter()
                    .map(|a| if let JfValue::Array(_) = a { a.clone() } else { JfValue::Array(vec![a.clone()]) })
                    .collect();
                return Ok(JfValue::Array(out));
            }
        }
    }

    if !is_array_type(actual) && !is_object(actual) {
        return match expected {
            DataType::ArrayString => Ok(JfValue::Array(vec![JfValue::String(to_string(
                arg_value,
            )?)])),
            DataType::ArrayNumber => Ok(JfValue::Array(vec![JfValue::Number(to_number(
                arg_value,
            )?)])),
            DataType::Array => Ok(JfValue::Array(vec![arg_value])),
            DataType::Number => Ok(JfValue::Number(to_number(arg_value)?)),
            DataType::String => Ok(JfValue::String(to_string(arg_value)?)),
            DataType::Boolean => Ok(JfValue::Bool(match arg_value {
                JfValue::Bool(b) => b,
                JfValue::Null => false,
                JfValue::Number(n) => n != 0.0,
                JfValue::String(s) => !s.is_empty(),
                _ => true,
            })),
            _ => Err(crate::errors::JsonFormulaError::ty(format!(
                "{} expected argument to be type {} but received type {} instead.",
                context,
                type_name(expected),
                type_name(actual)
            ))),
        };
    }

    Err(crate::errors::JsonFormulaError::ty(format!(
        "{} expected argument to be type {} but received type {} instead.",
        context,
        type_name(expected),
        type_name(actual)
    )))
}
