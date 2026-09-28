// Regression tests for the `$args` global binding that register_expression(),
// register_expression_with_params(), and the built-in register()/
// registerWithParams() functions now provide inside the body of a registered
// function, in addition to `@`.
//
// Bug context: some existing rubrics (see usage-issue/readme.txt) author their
// custom functions using `$args[0]...` instead of `@[0]...`/`@...`. Before this
// fix `$args` was always Null (it isn't a `variables:` global and wasn't bound
// by the registration APIs), so every `$args`-based expression silently
// evaluated to Null, causing downstream index/type errors.

use json_formula_rs::JsonFormula;
use serde_json::json;

#[test]
fn test_register_expression_binds_args_global() {
    let mut jf = JsonFormula::new();
    // register_expression() takes 0 or 1 argument; the single argument should
    // be reachable both via `@` and via `$args[0]`.
    jf.register_expression("_viaArgs", "$args[0].name").unwrap();
    let data = json!({"person": {"name": "Ada"}});
    let result = jf
        .evaluate("_viaArgs(person)", &data, None, None, false)
        .unwrap();
    assert_eq!(result, json!("Ada"));
}

#[test]
fn test_register_expression_with_params_binds_args_global() {
    let mut jf = JsonFormula::new();
    // This mirrors the rubric pattern from usage-issue/asset-rubric-conformance0.2-spec2.4.yml:
    //   _manifest_createdAssertions: $args[0].'claim.v2'.created_assertions || `[]`
    jf.register_expression_with_params(
        "_manifest_createdAssertions",
        "$args[0].'claim.v2'.created_assertions || `[]`",
    )
    .unwrap();
    let data = json!({
        "manifests": [{
            "claim.v2": { "created_assertions": [{"url": "self#jumbf=c2pa.assertions/c2pa.actions.v2"}] }
        }]
    });
    let result = jf
        .evaluate(
            "_manifest_createdAssertions(manifests[0])",
            &data,
            None,
            None,
            false,
        )
        .unwrap();
    assert_eq!(
        result,
        json!([{"url": "self#jumbf=c2pa.assertions/c2pa.actions.v2"}])
    );
}

#[test]
fn test_register_expression_with_params_args_matches_at_sign_array() {
    // `$args` should be exactly the same array that `@` is bound to when called
    // with one or more arguments.
    let mut jf = JsonFormula::new();
    jf.register_expression_with_params("_bothMatch", "@ == $args")
        .unwrap();
    let data = json!({});
    let result = jf
        .evaluate("_bothMatch(1, 2, 3)", &data, None, None, false)
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_register_expression_with_params_args_empty_call() {
    // With zero arguments, `$args` is `[]` even though `@` falls back to the
    // outer evaluation context (data).
    let mut jf = JsonFormula::new();
    jf.register_expression_with_params("_argsLen", "length($args)")
        .unwrap();
    let data = json!({"x": 1});
    let result = jf.evaluate("_argsLen()", &data, None, None, false).unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn test_builtin_register_binds_args_global() {
    let mut jf = JsonFormula::new();
    let data = json!({});
    jf.evaluate(
        r#"register("_ViaArgs", &$args[0] * 2)"#,
        &data,
        None,
        None,
        false,
    )
    .unwrap();
    let result = jf
        .evaluate("_ViaArgs(21)", &data, None, None, false)
        .unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn test_builtin_register_with_params_binds_args_global() {
    let mut jf = JsonFormula::new();
    let data = json!({});
    jf.evaluate(
        r#"registerWithParams("_ProductArgs", &$args[0] * $args[1])"#,
        &data,
        None,
        None,
        false,
    )
    .unwrap();
    let result = jf
        .evaluate("_ProductArgs(2, 21)", &data, None, None, false)
        .unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn test_args_global_restored_after_nested_registered_calls() {
    // Nested registered-function calls must each see their own $args, and the
    // outer call's $args must be restored after the inner call returns.
    let mut jf = JsonFormula::new();
    jf.register_expression_with_params("_inner", "$args[0] + 1")
        .unwrap();
    jf.register_expression_with_params("_outer", "_inner($args[0]) + $args[0]")
        .unwrap();
    let data = json!({});
    let result = jf.evaluate("_outer(10)", &data, None, None, false).unwrap();
    // _inner(10) => 11, plus outer's own $args[0] (10) => 21
    assert_eq!(result, json!(21));
}

#[test]
fn test_user_globals_still_take_precedence_and_survive() {
    // Existing `$`-prefixed user globals (from `variables:`) must remain
    // reachable from inside a registered function body, alongside the new
    // `$args` binding.
    let mut jf = JsonFormula::new();
    jf.register_expression_with_params("_withGlobal", "$args[0] == $myGlobal")
        .unwrap();
    let data = json!({});
    let globals = json!({"$myGlobal": "hello"});
    let result = jf
        .evaluate("_withGlobal(\"hello\")", &data, Some(&globals), None, false)
        .unwrap();
    assert_eq!(result, json!(true));
}
