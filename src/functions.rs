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

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use serde::Serialize;

use crate::errors::JsonFormulaError;
use crate::interpreter::Interpreter;
use crate::runtime::{FunctionEntry, FunctionImpl, Runtime, SignatureArg};
use crate::types::{get_type, is_array_type, DataType, JfValue};
use crate::utils::{get_property, get_value_of, strict_deep_equal, to_boolean};

const MS_IN_DAY: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

pub fn builtin_functions() -> HashMap<String, FunctionEntry> {
    let mut map = HashMap::new();

    macro_rules! is_opt {
        () => {
            false
        };
        (optional) => {
            true
        };
    }
    macro_rules! is_var {
        () => {
            false
        };
        (variadic) => {
            true
        };
    }
    macro_rules! sig {
        ( $( [ $( $t:expr ),+ ] $(, $opt:ident)? $(, $var:ident)? );+ $(;)? ) => {
            vec![
                $(
                    SignatureArg {
                        types: vec![$($t),+],
                        optional: is_opt!($($opt)?),
                        variadic: is_var!($($var)?),
                    }
                ),+
            ]
        };
    }

    let simple = |func: FunctionImpl, signature: Vec<SignatureArg>| FunctionEntry {
        func,
        signature,
        expref: None,
    };

    map.insert(
        "abs".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive(runtime, &args[0], |n| n.abs())
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "acos".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| valid_number(n.acos(), "acos"))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "and".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let mut result = to_boolean(&get_value_of(&args[0]));
                for arg in args.iter().skip(1) {
                    result = result && to_boolean(&get_value_of(arg));
                }
                Ok(JfValue::Bool(result))
            }),
            vec![SignatureArg {
                types: vec![DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "asin".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| valid_number(n.asin(), "asin"))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "atan2".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_binary(runtime, &args[0], &args[1], |y, x| Ok(y.atan2(x)))
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "avg".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let values = flatten(&args[0])
                    .into_iter()
                    .filter(|v| matches!(get_type(v), DataType::Number))
                    .map(|v| match v {
                        JfValue::Number(n) => n,
                        _ => 0.0,
                    })
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    return Err(JsonFormulaError::evaluation(
                        "avg() requires at least one argument".to_string(),
                    ));
                }
                let sum: f64 = values.iter().sum();
                Ok(JfValue::Number(sum / values.len() as f64))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "avgA".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut values = Vec::new();
                for v in flatten(&args[0]) {
                    if matches!(get_type(&v), DataType::Null) {
                        continue;
                    }
                    let n = runtime
                        .to_number(&v)
                        .map_err(|_| JsonFormulaError::ty("avgA() received non-numeric parameters"))?;
                    values.push(n);
                }
                if values.is_empty() {
                    return Err(JsonFormulaError::evaluation(
                        "avg() requires at least one argument".to_string(),
                    ));
                }
                let sum: f64 = values.iter().sum();
                Ok(JfValue::Number(sum / values.len() as f64))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "casefold".to_string(),
        simple(
            Box::new(|runtime, args, _data, interp| {
                evaluate(args, |vals| {
                    let s = runtime.to_string(&vals[0])?;
                    Ok(JfValue::String(casefold_locale(&s, &interp.language)))
                })
            }),
            sig!([DataType::String, DataType::ArrayString]),
        ),
    );

    map.insert(
        "ceil".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok(n.ceil()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "codePoint".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_result(runtime, &args[0], |text| {
                    if text.is_empty() {
                        Ok(JfValue::Null)
                    } else {
                        let code = text.chars().next().unwrap() as u32;
                        Ok(JfValue::Number(code as f64))
                    }
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "contains".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let subject = get_value_of(&args[0]);
                let search = get_value_of(&args[1]);
                if is_array_type(get_type(&args[0])) {
                    if let JfValue::Array(items) = subject {
                        let found = items.iter().any(|s| strict_deep_equal(s, &search));
                        return Ok(JfValue::Bool(found));
                    }
                }
                let source = runtime.to_string(&subject)?;
                if get_type(&search) != DataType::String {
                    return Err(JsonFormulaError::ty(
                        "contains() requires a string search value for string subjects".to_string(),
                    ));
                }
                let needle = runtime.to_string(&search)?;
                if needle.is_empty() {
                    return Ok(JfValue::Bool(true));
                }
                let source_chars: Vec<char> = source.chars().collect();
                let needle_chars: Vec<char> = needle.chars().collect();
                if source_chars.len() < needle_chars.len() {
                    return Ok(JfValue::Bool(false));
                }
                for i in 0..=source_chars.len().saturating_sub(needle_chars.len()) {
                    if source_chars[i..i + needle_chars.len()] == needle_chars[..] {
                        return Ok(JfValue::Bool(true));
                    }
                }
                Ok(JfValue::Bool(false))
            }),
            sig!([DataType::String, DataType::Array]; [DataType::Any]),
        ),
    );

    map.insert(
        "cos".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok(n.cos()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "datedif".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_datedif_recursive(runtime, &args[0], &args[1], &args[2])
            }),
            sig!([DataType::Any]; [DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "datetime".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let year = to_integer(runtime, &args[0])?;
                let month = to_integer(runtime, &args[1])? - 1;
                let day = to_integer(runtime, &args[2])?;
                let hours = if args.len() > 3 {
                    to_integer(runtime, &args[3])?
                } else {
                    0
                };
                let minutes = if args.len() > 4 {
                    to_integer(runtime, &args[4])?
                } else {
                    0
                };
                let seconds = if args.len() > 5 {
                    to_integer(runtime, &args[5])?
                } else {
                    0
                };
                let ms = if args.len() > 6 {
                    to_integer(runtime, &args[6])?
                } else {
                    0
                };
                Ok(JfValue::Number(datetime_to_num(
                    year, month, day, hours, minutes, seconds, ms,
                )))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "day".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).day() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "debug".to_string(),
        simple(
            Box::new(|_runtime, args, _data, interp| {
                let arg = args[0].clone();
                if args.len() > 1 {
                    match &args[1] {
                        JfValue::Expref(node) => {
                            let value = interp.visit(node, &arg)?;
                            interp.debug_mut().push(format!("{:?}", value.to_json()));
                        }
                        _ => {
                            interp
                                .debug_mut()
                                .push(to_json_string(&get_value_of(&args[1]), 0));
                        }
                    }
                } else {
                    interp
                        .debug_mut()
                        .push(to_json_string(&get_value_of(&arg), 0));
                }
                Ok(arg)
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any, DataType::Expref],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "deepScan".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let source = args[0].clone();
                let name_arg = args[1].clone();
                let (name, check_arrays) = if matches!(get_type(&name_arg), DataType::Number) {
                    (to_integer(runtime, &name_arg)?.to_string(), true)
                } else {
                    (runtime.to_string(&name_arg)?, false)
                };
                let mut items = Vec::new();
                fn scan(node: &JfValue, name: &str, check_arrays: bool, items: &mut Vec<JfValue>) {
                    match node {
                        JfValue::Null => {}
                        JfValue::Array(arr) => {
                            if check_arrays {
                                if let Ok(index) = name.parse::<usize>() {
                                    if let Some(val) = arr.get(index) {
                                        items.push(val.clone());
                                    }
                                }
                            }
                            for child in arr {
                                scan(child, name, check_arrays, items);
                            }
                        }
                        JfValue::Object(map) => {
                            for (k, v) in map {
                                if !check_arrays && k == name {
                                    items.push(v.clone());
                                }
                                scan(v, name, check_arrays, items);
                            }
                        }
                        _ => {}
                    }
                }
                scan(&source, &name, check_arrays, &mut items);
                Ok(JfValue::Array(items))
            }),
            sig!(
                [DataType::Object, DataType::Array, DataType::Null];
                [DataType::String, DataType::Number]
            ),
        ),
    );

    map.insert(
        "endsWith".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_binary(runtime, &args[0], &args[1], |s, suffix| {
                    Ok(JfValue::Bool(ends_with(s, suffix)))
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "entries".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                match get_value_of(&args[0]) {
                    JfValue::Object(map) => {
                        let mut out = Vec::new();
                        for (k, v) in map {
                            out.push(JfValue::Array(vec![JfValue::String(k), v]));
                        }
                        Ok(JfValue::Array(out))
                    }
                    JfValue::Array(items) => {
                        let mut out = Vec::new();
                        for (idx, v) in items.into_iter().enumerate() {
                            out.push(JfValue::Array(vec![
                                JfValue::String(idx.to_string()),
                                v,
                            ]));
                        }
                        Ok(JfValue::Array(out))
                    }
                    JfValue::Null => Err(JsonFormulaError::ty(
                        "entries() requires an object or array".to_string(),
                    )),
                    other => Ok(JfValue::Array(vec![JfValue::Array(vec![
                        JfValue::String("0".to_string()),
                        other,
                    ])])),
                }
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "eomonth".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_binary(runtime, &args[0], &args[1], |date, months| {
                    Ok(eomonth(date, months.trunc() as i64))
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "exp".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok(n.exp()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "false".to_string(),
        simple(Box::new(|_runtime, _args, _data, _interp| Ok(JfValue::Bool(false))), vec![]),
    );

    map.insert(
        "find".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let offset = if args.len() > 2 {
                    args[2].clone()
                } else {
                    JfValue::Number(0.0)
                };
                map_find_recursive(runtime, &args[0], &args[1], &offset)
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "floor".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok(n.floor()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "fromCodePoint".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let points = if let JfValue::Array(items) = &args[0] {
                    items
                        .iter()
                        .map(|v| to_integer(runtime, v))
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    vec![to_integer(runtime, &args[0])?]
                };
                let mut out = String::new();
                for p in points {
                    if let Some(ch) = std::char::from_u32(p as u32) {
                        out.push(ch);
                    } else {
                        return Err(JsonFormulaError::evaluation(format!(
                            "Invalid code point: \"{}\"",
                            p
                        )));
                    }
                }
                Ok(JfValue::String(out))
            }),
            sig!([DataType::Number, DataType::ArrayNumber]),
        ),
    );

    map.insert(
        "fromEntries".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let array = match &args[0] {
                    JfValue::Array(items) => items.clone(),
                    _ => {
                        return Err(JsonFormulaError::ty(
                            "fromEntries() requires an array of key value pairs".to_string(),
                        ))
                    }
                };
                let mut out = indexmap::IndexMap::new();
                for item in array {
                    match item {
                        JfValue::Array(pair) if pair.len() == 2 => {
                            if let JfValue::String(key) = &pair[0] {
                                out.insert(key.clone(), pair[1].clone());
                            } else {
                                return Err(JsonFormulaError::ty(
                                    "fromEntries() requires an array of key value pairs".to_string(),
                                ));
                            }
                        }
                        _ => {
                            return Err(JsonFormulaError::ty(
                                "fromEntries() requires an array of key value pairs".to_string(),
                            ))
                        }
                    }
                }
                Ok(JfValue::Object(out))
            }),
            sig!([DataType::ArrayArray]),
        ),
    );

    map.insert(
        "fround".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok((n as f32) as f64))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "hasProperty".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut key = args[1].clone();
                let key_type = get_type(&key);
                if let JfValue::Field { .. } = &args[0] {
                    if let JfValue::String(name) = &args[1] {
                        if name.starts_with('$') {
                            return Ok(JfValue::Bool(get_property(&args[0], name).is_some()));
                        }
                    }
                }
                let obj = get_value_of(&args[0]);
                if matches!(obj, JfValue::Null) {
                    return Ok(JfValue::Bool(false));
                }
                let is_array = is_array_type(get_type(&obj));
                if !is_array && get_type(&obj) != DataType::Object {
                    return Err(JsonFormulaError::ty(
                        "First parameter to hasProperty() must be either an object or array."
                            .to_string(),
                    ));
                }
                if is_array {
                    if key_type != DataType::Number {
                        return Err(JsonFormulaError::ty(
                            "hasProperty(): Array index must be an integer".to_string(),
                        ));
                    }
                    key = JfValue::Number(to_integer(runtime, &key)? as f64);
                } else if key_type != DataType::String {
                    return Err(JsonFormulaError::ty(
                        "hasProperty(): Object key must be a string".to_string(),
                    ));
                }
                let key_str = match key {
                    JfValue::String(s) => s,
                    JfValue::Number(n) => n.to_string(),
                    _ => return Ok(JfValue::Bool(false)),
                };
                Ok(JfValue::Bool(get_property(&obj, &key_str).is_some()))
            }),
            sig!([DataType::Any]; [DataType::String, DataType::Number]),
        ),
    );

    map.insert(
        "hour".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).hour() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "if".to_string(),
        simple(
            Box::new(|_runtime, _args, _data, _interp| {
                Err(JsonFormulaError::function(
                    "if() must be handled by interpreter".to_string(),
                ))
            }),
            sig!([DataType::Any]; [DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "join".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let glue = to_json_for_join(&get_value_of(&args[1]), 0);
                match &args[0] {
                    JfValue::Array(items) => {
                        let joined = items
                            .iter()
                            .map(|v| to_json_for_join(&get_value_of(v), 0))
                            .collect::<Vec<_>>()
                            .join(&glue);
                        Ok(JfValue::String(joined))
                    }
                    _ => Ok(JfValue::String(to_json_for_join(&get_value_of(&args[0]), 0))),
                }
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "keys".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                match get_value_of(&args[0]) {
                    JfValue::Object(map) => Ok(JfValue::Array(
                        map.keys().map(|k| JfValue::String(k.clone())).collect(),
                    )),
                    _ => Ok(JfValue::Array(Vec::new())),
                }
            }),
            sig!([DataType::Object]),
        ),
    );

    map.insert(
        "left".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let num = if args.len() > 1 {
                    to_integer(runtime, &args[1])?
                } else {
                    1
                };
                if num < 0 {
                    return Err(JsonFormulaError::evaluation(
                        "left() requires a non-negative number of elements".to_string(),
                    ));
                }
                if is_array_type(get_type(&args[0])) {
                    if let JfValue::Array(items) = &args[0] {
                        return Ok(JfValue::Array(items.iter().take(num as usize).cloned().collect()));
                    }
                }
                let text = runtime.to_string(&args[0])?;
                let chars: Vec<char> = text.chars().collect();
                Ok(JfValue::String(chars.into_iter().take(num as usize).collect()))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::String, DataType::Array],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "length".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let arg = get_value_of(&args[0]);
                match arg {
                    JfValue::Object(map) => Ok(JfValue::Number(map.len() as f64)),
                    JfValue::Array(items) => Ok(JfValue::Number(items.len() as f64)),
                    _ => {
                        let text = runtime.to_string(&arg)?;
                        Ok(JfValue::Number(text.chars().count() as f64))
                    }
                }
            }),
            sig!([DataType::String, DataType::Array, DataType::Object]),
        ),
    );

    map.insert(
        "log".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| valid_number(n.ln(), "log"))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "log10".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| valid_number(n.log10(), "log10"))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "lower".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_result(runtime, &args[0], |text| {
                    Ok(JfValue::String(text.to_lowercase()))
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "map".to_string(),
        simple(
            Box::new(|_runtime, args, _data, interp| {
                let array = match get_value_of(&args[0]) {
                    JfValue::Array(items) => items,
                    _ => Vec::new(),
                };
                let expref = match &args[1] {
                    JfValue::Expref(node) => node.clone(),
                    _ => return Err(JsonFormulaError::ty("map() requires an expression")),
                };
                let mut out = Vec::new();
                for item in array {
                    out.push(interp.visit(&expref, &item)?);
                }
                Ok(JfValue::Array(out))
            }),
            sig!([DataType::Array]; [DataType::Expref]),
        ),
    );

    map.insert(
        "max".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let values: Vec<f64> = args
                    .iter()
                    .flat_map(|v| flatten(v))
                    .filter_map(|v| match get_value_of(&v) {
                        JfValue::Number(n) => Some(n),
                        _ => None,
                    })
                    .collect();
                if values.is_empty() {
                    return Ok(JfValue::Number(0.0));
                }
                Ok(JfValue::Number(values.into_iter().fold(f64::MIN, f64::max)))
            }),
            vec![SignatureArg {
                types: vec![DataType::Array, DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "maxA".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut values = Vec::new();
                for v in args.iter().flat_map(|v| flatten(v)) {
                    if matches!(get_value_of(&v), JfValue::Null) {
                        continue;
                    }
                    let n = runtime.to_number(&v)?;
                    values.push(n);
                }
                if values.is_empty() {
                    return Ok(JfValue::Number(0.0));
                }
                Ok(JfValue::Number(values.into_iter().fold(f64::MIN, f64::max)))
            }),
            vec![SignatureArg {
                types: vec![DataType::Array, DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "merge".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let mut merged = indexmap::IndexMap::new();
                for current in args {
                    if matches!(current, JfValue::Null) {
                        continue;
                    }
                    if let JfValue::Object(map) = get_value_of(&current) {
                        for (k, v) in map {
                            merged.insert(k, v);
                        }
                    }
                }
                Ok(JfValue::Object(merged))
            }),
            vec![SignatureArg {
                types: vec![DataType::Object, DataType::Null],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "mid".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let start = to_integer(runtime, &args[1])?;
                let length = to_integer(runtime, &args[2])?;
                if start < 0 {
                    return Err(JsonFormulaError::evaluation(
                        "mid() requires a non-negative start position".to_string(),
                    ));
                }
                if length < 0 {
                    return Err(JsonFormulaError::evaluation(
                        "mid() requires a non-negative length parameter".to_string(),
                    ));
                }
                if is_array_type(get_type(&args[0])) {
                    if let JfValue::Array(items) = &args[0] {
                        let slice = items
                            .iter()
                            .skip(start as usize)
                            .take(length as usize)
                            .cloned()
                            .collect();
                        return Ok(JfValue::Array(slice));
                    }
                }
                let text = runtime.to_string(&args[0])?;
                let chars: Vec<char> = text.chars().collect();
                let slice: String = chars
                    .into_iter()
                    .skip(start as usize)
                    .take(length as usize)
                    .collect();
                Ok(JfValue::String(slice))
            }),
            sig!([DataType::String, DataType::Array]; [DataType::Number]; [DataType::Number]),
        ),
    );

    map.insert(
        "millisecond".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).timestamp_subsec_millis() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "min".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let values: Vec<f64> = args
                    .iter()
                    .flat_map(|v| flatten(v))
                    .filter_map(|v| match get_value_of(&v) {
                        JfValue::Number(n) => Some(n),
                        _ => None,
                    })
                    .collect();
                if values.is_empty() {
                    return Ok(JfValue::Number(0.0));
                }
                Ok(JfValue::Number(values.into_iter().fold(f64::MAX, f64::min)))
            }),
            vec![SignatureArg {
                types: vec![DataType::Array, DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "minA".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut values = Vec::new();
                for v in args.iter().flat_map(|v| flatten(v)) {
                    if matches!(get_value_of(&v), JfValue::Null) {
                        continue;
                    }
                    let n = runtime.to_number(&v)?;
                    values.push(n);
                }
                if values.is_empty() {
                    return Ok(JfValue::Number(0.0));
                }
                Ok(JfValue::Number(values.into_iter().fold(f64::MAX, f64::min)))
            }),
            vec![SignatureArg {
                types: vec![DataType::Array, DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "minute".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).minute() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "mod".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_binary(runtime, &args[0], &args[1], |a, b| {
                    let result = a % b;
                    if result.is_nan() {
                        return Err(JsonFormulaError::evaluation(format!(
                            "Bad parameter for mod: '{} % {}'",
                            a, b
                        )));
                    }
                    Ok(result)
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "month".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).month() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "not".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                Ok(JfValue::Bool(!to_boolean(&get_value_of(&args[0]))))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "notNull".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let result = args
                    .into_iter()
                    .find(|arg| get_type(arg) != DataType::Null);
                Ok(result.unwrap_or(JfValue::Null))
            }),
            vec![SignatureArg {
                types: vec![DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "now".to_string(),
        simple(
            Box::new(|_runtime, _args, _data, _interp| {
                let ms = chrono::Utc::now().timestamp_millis();
                Ok(JfValue::Number(ms as f64 / MS_IN_DAY))
            }),
            vec![],
        ),
    );

    map.insert(
        "null".to_string(),
        simple(Box::new(|_runtime, _args, _data, _interp| Ok(JfValue::Null)), vec![]),
    );

    map.insert(
        "or".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let mut result = to_boolean(&get_value_of(&args[0]));
                for arg in args.iter().skip(1) {
                    result = result || to_boolean(&get_value_of(arg));
                }
                Ok(JfValue::Bool(result))
            }),
            vec![SignatureArg {
                types: vec![DataType::Any],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map.insert(
        "power".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_binary(runtime, &args[0], &args[1], |a, b| {
                    let result = a.powf(b);
                    valid_number(result, "power")
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "proper".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_result(runtime, &args[0], |text| {
                    Ok(JfValue::String(proper(text)))
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "random".to_string(),
        simple(
            Box::new(|_runtime, _args, _data, _interp| {
                Ok(JfValue::Number(rand::random::<f64>()))
            }),
            vec![],
        ),
    );

    map.insert(
        "reduce".to_string(),
        simple(
            Box::new(|_runtime, args, _data, interp| {
                let array = match &args[0] {
                    JfValue::Array(items) => items.clone(),
                    _ => Vec::new(),
                };
                let expref = match &args[1] {
                    JfValue::Expref(node) => node.clone(),
                    _ => return Err(JsonFormulaError::ty("reduce() requires an expression")),
                };
                let mut accumulated = if args.len() == 3 {
                    args[2].clone()
                } else {
                    JfValue::Null
                };
                for (index, current) in array.iter().enumerate() {
                    let ctx = JfValue::Object(
                        [
                            ("accumulated".to_string(), accumulated),
                            ("current".to_string(), current.clone()),
                            ("index".to_string(), JfValue::Number(index as f64)),
                            ("array".to_string(), JfValue::Array(array.clone())),
                        ]
                        .into_iter()
                        .collect(),
                    );
                    accumulated = interp.visit(&expref, &ctx)?;
                }
                Ok(accumulated)
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Array],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Expref],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "register".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let function_name = match &args[0] {
                    JfValue::String(s) => s.clone(),
                    _ => {
                        return Err(JsonFormulaError::function(
                            "Invalid function name".to_string(),
                        ))
                    }
                };
                let expref = match &args[1] {
                    JfValue::Expref(node) => node.clone(),
                    _ => return Err(JsonFormulaError::ty("register() requires an expression")),
                };
                let expref_node = (*expref).clone();
                if !regex::Regex::new(r"^[_A-Z][_a-zA-Z0-9$]*$")
                    .unwrap()
                    .is_match(&function_name)
                {
                    return Err(JsonFormulaError::function(format!(
                        "Invalid function name: \"{}\"",
                        function_name
                    )));
                }
                if let Some(existing) = runtime.functions.get(&function_name) {
                    if existing.expref.as_ref() != Some(&expref) {
                        return Err(JsonFormulaError::function(format!(
                            "Cannot override function: \"{}\" with a different definition",
                            function_name
                        )));
                    }
                }
                let entry = FunctionEntry {
                    func: Box::new(move |_runtime, args, _data, interp| {
                        let value = if args.is_empty() {
                            JfValue::Null
                        } else {
                            args[0].clone()
                        };
                        interp.visit(&expref, &value)
                    }),
                    signature: vec![SignatureArg {
                        types: vec![DataType::Any],
                        optional: true,
                        variadic: false,
                    }],
                    expref: Some(expref_node),
                };
                runtime.functions.insert(function_name, entry);
                Ok(JfValue::Object(Default::default()))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::String],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Expref],
                    optional: false,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "registerWithParams".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let function_name = match &args[0] {
                    JfValue::String(s) => s.clone(),
                    _ => {
                        return Err(JsonFormulaError::function(
                            "Invalid function name".to_string(),
                        ))
                    }
                };
                let expref = match &args[1] {
                    JfValue::Expref(node) => node.clone(),
                    _ => return Err(JsonFormulaError::ty("registerWithParams() requires an expression")),
                };
                let expref_node = (*expref).clone();
                if !regex::Regex::new(r"^[_A-Z][_a-zA-Z0-9$]*$")
                    .unwrap()
                    .is_match(&function_name)
                {
                    return Err(JsonFormulaError::function(format!(
                        "Invalid function name: \"{}\"",
                        function_name
                    )));
                }
                if let Some(existing) = runtime.functions.get(&function_name) {
                    if existing.expref.as_ref() != Some(&expref) {
                        return Err(JsonFormulaError::function(format!(
                            "Cannot override function: \"{}\" with a different definition",
                            function_name
                        )));
                    }
                }
                let entry = FunctionEntry {
                    func: Box::new(move |_runtime, args, _data, interp| {
                        interp.visit(&expref, &JfValue::Array(args))
                    }),
                    signature: vec![SignatureArg {
                        types: vec![DataType::Any],
                        optional: true,
                        variadic: true,
                    }],
                    expref: Some(expref_node),
                };
                runtime.functions.insert(function_name, entry);
                Ok(JfValue::Object(Default::default()))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::String],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Expref],
                    optional: false,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "replace".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let start = to_integer(runtime, &args[1])?;
                let length = to_integer(runtime, &args[2])?;
                if start < 0 {
                    return Err(JsonFormulaError::evaluation(
                        "replace() start position must be greater than or equal to 0".to_string(),
                    ));
                }
                if length < 0 {
                    return Err(JsonFormulaError::evaluation(
                        "replace() length must be greater than or equal to 0".to_string(),
                    ));
                }
                if is_array_type(get_type(&args[0])) {
                    let mut source = match &args[0] {
                        JfValue::Array(items) => items.clone(),
                        _ => Vec::new(),
                    };
                    let mut replacement = match &args[3] {
                        JfValue::Array(items) => items.clone(),
                        _ => vec![args[3].clone()],
                    };
                    let start = start as usize;
                    let length = length as usize;
                    source.splice(start..(start + length).min(source.len()), replacement.drain(..));
                    return Ok(JfValue::Array(source));
                }
                if is_array_type(get_type(&args[3])) || get_type(&args[3]) == DataType::Object {
                    return Err(JsonFormulaError::ty(
                        "replace() replacement must not be an array or object".to_string(),
                    ));
                }
                let subject: Vec<char> = runtime.to_string(&args[0])?.chars().collect();
                let new_text = runtime.to_string(&args[3])?;
                let mut chars = subject;
                let start = start as usize;
                let length = length as usize;
                chars.splice(start..(start + length).min(chars.len()), new_text.chars());
                Ok(JfValue::String(chars.into_iter().collect()))
            }),
            sig!(
                [DataType::String, DataType::Array];
                [DataType::Number];
                [DataType::Number];
                [DataType::Any]
            ),
        ),
    );

    map.insert(
        "rept".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_number_recursive(runtime, &args[0], &args[1], |text, count| {
                    if count < 0 {
                        return Err(JsonFormulaError::evaluation(
                            "rept() count must be greater than or equal to 0".to_string(),
                        ));
                    }
                    Ok(JfValue::String(text.repeat(count as usize)))
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "reverse".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let val = get_value_of(&args[0]);
                match val {
                    JfValue::String(s) => {
                        let rev: String = s.chars().rev().collect();
                        Ok(JfValue::String(rev))
                    }
                    JfValue::Array(items) => {
                        let mut rev = items.clone();
                        rev.reverse();
                        Ok(JfValue::Array(rev))
                    }
                    _ => Ok(JfValue::Null),
                }
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "right".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let count = if args.len() > 1 {
                    to_integer(runtime, &args[1])?
                } else {
                    1
                };
                if count < 0 {
                    return Err(JsonFormulaError::evaluation(
                        "right() count must be greater than or equal to 0".to_string(),
                    ));
                }
                if is_array_type(get_type(&args[0])) {
                    if let JfValue::Array(items) = &args[0] {
                        if count == 0 {
                            return Ok(JfValue::Array(vec![]));
                        }
                        let start = items.len().saturating_sub(count as usize);
                        return Ok(JfValue::Array(items[start..].to_vec()));
                    }
                }
                if count == 0 {
                    return Ok(JfValue::String(String::new()));
                }
                let text = runtime.to_string(&args[0])?;
                let chars: Vec<char> = text.chars().collect();
                let start = chars.len().saturating_sub(count as usize);
                Ok(JfValue::String(chars[start..].iter().collect()))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::String, DataType::Array],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "round".to_string(),
        simple(
            Box::new(|runtime, mut args, _data, _interp| {
                if args.len() < 2 {
                    args.push(JfValue::Number(0.0));
                }
                evaluate(args, |vals| {
                    let a = runtime.to_number(&vals[0])?;
                    let digits = to_integer(runtime, &vals[1])?;
                    Ok(JfValue::Number(round(a, digits)))
                })
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "search".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let start = if args.len() > 2 {
                    args[2].clone()
                } else {
                    JfValue::Number(0.0)
                };
                map_search_recursive(runtime, &args[0], &args[1], &start)
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "second".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).second() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "sign".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    if n > 0.0 {
                        Ok(1.0)
                    } else if n < 0.0 {
                        Ok(-1.0)
                    } else {
                        Ok(0.0)
                    }
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "sin".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok(n.sin()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "sort".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let values = match &args[0] {
                    JfValue::Array(items) => items.clone(),
                    _ => Vec::new(),
                };
                let mut numbers = Vec::new();
                let mut strings = Vec::new();
                let mut bools = Vec::new();
                let mut nulls = Vec::new();
                for v in values {
                    match get_type(&v) {
                        DataType::Number => {
                            if let JfValue::Number(n) = v {
                                numbers.push(n);
                            }
                        }
                        DataType::String => {
                            if let JfValue::String(s) = v {
                                strings.push(s);
                            }
                        }
                        DataType::Boolean => {
                            if let JfValue::Bool(b) = v {
                                bools.push(b);
                            }
                        }
                        DataType::Null => nulls.push(()),
                        _ => {
                            return Err(JsonFormulaError::evaluation(
                                "Bad datatype for sort".to_string(),
                            ))
                        }
                    }
                }
                numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());
                strings.sort();
                let mut out = Vec::new();
                out.extend(numbers.into_iter().map(JfValue::Number));
                out.extend(strings.into_iter().map(JfValue::String));
                out.extend(bools.into_iter().map(JfValue::Bool));
                out.extend(nulls.into_iter().map(|_| JfValue::Null));
                Ok(JfValue::Array(out))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "sortBy".to_string(),
        simple(
            Box::new(|_runtime, args, _data, interp| {
                let mut array = match &args[0] {
                    JfValue::Array(items) => items.clone(),
                    _ => Vec::new(),
                };
                if array.is_empty() {
                    return Ok(JfValue::Array(array));
                }
                let expref = match &args[1] {
                    JfValue::Expref(node) => node.clone(),
                    _ => return Err(JsonFormulaError::ty("sortBy() requires an expression")),
                };
                let mut decorated: Vec<(usize, JfValue, JfValue)> = Vec::with_capacity(array.len());
                let mut first_type: Option<DataType> = None;
                for (idx, item) in array.iter().cloned().enumerate() {
                    let key = interp.visit(&expref, &item)?;
                    let key_type = get_type(&key);
                    if !matches!(key_type, DataType::Number | DataType::String) {
                        return Err(JsonFormulaError::ty("Bad data type for sortBy()".to_string()));
                    }
                    if let Some(first) = first_type {
                        if key_type != first {
                            return Err(JsonFormulaError::ty("Bad data type for sortBy()".to_string()));
                        }
                    } else {
                        first_type = Some(key_type);
                    }
                    decorated.push((idx, item, key));
                }
                decorated.sort_by(|a, b| {
                    let ord = match (&a.2, &b.2) {
                        (JfValue::Number(x), JfValue::Number(y)) => x.partial_cmp(&y).unwrap(),
                        (JfValue::String(x), JfValue::String(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if ord == std::cmp::Ordering::Equal {
                        a.0.cmp(&b.0)
                    } else {
                        ord
                    }
                });
                for (idx, (_, item, _)) in decorated.into_iter().enumerate() {
                    array[idx] = item;
                }
                Ok(JfValue::Array(array))
            }),
            sig!([DataType::Array]; [DataType::Expref]),
        ),
    );

    map.insert(
        "split".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_binary(runtime, &args[0], &args[1], |string, separator| {
                    if separator.is_empty() {
                        return Ok(JfValue::Array(
                            string
                                .chars()
                                .map(|c| JfValue::String(c.to_string()))
                                .collect(),
                        ));
                    }
                    let parts = string
                        .split(separator)
                        .map(|s| JfValue::String(s.to_string()))
                        .collect();
                    Ok(JfValue::Array(parts))
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "sqrt".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| valid_number(n.sqrt(), "sqrt"))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "startsWith".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_binary(runtime, &args[0], &args[1], |s, prefix| {
                    Ok(JfValue::Bool(starts_with(s, prefix)))
                })
            }),
            sig!([DataType::Any]; [DataType::Any]),
        ),
    );

    map.insert(
        "stdev".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let values: Vec<f64> = flatten(&args[0])
                    .into_iter()
                    .filter_map(|v| match get_value_of(&v) {
                        JfValue::Number(n) => Some(n),
                        _ => None,
                    })
                    .collect();
                if values.len() <= 1 {
                    return Err(JsonFormulaError::evaluation(
                        "stdev() must have at least two values".to_string(),
                    ));
                }
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let sum_square = values.iter().map(|v| v * v).sum::<f64>();
                let result = ((sum_square - values.len() as f64 * mean * mean)
                    / (values.len() as f64 - 1.0))
                    .sqrt();
                Ok(JfValue::Number(valid_number(result, "stdev")?))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "stdevA".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut values = Vec::new();
                for v in flatten(&args[0]) {
                    if matches!(get_type(&v), DataType::Null) {
                        continue;
                    }
                    let n = runtime.to_number(&v).map_err(|_| {
                        JsonFormulaError::evaluation("stdevA() received non-numeric parameters".to_string())
                    })?;
                    values.push(n);
                }
                if values.len() <= 1 {
                    return Err(JsonFormulaError::evaluation(
                        "stdevA() must have at least two values".to_string(),
                    ));
                }
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let sum_square = values.iter().map(|v| v * v).sum::<f64>();
                let result = ((sum_square - values.len() as f64 * mean * mean)
                    / (values.len() as f64 - 1.0))
                    .sqrt();
                Ok(JfValue::Number(valid_number(result, "stdevA")?))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "stdevp".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let values: Vec<f64> = flatten(&args[0])
                    .into_iter()
                    .filter_map(|v| match get_value_of(&v) {
                        JfValue::Number(n) => Some(n),
                        _ => None,
                    })
                    .collect();
                if values.is_empty() {
                    return Err(JsonFormulaError::evaluation(
                        "stdevp() must have at least one value".to_string(),
                    ));
                }
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let mean_square = values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64;
                let result = (mean_square - mean * mean).sqrt();
                Ok(JfValue::Number(valid_number(result, "stdevp")?))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "stdevpA".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut values = Vec::new();
                for v in flatten(&args[0]) {
                    if matches!(get_type(&v), DataType::Null) {
                        continue;
                    }
                    let n = runtime.to_number(&v)?;
                    values.push(n);
                }
                if values.is_empty() {
                    return Err(JsonFormulaError::evaluation(
                        "stdevp() must have at least one value".to_string(),
                    ));
                }
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let mean_square = values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64;
                let result = (mean_square - mean * mean).sqrt();
                Ok(JfValue::Number(valid_number(result, "stdevp")?))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "substitute".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let which = if args.len() > 3 {
                    Some(&args[3])
                } else {
                    None
                };
                map_substitute_recursive(runtime, &args[0], &args[1], &args[2], which)
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "sum".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let sum: f64 = flatten(&args[0])
                    .into_iter()
                    .filter_map(|v| match get_type(&v) {
                        DataType::Number => match v {
                            JfValue::Number(n) => Some(n),
                            _ => None,
                        },
                        _ => None,
                    })
                    .sum();
                Ok(JfValue::Number(sum))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "tan".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| Ok(n.tan()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "time".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let hours = to_integer(runtime, &args[0])?;
                let minutes = if args.len() > 1 {
                    to_integer(runtime, &args[1])?
                } else {
                    0
                };
                let seconds = if args.len() > 2 {
                    to_integer(runtime, &args[2])?
                } else {
                    0
                };
                Ok(JfValue::Number(datetime_to_num(
                    1970, 0, 1, hours, minutes, seconds, 0,
                )))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "toArray".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                if is_array_type(get_type(&args[0])) {
                    Ok(args[0].clone())
                } else {
                    Ok(JfValue::Array(vec![args[0].clone()]))
                }
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "toDate".to_string(),
        simple(
            Box::new(|runtime, args, _data, interp| {
                let iso = runtime.to_string(&args[0])?;
                match to_date(&iso, interp) {
                    Some(num) => Ok(JfValue::Number(num)),
                    None => Ok(JfValue::Null),
                }
            }),
            sig!([DataType::String]),
        ),
    );

    map.insert(
        "today".to_string(),
        simple(
            Box::new(|_runtime, _args, _data, _interp| {
                let now = Utc::now();
                let today = Utc
                    .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                    .earliest()
                    .unwrap();
                Ok(JfValue::Number(today.timestamp_millis() as f64 / MS_IN_DAY))
            }),
            vec![],
        ),
    );

    map.insert(
        "toNumber".to_string(),
        simple(
            Box::new(|runtime, args, _data, interp| {
                let base_val = if args.len() > 1 {
                    match &args[1] {
                        JfValue::Array(items) => JfValue::Array(
                            items
                                .iter()
                                .map(|v| to_integer(runtime, v).map(|n| JfValue::Number(n as f64)))
                                .collect::<Result<Vec<_>, _>>()?,
                        ),
                        _ => JfValue::Number(to_integer(runtime, &args[1])? as f64),
                    }
                } else {
                    JfValue::Number(10.0)
                };
                evaluate(vec![args[0].clone(), base_val], |vals| {
                    let base = match &vals[1] {
                        JfValue::Number(n) => *n as i64,
                        _ => 10,
                    };
                    to_number_base(runtime, &vals[0], base, interp)
                })
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number, DataType::ArrayNumber],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "toString".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let indent = if args.len() > 1 { to_indent(&args[1]) } else { 0 };
                Ok(JfValue::String(to_json_string(&get_value_of(&args[0]), indent)))
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Number],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "trim".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_result(runtime, &args[0], |text| {
                    let trimmed = text.split(' ').filter(|x| !x.is_empty()).collect::<Vec<_>>();
                    Ok(JfValue::String(trimmed.join(" ")))
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "true".to_string(),
        simple(Box::new(|_runtime, _args, _data, _interp| Ok(JfValue::Bool(true))), vec![]),
    );

    map.insert(
        "trunc".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let digits = if args.len() > 1 {
                    args[1].clone()
                } else {
                    JfValue::Number(0.0)
                };
                map_number_recursive_binary(runtime, &args[0], &digits, |number, digits| {
                    Ok(trunc(number, digits.trunc() as i64))
                })
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "type".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let t = match get_type(&args[0]) {
                    DataType::Number => "number",
                    DataType::String => "string",
                    DataType::Array | DataType::ArrayNumber | DataType::ArrayString | DataType::ArrayArray | DataType::EmptyArray => "array",
                    DataType::Object => "object",
                    DataType::Boolean => "boolean",
                    DataType::Expref => "expref",
                    DataType::Null => "null",
                    DataType::Any => "any",
                };
                Ok(JfValue::String(t.to_string()))
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "unique".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let array = match &args[0] {
                    JfValue::Array(items) => items.clone(),
                    _ => Vec::new(),
                };
                let values = array.iter().map(get_value_of).collect::<Vec<_>>();
                let mut out = Vec::new();
                for (idx, v) in values.iter().enumerate() {
                    if values
                        .iter()
                        .position(|lookup| strict_deep_equal(lookup, v))
                        == Some(idx)
                    {
                        out.push(array[idx].clone());
                    }
                }
                Ok(JfValue::Array(out))
            }),
            sig!([DataType::Array]),
        ),
    );

    map.insert(
        "upper".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_string_recursive_result(runtime, &args[0], |text| {
                    Ok(JfValue::String(text.to_uppercase()))
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "value".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let mut index = args[1].clone();
                let index_type = get_type(&index);
                let subject_array = is_array_type(get_type(&args[0]));
                if let JfValue::Field { .. } = &args[0] {
                    if let JfValue::String(key) = &args[1] {
                        if key.starts_with('$') {
                            return Ok(get_property(&args[0], key).unwrap_or(JfValue::Null));
                        }
                    }
                }
                let obj = get_value_of(&args[0]);
                if matches!(obj, JfValue::Null) {
                    return Ok(JfValue::Null);
                }
                if !matches!(get_type(&obj), DataType::Object) && !subject_array {
                    return Err(JsonFormulaError::ty(
                        "First parameter to value() must be one of: object, array, null.".to_string(),
                    ));
                }
                if subject_array {
                    if index_type != DataType::Number {
                        return Err(JsonFormulaError::ty(
                            "value() requires an integer index for arrays".to_string(),
                        ));
                    }
                    index = JfValue::Number(to_integer(runtime, &index)? as f64);
                } else if index_type != DataType::String {
                    return Err(JsonFormulaError::ty(
                        "value() requires a string index for objects".to_string(),
                    ));
                }
                let key = match index {
                    JfValue::String(s) => s,
                    JfValue::Number(n) => n.to_string(),
                    _ => return Ok(JfValue::Null),
                };
                let result = get_property(&obj, &key);
                if result.is_none() {
                    if subject_array {
                        if let JfValue::Array(items) = obj {
                            runtime
                                .functions
                                .get("debug")
                                .map(|_| ())
                                .unwrap_or(());
                            // no-op
                            let _ = items;
                        }
                        // note: no debug as we don't have access here
                    } else {
                        // no debug
                    }
                    return Ok(JfValue::Null);
                }
                Ok(result.unwrap())
            }),
            sig!([DataType::Any]; [DataType::String, DataType::Number]),
        ),
    );

    map.insert(
        "values".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                match get_value_of(&args[0]) {
                    JfValue::Object(map) => Ok(JfValue::Array(map.values().cloned().collect())),
                    _ => Ok(JfValue::Array(Vec::new())),
                }
            }),
            sig!([DataType::Object]),
        ),
    );

    map.insert(
        "weekday".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                let return_type = if args.len() > 1 {
                    args[1].clone()
                } else {
                    JfValue::Number(1.0)
                };
                map_weekday_recursive(runtime, &args[0], &return_type)
            }),
            vec![
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: false,
                    variadic: false,
                },
                SignatureArg {
                    types: vec![DataType::Any],
                    optional: true,
                    variadic: false,
                },
            ],
        ),
    );

    map.insert(
        "year".to_string(),
        simple(
            Box::new(|runtime, args, _data, _interp| {
                map_number_recursive_result(runtime, &args[0], |n| {
                    Ok(date_to_local(n).year() as f64)
                })
            }),
            sig!([DataType::Any]),
        ),
    );

    map.insert(
        "zip".to_string(),
        simple(
            Box::new(|_runtime, args, _data, _interp| {
                let arrays: Vec<Vec<JfValue>> = args
                    .iter()
                    .map(|v| match v {
                        JfValue::Array(items) => items.clone(),
                        _ => Vec::new(),
                    })
                    .collect();
                let count = arrays.iter().map(|a| a.len()).min().unwrap_or(0);
                let mut result = Vec::new();
                for i in 0..count {
                    let mut row = Vec::new();
                    for a in &arrays {
                        row.push(a[i].clone());
                    }
                    result.push(JfValue::Array(row));
                }
                Ok(JfValue::Array(result))
            }),
            vec![SignatureArg {
                types: vec![DataType::Array],
                optional: false,
                variadic: true,
            }],
        ),
    );

    map
}

fn evaluate<F>(args: Vec<JfValue>, mut f: F) -> Result<JfValue, JsonFormulaError>
where
    F: FnMut(Vec<JfValue>) -> Result<JfValue, JsonFormulaError>,
{
    fn inner<F>(args: Vec<JfValue>, f: &mut F) -> Result<JfValue, JsonFormulaError>
    where
        F: FnMut(Vec<JfValue>) -> Result<JfValue, JsonFormulaError>,
    {
        if args.iter().any(|a| matches!(a, JfValue::Array(_))) {
            let balanced = balance_arrays(&args);
            let mut results = Vec::new();
            for row in balanced {
                results.push(inner(row, f)?);
            }
            return Ok(JfValue::Array(results));
        }
        f(args)
    }
    inner(args, &mut f)
}

fn balance_arrays(list: &[JfValue]) -> Vec<Vec<JfValue>> {
    let max_len = list
        .iter()
        .filter_map(|v| match v {
            JfValue::Array(items) => Some(items.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let mut normalized = Vec::new();
    for v in list {
        match v {
            JfValue::Array(items) => {
                let mut out = items.clone();
                if out.len() < max_len {
                    out.resize(max_len, JfValue::Null);
                }
                normalized.push(out);
            }
            _ => normalized.push(vec![v.clone(); max_len]),
        }
    }
    let mut rows = Vec::new();
    for i in 0..max_len {
        let mut row = Vec::new();
        for arr in &normalized {
            row.push(arr[i].clone());
        }
        rows.push(row);
    }
    rows
}

fn flatten(value: &JfValue) -> Vec<JfValue> {
    match value {
        JfValue::Array(items) => items.iter().flat_map(flatten).collect(),
        _ => vec![value.clone()],
    }
}

fn map_number_recursive<F>(
    runtime: &Runtime,
    value: &JfValue,
    f: F,
) -> Result<JfValue, JsonFormulaError>
where
    F: Fn(f64) -> f64 + Copy,
{
    map_number_recursive_result(runtime, value, |n| Ok(f(n)))
}

fn map_number_recursive_result<F>(
    runtime: &Runtime,
    value: &JfValue,
    f: F,
) -> Result<JfValue, JsonFormulaError>
where
    F: Fn(f64) -> Result<f64, JsonFormulaError> + Copy,
{
    match value {
        JfValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(map_number_recursive_result(runtime, item, f)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let n = runtime.to_number(value)?;
            Ok(JfValue::Number(f(n)?))
        }
    }
}

fn map_number_recursive_binary<F>(
    runtime: &Runtime,
    left: &JfValue,
    right: &JfValue,
    f: F,
) -> Result<JfValue, JsonFormulaError>
where
    F: Fn(f64, f64) -> Result<f64, JsonFormulaError> + Copy,
{
    match (left, right) {
        (JfValue::Array(left_items), JfValue::Array(right_items)) => {
            let len = left_items.len().max(right_items.len());
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                let l = left_items.get(i).unwrap_or(&JfValue::Null);
                let r = right_items.get(i).unwrap_or(&JfValue::Null);
                out.push(map_number_recursive_binary(runtime, l, r, f)?);
            }
            Ok(JfValue::Array(out))
        }
        (JfValue::Array(left_items), _) => {
            let mut out = Vec::with_capacity(left_items.len());
            for item in left_items {
                out.push(map_number_recursive_binary(runtime, item, right, f)?);
            }
            Ok(JfValue::Array(out))
        }
        (_, JfValue::Array(right_items)) => {
            let mut out = Vec::with_capacity(right_items.len());
            for item in right_items {
                out.push(map_number_recursive_binary(runtime, left, item, f)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let l = runtime.to_number(left)?;
            let r = runtime.to_number(right)?;
            Ok(JfValue::Number(f(l, r)?))
        }
    }
}

fn map_string_recursive_result<F>(
    runtime: &Runtime,
    value: &JfValue,
    f: F,
) -> Result<JfValue, JsonFormulaError>
where
    F: Fn(&str) -> Result<JfValue, JsonFormulaError> + Copy,
{
    match value {
        JfValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(map_string_recursive_result(runtime, item, f)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let s = runtime.to_string(value)?;
            f(&s)
        }
    }
}

fn map_string_recursive_binary<F>(
    runtime: &Runtime,
    left: &JfValue,
    right: &JfValue,
    f: F,
) -> Result<JfValue, JsonFormulaError>
where
    F: Fn(&str, &str) -> Result<JfValue, JsonFormulaError> + Copy,
{
    match (left, right) {
        (JfValue::Array(left_items), JfValue::Array(right_items)) => {
            let len = left_items.len().max(right_items.len());
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                let l = left_items.get(i).unwrap_or(&JfValue::Null);
                let r = right_items.get(i).unwrap_or(&JfValue::Null);
                out.push(map_string_recursive_binary(runtime, l, r, f)?);
            }
            Ok(JfValue::Array(out))
        }
        (JfValue::Array(left_items), _) => {
            let mut out = Vec::with_capacity(left_items.len());
            for item in left_items {
                out.push(map_string_recursive_binary(runtime, item, right, f)?);
            }
            Ok(JfValue::Array(out))
        }
        (_, JfValue::Array(right_items)) => {
            let mut out = Vec::with_capacity(right_items.len());
            for item in right_items {
                out.push(map_string_recursive_binary(runtime, left, item, f)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let l = runtime.to_string(left)?;
            let r = runtime.to_string(right)?;
            f(&l, &r)
        }
    }
}

fn map_string_number_recursive<F>(
    runtime: &Runtime,
    text: &JfValue,
    count: &JfValue,
    f: F,
) -> Result<JfValue, JsonFormulaError>
where
    F: Fn(&str, i64) -> Result<JfValue, JsonFormulaError> + Copy,
{
    match (text, count) {
        (JfValue::Array(texts), JfValue::Array(counts)) => {
            let len = texts.len().max(counts.len());
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                let t = texts.get(i).unwrap_or(&JfValue::Null);
                let c = counts.get(i).unwrap_or(&JfValue::Null);
                out.push(map_string_number_recursive(runtime, t, c, f)?);
            }
            Ok(JfValue::Array(out))
        }
        (JfValue::Array(texts), _) => {
            let mut out = Vec::with_capacity(texts.len());
            for t in texts {
                out.push(map_string_number_recursive(runtime, t, count, f)?);
            }
            Ok(JfValue::Array(out))
        }
        (_, JfValue::Array(counts)) => {
            let mut out = Vec::with_capacity(counts.len());
            for c in counts {
                out.push(map_string_number_recursive(runtime, text, c, f)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let t = runtime.to_string(text)?;
            let c = to_integer(runtime, count)?;
            f(&t, c)
        }
    }
}

fn map_datedif_recursive(
    runtime: &Runtime,
    date1: &JfValue,
    date2: &JfValue,
    unit: &JfValue,
) -> Result<JfValue, JsonFormulaError> {
    if let (JfValue::Array(left), JfValue::Array(right)) = (date1, date2) {
        let len = left.len().max(right.len());
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let l = left.get(i).unwrap_or(&JfValue::Null);
            let r = right.get(i).unwrap_or(&JfValue::Null);
            let u = match unit {
                JfValue::Array(units) => units.get(i).unwrap_or(&JfValue::Null),
                _ => unit,
            };
            out.push(map_datedif_recursive(runtime, l, r, u)?);
        }
        return Ok(JfValue::Array(out));
    }
    match (date1, date2, unit) {
        (JfValue::Array(left), _, _) => {
            let mut out = Vec::with_capacity(left.len());
            for item in left {
                out.push(map_datedif_recursive(runtime, item, date2, unit)?);
            }
            Ok(JfValue::Array(out))
        }
        (_, JfValue::Array(right), _) => {
            let mut out = Vec::with_capacity(right.len());
            for item in right {
                out.push(map_datedif_recursive(runtime, date1, item, unit)?);
            }
            Ok(JfValue::Array(out))
        }
        (_, _, JfValue::Array(units)) => {
            let mut out = Vec::with_capacity(units.len());
            for item in units {
                out.push(map_datedif_recursive(runtime, date1, date2, item)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let d1 = runtime.to_number(date1)?;
            let d2 = runtime.to_number(date2)?;
            let u = runtime.to_string(unit)?;
            Ok(JfValue::Number(datedif(d1, d2, &u)? as f64))
        }
    }
}

fn map_weekday_recursive(
    runtime: &Runtime,
    date: &JfValue,
    return_type: &JfValue,
) -> Result<JfValue, JsonFormulaError> {
    match (date, return_type) {
        (JfValue::Array(dates), JfValue::Array(types)) => {
            let len = dates.len().max(types.len());
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                let d = dates.get(i).unwrap_or(&JfValue::Null);
                let t = types.get(i).unwrap_or(&JfValue::Null);
                out.push(map_weekday_recursive(runtime, d, t)?);
            }
            Ok(JfValue::Array(out))
        }
        (JfValue::Array(dates), _) => {
            let mut out = Vec::with_capacity(dates.len());
            for d in dates {
                out.push(map_weekday_recursive(runtime, d, return_type)?);
            }
            Ok(JfValue::Array(out))
        }
        (_, JfValue::Array(types)) => {
            let mut out = Vec::with_capacity(types.len());
            for t in types {
                out.push(map_weekday_recursive(runtime, date, t)?);
            }
            Ok(JfValue::Array(out))
        }
        _ => {
            let d = runtime.to_number(date)?;
            let t = to_integer(runtime, return_type)?;
            Ok(JfValue::Number(weekday(d, t)? as f64))
        }
    }
}

fn map_find_recursive(
    runtime: &Runtime,
    query: &JfValue,
    text: &JfValue,
    offset: &JfValue,
) -> Result<JfValue, JsonFormulaError> {
    let is_array = |v: &JfValue| matches!(v, JfValue::Array(_));
    if is_array(query) || is_array(text) || is_array(offset) {
        let q_len = match query {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let t_len = match text {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let o_len = match offset {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let len = q_len.max(t_len).max(o_len);
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let q = match query {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => query,
            };
            let t = match text {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => text,
            };
            let o = match offset {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => offset,
            };
            out.push(map_find_recursive(runtime, q, t, o)?);
        }
        return Ok(JfValue::Array(out));
    }
    let q = runtime.to_string(query)?;
    let t = runtime.to_string(text)?;
    let off = to_integer(runtime, offset)?;
    if off < 0 {
        return Err(JsonFormulaError::evaluation(
            "find() start position must be >= 0".to_string(),
        ));
    }
    Ok(match find_text(&q, &t, off) {
        Some(idx) => JfValue::Number(idx as f64),
        None => JfValue::Null,
    })
}

fn map_search_recursive(
    runtime: &Runtime,
    find_text: &JfValue,
    within_text: &JfValue,
    start: &JfValue,
) -> Result<JfValue, JsonFormulaError> {
    let is_array = |v: &JfValue| matches!(v, JfValue::Array(_));
    if is_array(find_text) || is_array(within_text) || is_array(start) {
        let f_len = match find_text {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let w_len = match within_text {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let s_len = match start {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let len = f_len.max(w_len).max(s_len);
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let f = match find_text {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => find_text,
            };
            let w = match within_text {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => within_text,
            };
            let s = match start {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => start,
            };
            out.push(map_search_recursive(runtime, f, w, s)?);
        }
        return Ok(JfValue::Array(out));
    }
    let f = runtime.to_string(find_text)?;
    let w = runtime.to_string(within_text)?;
    let s = to_integer(runtime, start)?;
    if s < 0 {
        return Err(JsonFormulaError::function(
            "search() startPos must be greater than or equal to 0".to_string(),
        ));
    }
    if w.is_empty() {
        return Ok(JfValue::Array(vec![]));
    }
    if f.is_empty() {
        return Ok(JfValue::Array(vec![
            JfValue::Number(s as f64),
            JfValue::String(String::new()),
        ]));
    }
    Ok(JfValue::Array(wildcard_search(&f, &w, s as usize)))
}

fn map_substitute_recursive(
    runtime: &Runtime,
    source: &JfValue,
    old: &JfValue,
    replacement: &JfValue,
    which: Option<&JfValue>,
) -> Result<JfValue, JsonFormulaError> {
    let is_array = |v: &JfValue| matches!(v, JfValue::Array(_));
    let which_is_array = which.map_or(false, is_array);
    if is_array(source) || is_array(old) || is_array(replacement) || which_is_array {
        let s_len = match source {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let o_len = match old {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let r_len = match replacement {
            JfValue::Array(items) => items.len(),
            _ => 0,
        };
        let w_len = match which {
            Some(JfValue::Array(items)) => items.len(),
            _ => 0,
        };
        let len = s_len.max(o_len).max(r_len).max(w_len);
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let s = match source {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => source,
            };
            let o = match old {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => old,
            };
            let r = match replacement {
                JfValue::Array(items) => items.get(i).unwrap_or(&JfValue::Null),
                _ => replacement,
            };
            let w = match which {
                Some(JfValue::Array(items)) => Some(items.get(i).unwrap_or(&JfValue::Null)),
                Some(other) => Some(other),
                None => None,
            };
            out.push(map_substitute_recursive(runtime, s, o, r, w)?);
        }
        return Ok(JfValue::Array(out));
    }
    let source = runtime.to_string(source)?;
    let old = runtime.to_string(old)?;
    let replacement = runtime.to_string(replacement)?;
    let which = if let Some(which) = which {
        let n = to_integer(runtime, which)?;
        if n < 0 {
            return Err(JsonFormulaError::evaluation(
                "substitute() which parameter must be greater than or equal to 0".to_string(),
            ));
        }
        n
    } else {
        -1
    };
    Ok(JfValue::String(substitute(
        &source,
        &old,
        &replacement,
        which,
    )))
}

fn round(num: f64, digits: i64) -> f64 {
    let precision = 10_f64.powi(digits as i32);
    ((num * precision) + 0.5).floor() / precision
}

fn trunc(num: f64, digits: i64) -> f64 {
    let method = if num >= 0.0 { f64::floor } else { f64::ceil };
    method(num * 10_f64.powi(digits as i32)) / 10_f64.powi(digits as i32)
}

fn valid_number(n: f64, context: &str) -> Result<f64, JsonFormulaError> {
    if n.is_nan() || !n.is_finite() {
        return Err(JsonFormulaError::evaluation(format!(
            "Call to \"{}()\" resulted in an invalid number",
            context
        )));
    }
    Ok(n)
}

fn to_integer(runtime: &Runtime, value: &JfValue) -> Result<i64, JsonFormulaError> {
    let n = match get_type(value) {
        DataType::String => runtime.to_number(value)?,
        DataType::Number => runtime.to_number(value)?,
        DataType::Boolean => runtime.to_number(value)?,
        _ => runtime.to_number(value)?,
    };
    if n.is_nan() {
        Ok(0)
    } else {
        Ok(n.trunc() as i64)
    }
}

fn to_indent(value: &JfValue) -> usize {
    match value {
        JfValue::Number(n) => n.trunc().max(0.0) as usize,
        _ => 0,
    }
}

fn to_json_string(value: &JfValue, indent: usize) -> String {
    if indent == 0 {
        return match value {
            JfValue::String(s) => s.clone(),
            JfValue::Number(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            JfValue::Bool(b) => b.to_string(),
            JfValue::Null => "null".to_string(),
            _ => {
                let json = value.to_json();
                serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string())
            }
        };
    }
    let json = value.to_json();
    let mut buf = Vec::new();
    let indent_buf = vec![b' '; indent];
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_buf.as_slice());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    json.serialize(&mut ser).ok();
    match value {
        JfValue::String(s) => s.clone(),
        _ => String::from_utf8(buf).unwrap_or_else(|_| "null".to_string()),
    }
}

fn to_json_for_join(value: &JfValue, indent: usize) -> String {
    match value {
        JfValue::String(s) => s.clone(),
        JfValue::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        _ => to_json_string(value, indent),
    }
}

fn date_to_local(date_num: f64) -> chrono::DateTime<Utc> {
    let ms = (date_num * MS_IN_DAY).round() as i64;
    chrono::DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(|| chrono::DateTime::<Utc>::from_timestamp_millis(0).unwrap())
}

fn datetime_to_num(
    year: i64,
    month_zero_based: i64,
    day: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    ms: i64,
) -> f64 {
    let (year, month) = normalize_year_month(year, month_zero_based);
    let base_date = NaiveDate::from_ymd_opt(year as i32, month as u32, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let base_dt = base_date.and_hms_milli_opt(0, 0, 0, 0).unwrap();
    let dt = base_dt
        + Duration::days(day.saturating_sub(1))
        + Duration::hours(hours)
        + Duration::minutes(minutes)
        + Duration::seconds(seconds)
        + Duration::milliseconds(ms);
    let utc = Utc.from_utc_datetime(&dt);
    utc.timestamp_millis() as f64 / MS_IN_DAY
}

fn normalize_year_month(year: i64, month_zero_based: i64) -> (i64, i64) {
    let mut y = year;
    let mut m = month_zero_based;
    if m >= 12 || m < 0 {
        y += m.div_euclid(12);
        m = m.rem_euclid(12);
    }
    (y, m + 1)
}

fn datedif(date1: f64, date2: f64, unit: &str) -> Result<i64, JsonFormulaError> {
    let unit = unit.to_lowercase();
    let d1 = date_to_local(date1);
    let d2 = date_to_local(date2);
    if d2 == d1 {
        return Ok(0);
    }
    if d2 < d1 {
        return Err(JsonFormulaError::function(
            "end_date must be >= start_date in datedif()".to_string(),
        ));
    }
    if unit == "d" {
        let diff = d2.signed_duration_since(d1).num_days();
        return Ok(diff);
    }
    let year_diff = d2.year() as i64 - d1.year() as i64;
    let mut month_diff = d2.month() as i64 - d1.month() as i64;
    let day_diff = d2.day() as i64 - d1.day() as i64;
    if unit == "y" {
        let mut y = year_diff;
        if month_diff < 0 {
            y -= 1;
        }
        if month_diff == 0 && day_diff < 0 {
            y -= 1;
        }
        return Ok(y);
    }
    if unit == "m" {
        return Ok(year_diff * 12 + month_diff + if day_diff < 0 { -1 } else { 0 });
    }
    if unit == "ym" {
        if day_diff < 0 {
            month_diff -= 1;
        }
        if month_diff <= 0 && year_diff > 0 {
            return Ok(12 + month_diff);
        }
        return Ok(month_diff);
    }
    if unit == "yd" {
        if day_diff < 0 {
            month_diff -= 1;
        }
        let mut d2_adj = d2;
        if month_diff < 0 {
            d2_adj = d2_adj.with_year(d1.year() + 1).unwrap();
        } else {
            d2_adj = d2_adj.with_year(d1.year()).unwrap();
        }
        let diff = d2_adj.signed_duration_since(d1).num_days();
        return Ok(diff);
    }
    Err(JsonFormulaError::function(format!(
        "Unrecognized unit parameter \"{}\" for datedif()",
        unit
    )))
}

fn ends_with(search: &str, suffix: &str) -> bool {
    let search: Vec<char> = search.chars().collect();
    let suffix: Vec<char> = suffix.chars().collect();
    if suffix.len() > search.len() {
        return false;
    }
    search[search.len() - suffix.len()..] == suffix[..]
}

fn starts_with(subject: &str, prefix: &str) -> bool {
    let subject: Vec<char> = subject.chars().collect();
    let prefix: Vec<char> = prefix.chars().collect();
    if prefix.len() > subject.len() {
        return false;
    }
    subject[..prefix.len()] == prefix[..]
}

fn find_text(query: &str, text: &str, offset: i64) -> Option<i64> {
    let query_chars: Vec<char> = query.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    if query_chars.is_empty() {
        return if offset as usize > text_chars.len() {
            None
        } else {
            Some(offset)
        };
    }
    for i in offset.max(0) as usize..text_chars.len() {
        if i + query_chars.len() <= text_chars.len()
            && text_chars[i..i + query_chars.len()] == query_chars[..]
        {
            return Some(i as i64);
        }
    }
    None
}

fn eomonth(date: f64, months: i64) -> f64 {
    let js_date = date_to_local(date);
    let year = js_date.year() as i64;
    let month = js_date.month0() as i64 + months + 1;
    let (y, m) = normalize_year_month(year, month);
    let new_date = NaiveDate::from_ymd_opt(y as i32, m as u32, 1)
        .unwrap()
        .pred_opt()
        .unwrap();
    let dt = Utc.from_utc_datetime(&new_date.and_hms_opt(0, 0, 0).unwrap());
    dt.timestamp_millis() as f64 / MS_IN_DAY
}

fn wildcard_search(find_text: &str, within_text: &str, start: usize) -> Vec<JfValue> {
    let glob = parse_glob(find_text);
    let within: Vec<char> = within_text.chars().collect();
    for i in start..within.len() {
        if let Some(matched) = test_match(&within[i..], &glob) {
            return vec![JfValue::Number(i as f64), JfValue::String(matched.into_iter().collect())];
        }
    }
    Vec::new()
}

fn parse_glob(text: &str) -> Vec<GlobToken> {
    let mut result = Vec::new();
    let mut escape = false;
    for ch in text.chars() {
        if escape {
            result.push(GlobToken::Char(ch));
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch == '?' {
            result.push(GlobToken::Dot);
            continue;
        }
        if ch == '*' {
            if !matches!(result.last(), Some(GlobToken::Star)) {
                result.push(GlobToken::Star);
            }
            continue;
        }
        result.push(GlobToken::Char(ch));
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum GlobToken {
    Char(char),
    Dot,
    Star,
}

fn test_match(array: &[char], glob: &[GlobToken]) -> Option<Vec<char>> {
    if glob.is_empty() {
        return Some(Vec::new());
    }
    if array.is_empty() {
        if glob.first() == Some(&GlobToken::Star) {
            if glob.len() == 1 {
                return Some(Vec::new());
            }
            return test_match(array, &glob[1..]);
        }
        return None;
    }
    let test_char = array[0];
    let mut glob_iter = glob.iter();
    let first = glob_iter.next().unwrap();
    if *first == GlobToken::Star {
        let rest = &glob[1..];
        if let Some(match_next) = test_match(array, rest) {
            return Some(match_next);
        }
        if glob.len() == 1 {
            return Some(vec![test_char]);
        }
        if let Some(mut match_next) = test_match(&array[1..], rest) {
            match_next.insert(0, test_char);
            return Some(match_next);
        }
        if let Some(mut match_next) = test_match(&array[1..], glob) {
            match_next.insert(0, test_char);
            return Some(match_next);
        }
        return None;
    }
    if *first == GlobToken::Dot || matches!(*first, GlobToken::Char(c) if c == test_char) {
        if let Some(mut match_next) = test_match(&array[1..], &glob[1..]) {
            match_next.insert(0, test_char);
            return Some(match_next);
        }
    }
    None
}

fn substitute(source: &str, old: &str, replacement: &str, which: i64) -> String {
    if old.is_empty() {
        return source.to_string();
    }
    let src: Vec<char> = source.chars().collect();
    let old_chars: Vec<char> = old.chars().collect();
    let replacement: Vec<char> = replacement.chars().collect();
    let replace_all = which < 0;
    let mut found = 0;
    let mut result = Vec::new();
    let mut j = 0;
    while j < src.len() {
        let match_here = j + old_chars.len() <= src.len()
            && src[j..j + old_chars.len()] == old_chars[..];
        if match_here {
            found += 1;
        }
        if match_here && (replace_all || found == which + 1) {
            result.extend(&replacement);
            j += old_chars.len();
        } else {
            result.push(src[j]);
            j += 1;
        }
    }
    result.into_iter().collect()
}

fn proper(text: &str) -> String {
    let mut out = String::new();
    let mut start_word = true;
    for ch in text.chars() {
        if ch.is_whitespace() || ch.is_ascii_digit() || ch.is_ascii_punctuation() {
            start_word = true;
            out.push(ch);
        } else if start_word {
            out.extend(ch.to_uppercase());
            start_word = false;
        } else {
            out.extend(ch.to_lowercase());
        }
    }
    out
}

fn to_date(iso: &str, interp: &mut Interpreter) -> Option<f64> {
    let re_date = regex::Regex::new(r"(\d\d\d\d)(\d\d)(\d\d)").unwrap();
    let re_time = regex::Regex::new(r"T(\d\d)(\d\d)(\d\d)").unwrap();
    let mut expanded = re_date.replace(iso, "$1-$2-$3").to_string();
    expanded = re_time.replace(&expanded, "T$1:$2:$3").to_string();
    if !expanded.contains('T') && expanded.len() == 8 {
        expanded = format!(
            "{}-{}-{}",
            &expanded[0..4],
            &expanded[4..6],
            &expanded[6..8]
        );
    }
    let tz_offset = regex::Regex::new(r"([+-])(\d{2})(\d{2})$").unwrap();
    if tz_offset.is_match(&expanded) {
        expanded = tz_offset.replace(&expanded, "$1$2:$3").to_string();
    }
    let has_timezone = expanded.ends_with('Z')
        || expanded.ends_with('z')
        || regex::Regex::new(r"[+-]\d{2}:\d{2}$")
            .unwrap()
            .is_match(&expanded);
    let dateparts = regex::Regex::new(r"[\D,zZ]+")
        .unwrap()
        .split(&expanded)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i64>().ok())
        .collect::<Vec<_>>();
    if dateparts.len() <= 3 {
        if dateparts.len() < 3 || dateparts.iter().any(|x| x.is_none()) {
            interp
                .debug_mut()
                .push(format!("Failed to convert \"{}\" to a date", iso));
            return None;
        }
    }
    if !has_timezone && dateparts.len() <= 7 {
        let ranges = [99999, 12, 31, 23, 59, 59, 999];
        for (idx, part) in dateparts.iter().enumerate() {
            if let Some(v) = part {
                if *v > ranges[idx] {
                    interp
                        .debug_mut()
                        .push(format!("Failed to convert \"{}\" to a date", iso));
                    return None;
                }
            }
        }
        let year = dateparts[0]? as i64;
        let month = dateparts.get(1).and_then(|v| *v).unwrap_or(1) as i64 - 1;
        let day = dateparts.get(2).and_then(|v| *v).unwrap_or(1) as i64;
        let hour = dateparts.get(3).and_then(|v| *v).unwrap_or(0) as i64;
        let minute = dateparts.get(4).and_then(|v| *v).unwrap_or(0) as i64;
        let second = dateparts.get(5).and_then(|v| *v).unwrap_or(0) as i64;
        let ms = dateparts.get(6).and_then(|v| *v).unwrap_or(0) as i64;
        return Some(datetime_to_num(year, month, day, hour, minute, second, ms));
    }
    if has_timezone {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&expanded) {
            return Some(dt.timestamp_millis() as f64 / MS_IN_DAY);
        }
    }
    interp
        .debug_mut()
        .push(format!("Failed to convert \"{}\" to a date", iso));
    None
}

fn to_number_base(
    runtime: &Runtime,
    value: &JfValue,
    base: i64,
    interp: &mut Interpreter,
) -> Result<JfValue, JsonFormulaError> {
    let num = get_value_of(value);
    if matches!(get_type(&num), DataType::String) {
        let s = runtime.to_string(&num)?;
        if s.trim().is_empty() {
            return Ok(JfValue::Number(0.0));
        }
        if base == 10 {
            return match runtime.to_number(&num) {
                Ok(n) => Ok(JfValue::Number(n)),
                Err(_) => {
                    interp
                        .debug_mut()
                        .push(format!("Failed to convert \"{}\" to number", to_json_string(&num, 0)));
                    Ok(JfValue::Null)
                }
            };
        }
        let digit_check = match base {
            2 => regex::Regex::new(r"^\s*(\+|-)?[01.]+\s*$").unwrap(),
            8 => regex::Regex::new(r"^\s*(\+|-)?[0-7.]+\s*$").unwrap(),
            16 => regex::Regex::new(r"^\s*(\+|-)?[0-9A-Fa-f.]+\s*$").unwrap(),
            _ => {
                return Err(JsonFormulaError::evaluation(format!(
                    "Invalid base: \"{}\" for toNumber()",
                    base
                )))
            }
        };
        if !digit_check.is_match(&s) {
            interp
                .debug_mut()
                .push(format!("Failed to convert \"{}\" base \"{}\" to number", s, base));
            return Ok(JfValue::Null);
        }
        let parts: Vec<&str> = s.split('.').map(|p| p.trim()).collect();
        if parts.len() > 2 {
            interp
                .debug_mut()
                .push(format!("Failed to convert \"{}\" base \"{}\" to number", s, base));
            return Ok(JfValue::Null);
        }
        let int_part = i64::from_str_radix(parts[0].trim_start_matches('+'), base as u32)
            .map_err(|_| JsonFormulaError::evaluation("Invalid base".to_string()))?;
        let mut result = int_part as f64;
        if parts.len() == 2 {
            let frac = i64::from_str_radix(parts[1], base as u32).unwrap_or(0);
            result += frac as f64 * (base as f64).powi(-(parts[1].len() as i32));
        }
        return Ok(JfValue::Number(result));
    }
    match runtime.to_number(&num) {
        Ok(n) => Ok(JfValue::Number(n)),
        Err(_) => {
            interp
                .debug_mut()
                .push(format!("Failed to convert \"{}\" to number", to_json_string(&num, 0)));
            Ok(JfValue::Null)
        }
    }
}

fn weekday(date: f64, return_type: i64) -> Result<i64, JsonFormulaError> {
    let day = date_to_local(date).weekday().num_days_from_sunday() as i64;
    match return_type {
        1 => Ok(day + 1),
        2 => Ok(((day + 6) % 7) + 1),
        3 => Ok((day + 6) % 7),
        _ => Err(JsonFormulaError::function(format!(
            "Unsupported returnType: \"{}\" for weekday()",
            return_type
        ))),
    }
}

fn casefold_locale(text: &str, locale: &str) -> String {
    let locale = locale.to_lowercase();
    if locale.starts_with("tr") || locale.starts_with("az") {
        let mut upper = String::new();
        for ch in text.chars() {
            match ch {
                'i' => upper.push('İ'),
                'ı' => upper.push('I'),
                _ => upper.extend(ch.to_uppercase()),
            }
        }
        let mut lower = String::new();
        for ch in upper.chars() {
            match ch {
                'I' => lower.push('ı'),
                'İ' => lower.push('i'),
                _ => lower.extend(ch.to_lowercase()),
            }
        }
        lower
    } else {
        text.to_uppercase().to_lowercase()
    }
}
