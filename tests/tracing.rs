use json_formula_rs::{JsonFormula, SubExprTrace};
use serde_json::json;

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
