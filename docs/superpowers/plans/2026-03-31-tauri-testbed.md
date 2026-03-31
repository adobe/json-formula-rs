# Tauri Testbed Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri v1 desktop GUI in `testbed/` that lets developers interactively test json-formula-rs expressions against JSON input.

**Architecture:** A standalone `testbed/` Rust crate depends on `json-formula-rs` via path dependency. A single `#[tauri::command]` wraps the library's `evaluate` API and returns result + debug info as a serializable struct. The frontend is plain HTML/CSS/JS in `testbed/ui/` — no npm, no build step.

**Tech Stack:** Rust, Tauri v1, Vanilla HTML/CSS/JS, serde_json, json-formula-rs (path dep)

---

## File Map

| File | Purpose |
|------|---------|
| `testbed/Cargo.toml` | Tauri app crate; depends on json-formula-rs, tauri v1, serde, serde_json |
| `testbed/build.rs` | Calls `tauri_build::build()` — required by Tauri v1 |
| `testbed/tauri.conf.json` | Window title/size, `distDir`=`ui`, `withGlobalTauri`=true |
| `testbed/src/main.rs` | `EvaluateResult` struct + `run_evaluate` fn + `evaluate` command + `main()` |
| `testbed/ui/index.html` | Four-column layout: title + four labeled textareas |
| `testbed/ui/style.css` | Monospace font, flex layout, `:focus` border, `.error` red color |
| `testbed/ui/main.js` | Keydown handler, `invoke` call, DOM updates |

---

### Task 1: Create directory structure

**Files:**
- Create: `testbed/src/` (directory)
- Create: `testbed/ui/` (directory)

- [ ] **Step 1: Create directories**

Run from the repo root (`json-formula-rs/`):
```bash
mkdir -p testbed/src testbed/ui
```

- [ ] **Step 2: Verify**

```bash
ls testbed/
```

Expected output includes `src/` and `ui/`.

---

### Task 2: Write testbed/Cargo.toml

**Files:**
- Create: `testbed/Cargo.toml`

- [ ] **Step 1: Write the file**

```toml
[package]
name = "json-formula-testbed"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "1", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
json-formula-rs = { path = ".." }

[build-dependencies]
tauri-build = { version = "1", features = [] }
```

- [ ] **Step 2: Verify TOML parses cleanly**

```bash
cd testbed && cargo metadata --no-deps 2>&1 | head -5
```

Expected: JSON output with the package name `json-formula-testbed`. (Compile errors about missing files are fine here.)

---

### Task 3: Write testbed/build.rs

**Files:**
- Create: `testbed/build.rs`

- [ ] **Step 1: Write the file**

```rust
fn main() {
    tauri_build::build()
}
```

---

### Task 4: Write testbed/tauri.conf.json

**Files:**
- Create: `testbed/tauri.conf.json`

- [ ] **Step 1: Write the file**

```json
{
  "build": {
    "beforeDevCommand": "",
    "beforeBuildCommand": "",
    "devPath": "ui",
    "distDir": "ui",
    "withGlobalTauri": true
  },
  "package": {
    "productName": "json-formula-testbed",
    "version": "0.1.0"
  },
  "tauri": {
    "allowlist": {
      "all": false
    },
    "bundle": {
      "active": false,
      "identifier": "com.adobe.json-formula-testbed",
      "icon": []
    },
    "security": {
      "csp": null
    },
    "updater": {
      "active": false
    },
    "windows": [
      {
        "fullscreen": false,
        "resizable": true,
        "title": "json-formula Testing",
        "width": 1400,
        "height": 900
      }
    ]
  }
}
```

---

### Task 5: Write testbed/src/main.rs

**Files:**
- Create: `testbed/src/main.rs`

- [ ] **Step 1: Write the failing tests first**

Create `testbed/src/main.rs` with only the test module (no implementation yet):

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
        // Accessing a missing field generates debug messages in the library.
        let result = run_evaluate(
            "missing_field".to_string(),
            r#"{"label": "x"}"#.to_string(),
        );
        assert!(!result.debug.is_empty(), "expected debug messages for missing field");
    }
}
```

- [ ] **Step 2: Run to confirm tests fail (function not found)**

```bash
cd testbed && cargo test 2>&1 | head -20
```

Expected: compile error — `run_evaluate` not defined.

- [ ] **Step 3: Add the full implementation above the test module**

Prepend this to `testbed/src/main.rs` (before `#[cfg(test)]`):

```rust
#[derive(serde::Serialize)]
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
    let debug = jf.debug().to_vec(); // collected unconditionally
    match result {
        Ok(v) => EvaluateResult {
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

#[tauri::command]
fn evaluate(expression: String, json: String) -> EvaluateResult {
    run_evaluate(expression, json)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![evaluate])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Run the tests**

```bash
cd testbed && cargo test 2>&1
```

Expected: all three tests pass:
```
test tests::evaluate_collects_debug_messages ... ok
test tests::evaluate_returns_error_on_invalid_json ... ok
test tests::evaluate_returns_result_on_valid_input ... ok

test result: ok. 3 passed; 0 failed
```

- [ ] **Step 5: Commit**

```bash
git add testbed/Cargo.toml testbed/build.rs testbed/tauri.conf.json testbed/src/main.rs
git commit -m "feat: add Tauri testbed scaffold and backend evaluate command"
```

---

### Task 6: Write testbed/ui/index.html

**Files:**
- Create: `testbed/ui/index.html`

- [ ] **Step 1: Write the file**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>json-formula Testing</title>
  <link rel="stylesheet" href="style.css" />
</head>
<body>
  <h1>json-formula Testing</h1>
  <div class="columns">
    <div class="column">
      <label for="json-input">Input JSON</label>
      <textarea id="json-input" spellcheck="false" placeholder='{ "key": "value" }'></textarea>
    </div>
    <div class="column">
      <label for="expression-input">Expression</label>
      <textarea id="expression-input" spellcheck="false" placeholder="key"></textarea>
    </div>
    <div class="column">
      <label for="result">Result</label>
      <textarea id="result" readonly></textarea>
    </div>
    <div class="column">
      <label for="debug">Debug Info</label>
      <textarea id="debug" readonly></textarea>
    </div>
  </div>
  <script src="main.js"></script>
</body>
</html>
```

---

### Task 7: Write testbed/ui/style.css

**Files:**
- Create: `testbed/ui/style.css`

- [ ] **Step 1: Write the file**

```css
*, *::before, *::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html, body {
  height: 100%;
  font-family: monospace;
  background: #fff;
  color: #000;
}

body {
  display: flex;
  flex-direction: column;
  padding: 8px;
  gap: 8px;
}

h1 {
  text-align: center;
  font-size: 1rem;
  font-weight: bold;
}

.columns {
  display: flex;
  flex: 1;
  gap: 8px;
  min-height: 0;
}

.column {
  display: flex;
  flex-direction: column;
  flex: 1;
  gap: 4px;
  min-width: 0;
}

label {
  font-weight: bold;
  text-align: center;
  font-size: 0.9rem;
}

textarea {
  flex: 1;
  font-family: monospace;
  font-size: 0.85rem;
  border: 1px solid #ccc;
  padding: 6px;
  resize: vertical;
  outline: none;
}

textarea:focus {
  border-color: #0078d4;
  box-shadow: 0 0 0 1px #0078d4;
}

textarea[readonly] {
  background: #fafafa;
  resize: none;
}

textarea.error {
  color: red;
}
```

---

### Task 8: Write testbed/ui/main.js

**Files:**
- Create: `testbed/ui/main.js`

- [ ] **Step 1: Write the file**

```js
const jsonInput = document.getElementById('json-input');
const expressionInput = document.getElementById('expression-input');
const resultArea = document.getElementById('result');
const debugArea = document.getElementById('debug');

function showError(message) {
  resultArea.classList.add('error');
  resultArea.value = message;
}

function clearError() {
  resultArea.classList.remove('error');
}

async function evaluate() {
  const json = jsonInput.value.trim();
  const expression = expressionInput.value.trim();

  if (!json) {
    clearError();
    resultArea.value = '';
    debugArea.value = '';
    return;
  }

  try {
    JSON.parse(json);
  } catch (e) {
    showError('JSON parse error: ' + e.message);
    debugArea.value = '';
    return;
  }

  try {
    const result = await window.__TAURI__.invoke('evaluate', { expression, json });
    debugArea.value = result.debug.join('\n');
    if (result.error) {
      showError(result.error);
    } else {
      clearError();
      resultArea.value = result.result ?? '';
    }
  } catch (e) {
    showError('IPC error: ' + e.message);
  }
}

function onKeyDown(event) {
  if (event.key === 'Enter' || event.key === 'Tab') {
    event.preventDefault();
    evaluate();
  }
}

jsonInput.addEventListener('keydown', onKeyDown);
expressionInput.addEventListener('keydown', onKeyDown);
```

- [ ] **Step 2: Commit**

```bash
git add testbed/ui/
git commit -m "feat: add Tauri testbed frontend (HTML/CSS/JS)"
```

---

### Task 9: Build and smoke test

**Files:** none new

- [ ] **Step 1: Build in debug mode**

```bash
cd testbed && cargo build 2>&1
```

Expected: build succeeds and produces `../target/debug/json-formula-testbed`.

If you see linker errors on macOS, run `xcode-select --install` and retry.

- [ ] **Step 2: Run the app**

```bash
cd testbed && cargo run
```

Expected: a window titled "json-formula Testing" opens with four equal-width columns.

- [ ] **Step 3: Manual smoke test — happy path**

1. Paste `{"a": 42}` in **Input JSON**, type `a` in **Expression**, press Enter.
   - Result shows `42`, Debug is empty.
2. Change expression to `a + 1`, press Enter.
   - Result shows `43`.
3. Change expression to `keys(@)`, press Enter.
   - Result shows `["a"]`.

- [ ] **Step 4: Manual smoke test — error paths**

4. Replace Input JSON with `{bad json`, press Tab.
   - Result shows red "JSON parse error: ..." message, Debug clears.
5. Restore valid JSON `{"label": "x"}`, type `'c2pa.actions.v2'.action` in Expression, press Enter.
   - Result is empty or error; Debug shows "Failed to find: 'c2pa.actions.v2'" messages.
6. Clear Expression and press Enter.
   - Result clears (empty expression against `{"label":"x"}` returns the whole object or null).

- [ ] **Step 5: Manual smoke test — focus behavior**

7. Click into **Input JSON** textarea, press Enter — evaluation fires from that pane.
8. Click into **Expression** textarea, press Tab — evaluation fires, Tab character is not inserted.

- [ ] **Step 6: Final commit**

```bash
git add .
git commit -m "feat: Tauri testbed complete and smoke tested"
```
