# Tauri Testbed — Design Spec

**Date:** 2026-03-31
**Branch:** `feature/tauri-testbed`
**Status:** Approved

---

## Goal

Build a desktop GUI testbed for the `json-formula-rs` library. The app lets a developer type a JSON document and a json-formula expression, then see the result and debug output in real time.

---

## Repository Structure

The testbed lives in a `testbed/` subdirectory of this repo. The root `Cargo.toml` is not a workspace, so `testbed/Cargo.toml` stands alone and needs no changes to the root.

```
json-formula-rs/
├── src/                        existing library
├── Cargo.toml                  existing, unchanged
└── testbed/
    ├── Cargo.toml              Tauri v1 app; depends on json-formula-rs via path = ".."
    ├── src/
    │   └── main.rs             Tauri entry point + one #[tauri::command]
    └── ui/
        ├── index.html
        ├── style.css
        └── main.js
```

---

## Tauri Version

This app targets **Tauri v1**. The `tauri.conf.json` sets `"withGlobalTauri": true`, which injects `window.__TAURI__` into every page. The frontend calls `window.__TAURI__.invoke(...)` — no npm, no build step, no ES module import.

---

## Backend

`main.rs` exposes one Tauri command:

```rust
#[tauri::command]
fn evaluate(expression: String, json: String) -> EvaluateResult
```

`EvaluateResult` is a serializable struct:

```rust
#[derive(serde::Serialize)]
struct EvaluateResult {
    result: Option<String>,
    debug: Vec<String>,
    error: Option<String>,
}
```

`result` is `None` when `error` is `Some`; it is `Some(pretty-printed JSON string)` on success.

The command body:

```rust
let parsed: serde_json::Value = match serde_json::from_str(&json) {
    Ok(v) => v,
    Err(e) => return EvaluateResult { result: None, debug: vec![], error: Some(e.to_string()) },
};
let mut jf = json_formula_rs::JsonFormula::new();
let result = jf.evaluate(&expression, &parsed, None, None, false);
let debug = jf.debug().to_vec();   // always collected, even on error
match result {
    Ok(v) => EvaluateResult {
        result: Some(serde_json::to_string_pretty(&v).unwrap_or_default()),
        debug,
        error: None,
    },
    Err(e) => EvaluateResult { result: None, debug, error: Some(e.to_string()) },
}
```

Debug messages are collected unconditionally after `evaluate` returns, so messages from a failed expression parse are still surfaced in the UI.

---

## Frontend

**Technology:** Vanilla HTML, CSS, and JavaScript. No build step. No npm. `invoke` is accessed via `window.__TAURI__.invoke`.

### Layout

Four equal-width columns in a horizontal flex row that fills the viewport. A centered title sits above the columns.

```
json-formula Testing
┌──────────────┬──────────────┬──────────┬──────────────┐
│  Input JSON  │  Expression  │  Result  │  Debug Info  │
│              │              │          │              │
│  <textarea>  │  <textarea>  │<textarea>│  <textarea>  │
│              │              │(readonly)│  (readonly)  │
└──────────────┴──────────────┴──────────┴──────────────┘
```

### Styling

- White background, black text, monospace font throughout.
- Column headers bold, centered.
- Textareas fill remaining height; readonly columns have no resize handle.
- Standard browser `:focus` outline (blue) marks the active textarea — no special-casing.
- When `error` is set, the Result textarea text renders red.

### Evaluation Trigger

Evaluation fires when the user presses `Enter` or `Tab` in **either** the Expression or Input JSON textarea. The handler calls `event.preventDefault()` to suppress the default Tab behavior.

```js
async function evaluate() {
    const json = jsonInput.value;
    const expression = expressionInput.value;

    // Client-side parse catches obvious errors without an IPC round-trip.
    // The backend also validates; this is a latency optimisation, not the only guard.
    try { JSON.parse(json); } catch (e) {
        showError(e.message);
        debugArea.value = '';
        return;
    }

    const result = await window.__TAURI__.invoke('evaluate', { expression, json });
    if (result.error) {
        showError(result.error);
    } else {
        resultArea.style.color = '';
        resultArea.value = result.result ?? '';
    }
    debugArea.value = result.debug.join('\n');
}
```

On a client-side JSON parse failure the debug area is cleared to avoid showing stale output from a previous call.
```

The frontend parse is a latency optimisation — it skips the IPC call for obvious JSON errors. The backend always re-parses as a safety net.

---

## Out of Scope

- Globals / language parameters (not exposed in the UI).
- Saving or loading sessions.
- Syntax highlighting.
- The `fields_only` flag.
