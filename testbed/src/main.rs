#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[derive(serde::Serialize, Debug)]
struct EvaluateResult {
    result: Option<String>,
    debug: Vec<String>,
    error: Option<String>,
}

fn run_evaluate(expression: String, json: String) -> EvaluateResult {
    let parsed: serde_json::Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            return EvaluateResult {
                result: None,
                debug: vec![],
                error: Some(e.to_string()),
            }
        }
    };
    let mut jf = json_formula_rs::JsonFormula::new();
    let result = jf.evaluate(&expression, &parsed, None, None, false);
    let debug = jf.debug().to_vec(); // collected unconditionally, even on error
    match result {
        Ok(v) => EvaluateResult {
            // to_string_pretty for a bare integer (e.g. 42) produces "42" with no whitespace
            result: Some(serde_json::to_string_pretty(&v).unwrap_or_default()),
            debug,
            error: None,
        },
        Err(e) => EvaluateResult {
            result: None,
            debug,
            error: Some(e.to_string()),
        },
    }
}

// `evaluate` and `main` are gated out of test builds because tauri::generate_context!()
// reads icon assets at compile time that are not present in this testbed.
// run_evaluate (the logic under test) remains available to the test module.
#[cfg(not(test))]
#[tauri::command]
fn evaluate(expression: String, json: String) -> EvaluateResult {
    run_evaluate(expression, json)
}

#[cfg(not(test))]
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![evaluate])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_returns_error_on_invalid_json() {
        let result = run_evaluate("a".to_string(), "not json".to_string());
        assert!(result.error.is_some());
        assert!(result.result.is_none());
        assert!(result.debug.is_empty());
    }

    #[test]
    fn evaluate_returns_result_on_valid_input() {
        let result = run_evaluate("a".to_string(), r#"{"a": 42}"#.to_string());
        assert!(result.error.is_none());
        assert_eq!(result.result.as_deref(), Some("42"));
    }

    #[test]
    fn evaluate_collects_debug_messages() {
        // Accessing a missing field causes the library to emit debug messages.
        let result = run_evaluate(
            "missing_field".to_string(),
            r#"{"label": "x"}"#.to_string(),
        );
        assert!(result.error.is_none(), "missing field should not be an error");
        assert!(!result.debug.is_empty(), "expected debug messages for missing field");
    }
}
