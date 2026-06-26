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
    // The projection result appears somewhere in the tree — either as the root or a child.
    // Crucially, no per-element entries (3 items would give 3 children if per-element were traced).
    let proj_trace = if trace.value == json!([1, 2, 3]) {
        &trace
    } else {
        trace.children.iter().find(|c| c.value == json!([1, 2, 3]))
            .expect("projection result [1,2,3] should appear in trace")
    };
    // No per-element children — only one entry for the whole projection result
    assert!(
        proj_trace.children.len() != 3,
        "projection should not emit one child per element"
    );
}

#[test]
fn test_bracket_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"arr": [10, 20, 30]});
    let (value, trace) = jf.evaluate_with_trace("arr[1]", &data, None, None, false).unwrap();
    assert_eq!(value, json!(20));
    // The bracket result should appear in the trace tree
    let found = trace.value == json!(20)
        || trace.children.iter().any(|c| c.value == json!(20));
    assert!(found, "bracket result 20 should appear in trace");
}

#[test]
fn test_chained_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"foo": {"bar": {"baz": 42}}});
    let (value, trace) = jf.evaluate_with_trace("foo.bar.baz", &data, None, None, false).unwrap();
    assert_eq!(value, json!(42));
    assert_eq!(trace.value, json!(42));
}

// --- Negative / error-path tests ---

#[test]
fn test_evaluate_with_trace_syntax_error() {
    let mut jf = JsonFormula::new();
    let data = json!({});
    let result = jf.evaluate_with_trace("(((", &data, None, None, false);
    assert!(result.is_err(), "syntax error should propagate through evaluate_with_trace");
}

#[test]
fn test_evaluate_with_trace_unknown_function() {
    let mut jf = JsonFormula::new();
    let data = json!({});
    let result = jf.evaluate_with_trace("noSuchFunction()", &data, None, None, false);
    assert!(result.is_err(), "unknown function should propagate through evaluate_with_trace");
}

#[test]
fn test_evaluate_with_trace_type_error() {
    let mut jf = JsonFormula::new();
    // unary minus on a string is a TypeError
    let data = json!({"s": "hello"});
    let result = jf.evaluate_with_trace("-s", &data, None, None, false);
    assert!(result.is_err(), "type error should propagate through evaluate_with_trace");
}

#[test]
fn test_evaluate_still_works_after_trace_error() {
    // Ensures the interpreter is re-created per call, so a failed trace call
    // doesn't corrupt subsequent non-tracing calls.
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5});
    let _ = jf.evaluate_with_trace("noSuchFunction()", &data, None, None, false);
    // subsequent evaluate must still work correctly
    let result = jf.evaluate("x > 1", &data, None, None, false).unwrap();
    assert_eq!(result, json!(true));
}
