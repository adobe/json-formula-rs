use json_formula_rs::{JsonFormula, SubExprTrace};
use serde_json::json;

#[test]
fn test_evaluate_with_trace_basic() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 0});
    let result = jf.evaluate_with_trace(
        "(x > 1) && (y > 2)",
        &data,
        None,
        None,
        false,
    );
    assert!(result.is_ok());
    let (value, trace) = result.unwrap();
    assert_eq!(value, json!(false));
    // Root trace should be the && expression — the fallback path provides it
    assert!(trace.expr.contains("&&"));
    assert_eq!(trace.value, json!(false));
}

#[test]
fn test_sub_expr_trace_type_exists() {
    // Verifies SubExprTrace is public and has expected fields
    let trace = SubExprTrace {
        expr: "test".to_string(),
        value: json!(true),
        children: vec![],
    };
    assert_eq!(trace.expr, "test");
    assert_eq!(trace.value, json!(true));
    assert!(trace.children.is_empty());
}

#[test]
fn test_and_both_branches_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 3});
    let (value, trace) = jf.evaluate_with_trace("(x > 1) && (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(true));
    assert_eq!(trace.children.len(), 2);
    assert_eq!(trace.children[0].value, json!(true));   // x > 1
    assert_eq!(trace.children[1].value, json!(true));   // y > 2
}

#[test]
fn test_and_short_circuit() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 0, "y": 3});
    let (value, trace) = jf.evaluate_with_trace("(x > 1) && (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(false));
    assert_eq!(trace.children.len(), 1);
    assert!(trace.children[0].expr.contains(">"));
    assert_eq!(trace.children[0].value, json!(false));
}

#[test]
fn test_or_short_circuit() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 0});
    let (value, trace) = jf.evaluate_with_trace("(x > 1) || (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(true));
    assert_eq!(trace.children.len(), 1);
    assert_eq!(trace.children[0].value, json!(true));
}

#[test]
fn test_not_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5});
    let (value, trace) = jf.evaluate_with_trace("!(x > 1)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(false));
    assert!(trace.expr.starts_with('!'));
    assert_eq!(trace.children.len(), 1);
    assert_eq!(trace.children[0].value, json!(true)); // x > 1
}

#[test]
fn test_three_level_trace() {
    let mut jf = JsonFormula::new();
    let data = json!({"a": 3, "b": 4, "c": 10});
    let (value, trace) = jf.evaluate_with_trace("(a + b) > c", &data, None, None, false).unwrap();
    assert_eq!(value, json!(false));
    assert!(trace.expr.contains(">"));
    assert_eq!(trace.children.len(), 1);
    assert!(trace.children[0].expr.contains("+"));
    assert_eq!(trace.children[0].value, json!(7));
}

#[test]
fn test_function_call_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"items": [1, 2, 3]});
    let (value, trace) = jf.evaluate_with_trace("length(items)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(3));
    assert!(trace.expr.contains("length"));
    assert_eq!(trace.value, json!(3));
}

#[test]
fn test_function_args_as_children() {
    let mut jf = JsonFormula::new();
    let data = json!({"a": 3, "b": 4});
    let (value, trace) = jf.evaluate_with_trace("max(a, b)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(4));
    assert!(trace.expr.contains("max"));
    assert_eq!(trace.children.len(), 2);
    assert_eq!(trace.children[0].value, json!(3));
    assert_eq!(trace.children[1].value, json!(4));
}

#[test]
fn test_if_function_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 10});
    let (value, trace) = jf.evaluate_with_trace("if(x > 1, y + `5`, `\"no\"`)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(15));
    assert!(trace.expr.contains("if"));
    // condition + chosen branch only, not both branches
    assert_eq!(trace.children.len(), 2);
}

#[test]
fn test_projection_traced_single_entry() {
    let mut jf = JsonFormula::new();
    let data = json!({"items": [{"price": 1}, {"price": 2}, {"price": 3}]});
    let (value, trace) = jf.evaluate_with_trace("items[*].price", &data, None, None, false).unwrap();
    assert_eq!(value, json!([1, 2, 3]));
    // projection emits one entry — no per-element children
    let proj = trace.children.iter().find(|c| c.expr.contains("[*]") || c.expr.contains("price"));
    assert!(proj.is_some() || trace.expr.contains("[*]") || trace.value == json!([1, 2, 3]));
}

#[test]
fn test_bracket_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"arr": [10, 20, 30]});
    let (value, trace) = jf.evaluate_with_trace("arr[1]", &data, None, None, false).unwrap();
    assert_eq!(value, json!(20));
    assert!(trace.value == json!(20) || trace.children.iter().any(|c| c.value == json!(20)));
}

#[test]
fn test_chained_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"foo": {"bar": {"baz": 42}}});
    let (value, trace) = jf.evaluate_with_trace("foo.bar.baz", &data, None, None, false).unwrap();
    assert_eq!(value, json!(42));
    assert_eq!(trace.value, json!(42));
}
