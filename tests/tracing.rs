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
