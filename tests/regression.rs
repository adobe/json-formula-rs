use json_formula_rs::JsonFormula;
use serde_json::json;

#[test]
fn test_evaluate_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 3});
    let result = jf.evaluate("(x > 1) && (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_evaluate_arithmetic_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"a": 3, "b": 4});
    let result = jf.evaluate("a + b", &data, None, None, false).unwrap();
    assert_eq!(result, json!(7));
}

#[test]
fn test_evaluate_function_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"items": [3, 1, 4, 1, 5]});
    let result = jf.evaluate("max(items)", &data, None, None, false).unwrap();
    assert_eq!(result, json!(5));
}

#[test]
fn test_run_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"name": "Alice"});
    let ast = jf.compile("name", &[]).unwrap();
    let result = jf.run(&ast, &data, None, None, false).unwrap();
    assert_eq!(result, json!("Alice"));
}

#[test]
fn test_search_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"foo": {"bar": 42}});
    let result = jf.search("foo.bar", &data, None, None).unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn test_evaluate_projection_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"items": [{"v": 1}, {"v": 2}, {"v": 3}]});
    let result = jf.evaluate("items[*].v", &data, None, None, false).unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn test_evaluate_error_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({});
    let result = jf.evaluate("unknownFunc()", &data, None, None, false);
    assert!(result.is_err());
}
