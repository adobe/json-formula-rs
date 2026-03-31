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

use std::fs;
use std::path::Path;

use json_formula_rs::JsonFormula;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct TestSuite {
    given: Value,
    cases: Vec<TestCase>,
    #[allow(dead_code)]
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestCase {
    expression: String,
    result: Option<Option<Value>>,
    error: Option<String>,
    data: Option<Value>,
    language: Option<String>,
    fields_only: Option<bool>,
    #[allow(dead_code)]
    precedence: Option<String>,
    #[allow(dead_code)]
    comment: Option<String>,
}

#[test]
fn official_test_suite() {
    let mut engine = JsonFormula::new();
    let mut globals = serde_json::json!({
        "$days": ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
        "$": 42,
        "$$": 43
    });

    let _ = engine.evaluate(
        r#"register("_summarize",
            &reduce(
              @,
              &merge(accumulated, fromEntries([[current, 1 + value(accumulated, current)]])),
              fromEntries(map(@, &[@, 0]))
            )
          )"#,
        &serde_json::json!({}),
        Some(&globals),
        Some("en-US"),
        false,
    );
    let _ = engine.evaluate(
        r#"register(
            "_localDate",
            &split(@, "-") | datetime(@[0], @[1], @[2]))"#,
        &serde_json::json!({}),
        Some(&globals),
        Some("en-US"),
        false,
    );
    let fixture_dir = Path::new("tests/fixtures");

    let entries = fs::read_dir(fixture_dir).expect("failed to read fixtures directory");
    for entry in entries {
        let entry = entry.expect("failed to read fixture entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let content = fs::read_to_string(&path).expect("failed to read fixture file");
        let content = sanitize_unpaired_surrogates(&content);
        let suites: Vec<TestSuite> =
            serde_json::from_str(&content).expect("failed to parse fixture file");

        for (suite_idx, suite) in suites.into_iter().enumerate() {
            for (case_idx, case) in suite.cases.into_iter().enumerate() {
                let fields_only = case.fields_only.unwrap_or(false);
                let language = case.language.as_deref();
                let base_data = suite.given.clone();

                let data = match case.data {
                    None => base_data,
                    Some(Value::String(expr)) => {
                        let outcome = engine
                            .evaluate(&expr, &base_data, Some(&globals), language, fields_only)
                            .unwrap_or_else(|err| {
                                panic!(
                                    "failed to evaluate data expression {} (fixture {}, suite {}, case {}): {:?}",
                                    expr,
                                    path.display(),
                                    suite_idx,
                                    case_idx,
                                    err
                                )
                            });
                        outcome
                    }
                    Some(value) => value,
                };
                globals["$form"] = data.clone();

                match (case.result, case.error) {
                    (Some(expected), None) => {
                        let expected = expected.unwrap_or(Value::Null);
                        let actual = engine
                            .evaluate(&case.expression, &data, Some(&globals), language, fields_only)
                            .unwrap_or_else(|err| {
                                panic!(
                                    "failed to evaluate expression {} (fixture {}, suite {}, case {}): {:?}",
                                    case.expression,
                                    path.display(),
                                    suite_idx,
                                    case_idx,
                                    err
                                )
                            });
                        assert_json_eq(
                            &expected,
                            &actual,
                            &format!(
                                "fixture {} suite {} case {} expression {}",
                                path.display(),
                                suite_idx,
                                case_idx,
                                case.expression
                            ),
                        );
                    }
                    (None, None) => {
                        let actual = engine
                            .evaluate(&case.expression, &data, Some(&globals), language, fields_only)
                            .unwrap_or_else(|err| {
                                panic!(
                                    "failed to evaluate expression {} (fixture {}, suite {}, case {}): {:?}",
                                    case.expression,
                                    path.display(),
                                    suite_idx,
                                    case_idx,
                                    err
                                )
                            });
                        assert_json_eq(
                            &Value::Null,
                            &actual,
                            &format!(
                                "fixture {} suite {} case {} expression {}",
                                path.display(),
                                suite_idx,
                                case_idx,
                                case.expression
                            ),
                        );
                    }
                    (None, Some(expected_error)) => {
                        let err = engine
                            .evaluate(&case.expression, &data, Some(&globals), language, fields_only)
                            .expect_err(&format!(
                                "expected error for expression {} (fixture {}, suite {}, case {})",
                                case.expression,
                                path.display(),
                                suite_idx,
                                case_idx
                            ));
                        let actual_name = match err.kind {
                            json_formula_rs::JsonFormulaErrorKind::SyntaxError => "SyntaxError",
                            json_formula_rs::JsonFormulaErrorKind::TypeError => "TypeError",
                            json_formula_rs::JsonFormulaErrorKind::FunctionError => "FunctionError",
                            json_formula_rs::JsonFormulaErrorKind::EvaluationError => "EvaluationError",
                        };
                        assert_eq!(
                            actual_name, expected_error,
                            "fixture {} suite {} case {}: error mismatch",
                            path.display(),
                            suite_idx,
                            case_idx
                        );
                    }
                    _ => {
                        panic!(
                            "fixture {} suite {} case {}: test must define result or error",
                            path.display(),
                            suite_idx,
                            case_idx
                        );
                    }
                }
            }
        }
    }
}

fn sanitize_unpaired_surrogates(input: &str) -> String {
    fn is_hex(byte: u8) -> bool {
        matches!(byte, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
    }
    fn parse_hex4(bytes: &[u8]) -> Option<u16> {
        if bytes.len() != 4 || !bytes.iter().all(|b| is_hex(*b)) {
            return None;
        }
        let s = std::str::from_utf8(bytes).ok()?;
        u16::from_str_radix(s, 16).ok()
    }

    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut last = 0;
    while i < bytes.len() {
        let mut backslashes = 0usize;
        let mut j = i;
        while j > 0 && bytes[j - 1] == b'\\' {
            backslashes += 1;
            j -= 1;
        }
        let escaped = backslashes % 2 == 1;
        if bytes[i] == b'\\'
            && i + 5 < bytes.len()
            && bytes[i + 1] == b'u'
            && !escaped
            && parse_hex4(&bytes[i + 2..i + 6]).is_some()
        {
            if last < i {
                out.push_str(&input[last..i]);
            }
            let code = parse_hex4(&bytes[i + 2..i + 6]).unwrap();
            if (0xD800..=0xDBFF).contains(&code) {
                let has_low = if i + 11 < bytes.len()
                    && bytes[i + 6] == b'\\'
                    && bytes[i + 7] == b'u'
                {
                    parse_hex4(&bytes[i + 8..i + 12])
                        .map(|low| (0xDC00..=0xDFFF).contains(&low))
                        .unwrap_or(false)
                } else {
                    false
                };
                if has_low {
                    out.push_str(&input[i..i + 12]);
                    i += 12;
                    last = i;
                    continue;
                }
                out.push_str("\\uFFFD");
                i += 6;
                last = i;
                continue;
            }
            if (0xDC00..=0xDFFF).contains(&code) {
                out.push_str("\\uFFFD");
                i += 6;
                last = i;
                continue;
            }
            out.push_str(&input[i..i + 6]);
            i += 6;
            last = i;
            continue;
        }
        i += 1;
    }
    if last < bytes.len() {
        out.push_str(&input[last..]);
    }
    out
}

fn assert_json_eq(expected: &Value, actual: &Value, context: &str) {
    match (expected, actual) {
        (Value::Number(left), Value::Number(right)) => {
            let left = left.as_f64().unwrap_or(f64::NAN);
            let right = right.as_f64().unwrap_or(f64::NAN);
            let delta = (left - right).abs();
            assert!(
                delta <= 1e-9 || (left.is_nan() && right.is_nan()),
                "{}: numeric mismatch {} != {}",
                context,
                left,
                right
            );
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(
                left.len(),
                right.len(),
                "{}: array length mismatch {} != {}",
                context,
                left.len(),
                right.len()
            );
            for (idx, (l, r)) in left.iter().zip(right.iter()).enumerate() {
                assert_json_eq(l, r, &format!("{}[{}]", context, idx));
            }
        }
        (Value::Object(left), Value::Object(right)) => {
            assert_eq!(
                left.len(),
                right.len(),
                "{}: object size mismatch {} != {}",
                context,
                left.len(),
                right.len()
            );
            for (key, lvalue) in left {
                let rvalue = right.get(key).unwrap_or_else(|| {
                    panic!("{}: missing key {}", context, key);
                });
                assert_json_eq(lvalue, rvalue, &format!("{}.{}", context, key));
            }
        }
        _ => {
            assert_eq!(
                expected, actual,
                "{}: value mismatch {:?} != {:?}",
                context, expected, actual
            );
        }
    }
}
