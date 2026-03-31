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

use crate::types::JfValue;

pub fn wrap_fields(value: &JfValue) -> JfValue {
    match value {
        JfValue::Array(items) => JfValue::Array(items.iter().map(wrap_fields).collect()),
        JfValue::Object(map) => {
            let mut out = indexmap::IndexMap::new();
            for (key, val) in map {
                out.insert(
                    key.clone(),
                    JfValue::Field {
                        name: key.clone(),
                        value: Box::new(wrap_fields(val)),
                        readonly: false,
                        required: true,
                    },
                );
            }
            JfValue::Object(out)
        }
        _ => value.clone(),
    }
}

pub fn get_value_of(value: &JfValue) -> JfValue {
    match value {
        JfValue::Array(items) => {
            JfValue::Array(items.iter().map(get_value_of).collect())
        }
        JfValue::Field { value, .. } => *value.clone(),
        _ => value.clone(),
    }
}

pub fn to_boolean(value: &JfValue) -> bool {
    if matches!(value, JfValue::Null) {
        return false;
    }
    let val = get_value_of(value);
    match val {
        JfValue::Array(items) => !items.is_empty(),
        JfValue::Object(map) => !map.is_empty(),
        JfValue::Field { .. } => true,
        JfValue::Bool(b) => b,
        JfValue::Number(n) => n != 0.0,
        JfValue::String(s) => !s.is_empty(),
        JfValue::Null => false,
        JfValue::Expref(_) => true,
    }
}

pub fn strict_deep_equal(lhs: &JfValue, rhs: &JfValue) -> bool {
    let first = get_value_of(lhs);
    let second = get_value_of(rhs);
    if first == second {
        return true;
    }
    match (&first, &second) {
        (JfValue::Array(a), JfValue::Array(b)) => {
            if a.len() != b.len() {
                return false;
            }
            a.iter()
                .zip(b.iter())
                .all(|(l, r)| strict_deep_equal(l, r))
        }
        (JfValue::Object(a), JfValue::Object(b)) => {
            if a.len() != b.len() {
                return false;
            }
            a.iter().all(|(k, v)| {
                b.get(k)
                    .map_or(false, |rv| strict_deep_equal(v, rv))
            })
        }
        _ => false,
    }
}

pub fn get_property(value: &JfValue, key: &str) -> Option<JfValue> {
    match value {
        JfValue::Object(map) => map.get(key).cloned(),
        JfValue::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i).cloned()),
        JfValue::Field {
            name,
            value,
            readonly,
            required,
        } => match key {
            "$name" => Some(JfValue::String(name.clone())),
            "$value" => Some(*value.clone()),
            "$readonly" => Some(JfValue::Bool(*readonly)),
            "$required" => Some(JfValue::Bool(*required)),
            _ => get_property(value, key),
        },
        _ => None,
    }
}

pub fn debug_available(debug: &mut Vec<String>, value: &JfValue, key: &str, chain_start: Option<&str>) {
    if let JfValue::Array(items) = value {
        if !items.is_empty() {
            debug.push(format!("Failed to find: '{}' on an array object.", key));
            debug.push(format!(
                "Did you mean to use a projection? e.g. {}[*].{}",
                chain_start.unwrap_or("array"),
                key
            ));
            return;
        }
    }
    debug.push(format!("Failed to find: '{}'", key));
    if let JfValue::Object(map) = value {
        let available: Vec<String> = map
            .keys()
            .filter(|k| !k.chars().all(|c| c.is_ascii_digit()))
            .filter(|k| !k.starts_with('$') || key.starts_with('$'))
            .map(|k| format!("'{}'", k))
            .collect();
        if !available.is_empty() {
            debug.push(format!("Available fields: {}", available.join(", ")));
        }
    }
}
