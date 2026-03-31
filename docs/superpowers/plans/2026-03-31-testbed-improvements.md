# Testbed Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Tab key behaviour (evaluate + advance focus) and add CodeMirror 5 JSON syntax highlighting to the Input JSON and Result fields.

**Architecture:** Two independent changes to `testbed/ui/`: (1) a one-line `onKeyDown` edit, and (2) three vendored CodeMirror files plus coordinated edits to `index.html`, `style.css`, and `main.js`. No Rust changes, no build step, no npm.

**Tech Stack:** Vanilla JS, CodeMirror 5.65.16 (vendored), existing Tauri v1 testbed

---

## File Map

| File | Change |
|------|--------|
| `testbed/ui/vendor/codemirror.min.js` | Create — CodeMirror 5 core (downloaded) |
| `testbed/ui/vendor/codemirror.min.css` | Create — CodeMirror 5 core CSS (downloaded) |
| `testbed/ui/vendor/javascript.min.js` | Create — CodeMirror JSON mode (downloaded) |
| `testbed/ui/index.html` | Modify — add vendor `<link>`/`<script>` tags, wrap two textareas in `.editor-wrap` |
| `testbed/ui/style.css` | Modify — add `.editor-wrap`, `.CodeMirror`, `.CodeMirror-focused`, `.error` rules |
| `testbed/ui/main.js` | Modify — fix `onKeyDown`, init CodeMirror editors, migrate all textarea API calls |

---

### Task 1: Fix Tab key behaviour

**Files:**
- Modify: `testbed/ui/main.js:53-59`

The current `onKeyDown` calls `event.preventDefault()` for both Enter and Tab, which suppresses Tab's default focus-advance behaviour. The fix: only call `event.preventDefault()` for Enter; Tab fires evaluation but lets the browser advance focus normally.

- [ ] **Step 1: Edit `onKeyDown` in `testbed/ui/main.js`**

Replace lines 53–60 (the entire `onKeyDown` function):

```js
function onKeyDown(event) {
  if (event.key === 'Enter') {
    event.preventDefault();
    evaluate();
  } else if (event.key === 'Tab') {
    evaluate(); // fires evaluation; focus-next is NOT suppressed
  }
}
```

- [ ] **Step 2: Verify the edit looks correct**

```bash
grep -A 6 "function onKeyDown" testbed/ui/main.js
```

Expected output: the new function body with the split if/else-if — no `preventDefault` on the Tab branch.

- [ ] **Step 3: Commit**

```bash
git add testbed/ui/main.js
git commit -m "fix: Tab fires evaluation and advances focus (no preventDefault)"
```

---

### Task 2: Download CodeMirror vendor files

**Files:**
- Create: `testbed/ui/vendor/codemirror.min.js`
- Create: `testbed/ui/vendor/codemirror.min.css`
- Create: `testbed/ui/vendor/javascript.min.js`

- [ ] **Step 1: Create the vendor directory**

```bash
mkdir -p testbed/ui/vendor
```

- [ ] **Step 2: Download the three files**

```bash
curl -L -o testbed/ui/vendor/codemirror.min.js \
  https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/codemirror.min.js

curl -L -o testbed/ui/vendor/codemirror.min.css \
  https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/codemirror.min.css

curl -L -o testbed/ui/vendor/javascript.min.js \
  https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/mode/javascript/javascript.min.js
```

- [ ] **Step 3: Verify files are non-empty**

```bash
wc -c testbed/ui/vendor/codemirror.min.js \
       testbed/ui/vendor/codemirror.min.css \
       testbed/ui/vendor/javascript.min.js
```

Expected: all three files are > 1000 bytes each. If any is tiny (e.g. an HTML error page), the download failed — check your internet connection and retry.

- [ ] **Step 4: Commit**

```bash
git add testbed/ui/vendor/
git commit -m "chore: vendor CodeMirror 5.65.16 (core + JS/JSON mode)"
```

---

### Task 3: Update index.html

**Files:**
- Modify: `testbed/ui/index.html`

Add vendor asset tags in `<head>` and wrap the two syntax-highlighted textareas in `.editor-wrap` divs. Expression and Debug fields are unchanged.

- [ ] **Step 1: Write the updated file**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>json-formula Testing</title>
  <link rel="stylesheet" href="vendor/codemirror.min.css" />
  <link rel="stylesheet" href="style.css" />
</head>
<body>
  <h1>json-formula Testing</h1>
  <div class="columns">
    <div class="column">
      <label for="json-input">Input JSON</label>
      <div class="editor-wrap">
        <textarea id="json-input" spellcheck="false" placeholder='{ "key": "value" }'></textarea>
      </div>
    </div>
    <div class="column">
      <label for="expression-input">Expression</label>
      <textarea id="expression-input" spellcheck="false" placeholder="key"></textarea>
    </div>
    <div class="column">
      <label for="result">Result</label>
      <div class="editor-wrap">
        <textarea id="result" readonly></textarea>
      </div>
    </div>
    <div class="column">
      <label for="debug">Debug Info</label>
      <textarea id="debug" readonly></textarea>
    </div>
  </div>
  <script src="vendor/codemirror.min.js"></script>
  <script src="vendor/javascript.min.js"></script>
  <script src="main.js"></script>
</body>
</html>
```

Key points:
- `vendor/codemirror.min.css` loads **before** `style.css` so our rules can override CM defaults
- The two vendor JS files load **before** `main.js` so `CodeMirror` is defined when `main.js` runs
- Only `#json-input` and `#result` are wrapped; `#expression-input` and `#debug` are not

- [ ] **Step 2: Verify structure**

```bash
grep -n "editor-wrap\|vendor\|script" testbed/ui/index.html
```

Expected: two `editor-wrap` divs (around json-input and result), three script/link tags referencing `vendor/`.

- [ ] **Step 3: Commit**

```bash
git add testbed/ui/index.html
git commit -m "feat: add CodeMirror vendor assets and editor-wrap divs to index.html"
```

---

### Task 4: Update style.css

**Files:**
- Modify: `testbed/ui/style.css`

Append CodeMirror layout rules to the end of the existing file. No existing rules need to change — the `.editor-wrap textarea { display: none }` rule neutralises the `textarea { flex: 1 }` rule for the two wrapped textareas, and `textarea:focus-visible` stays in place for Expression and Debug.

- [ ] **Step 1: Append the new rules**

Add the following block to the **end** of `testbed/ui/style.css`:

```css
/* ── CodeMirror editors ─────────────────────────────────────── */

/* Wrapper takes the flex slot the textarea used to occupy */
.editor-wrap {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* Prevent the hidden CM textarea from participating in flex layout */
.editor-wrap textarea {
  display: none;
}

/* CM editor fills the wrapper */
.editor-wrap .CodeMirror {
  height: 100%;
  font-family: monospace;
  font-size: 0.85rem;
  border: 1px solid #ccc;
}

/* Focus ring matches existing textarea:focus-visible style */
.editor-wrap .CodeMirror-focused {
  border-color: #0078d4;
  box-shadow: 0 0 0 1px #0078d4;
}

/* Error state: red text on the result editor */
.editor-wrap.error .CodeMirror {
  color: red;
}
```

- [ ] **Step 2: Verify the file ends correctly**

```bash
tail -10 testbed/ui/style.css
```

Expected: the last lines are the `.editor-wrap.error .CodeMirror` rule with a closing `}`.

- [ ] **Step 3: Commit**

```bash
git add testbed/ui/style.css
git commit -m "feat: add CodeMirror layout and focus styles"
```

---

### Task 5: Update main.js — CodeMirror integration

**Files:**
- Modify: `testbed/ui/main.js`

This is the largest change. Rewrite the entire file with:
1. CodeMirror editors initialised synchronously after DOM references
2. `showError`/`clearError` using the editor API
3. All three `resultArea.value` / `jsonInput.value` references replaced with editor calls
4. `jsonInput.addEventListener` replaced with `jsonEditor.on`

- [ ] **Step 1: Write the complete updated file**

```js
const jsonInput = document.getElementById('json-input');
const expressionInput = document.getElementById('expression-input');
const resultArea = document.getElementById('result');
const debugArea = document.getElementById('debug');

// Initialise CodeMirror editors synchronously so showError/evaluate can always
// reference jsonEditor and resultEditor safely.
const jsonEditor = CodeMirror.fromTextArea(jsonInput, {
  mode: { name: 'javascript', json: true },
  lineNumbers: false,
  lineWrapping: true,
});

const resultEditor = CodeMirror.fromTextArea(resultArea, {
  mode: { name: 'javascript', json: true },
  lineNumbers: false,
  lineWrapping: true,
  readOnly: 'nocursor',
});

function showError(message) {
  resultEditor.getWrapperElement().classList.add('error');
  resultEditor.setOption('mode', 'text/plain');
  resultEditor.setValue(message);
}

function clearError() {
  resultEditor.getWrapperElement().classList.remove('error');
  resultEditor.setOption('mode', { name: 'javascript', json: true });
  // Does NOT call setValue('') — the evaluate call site sets the result immediately after.
}

async function evaluate() {
  const json = jsonEditor.getValue().trim();
  const expression = expressionInput.value.trim();

  if (!json) {
    clearError();
    resultEditor.setValue('');
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

  if (!window.__TAURI__) {
    showError('Not running inside Tauri — window.__TAURI__ is unavailable');
    return;
  }

  try {
    const result = await window.__TAURI__.invoke('evaluate', { expression, json });
    debugArea.value = result.debug.join('\n');
    if (result.error) {
      showError(result.error);
    } else {
      clearError();
      resultEditor.setValue(result.result ?? '');
    }
  } catch (e) {
    showError('IPC error: ' + e.message);
  }
}

function onKeyDown(event) {
  if (event.key === 'Enter') {
    event.preventDefault();
    evaluate();
  } else if (event.key === 'Tab') {
    evaluate(); // fires evaluation; focus-next is NOT suppressed
  }
}

// jsonInput is hidden by CodeMirror — attach the listener to the editor instance.
jsonEditor.on('keydown', (_cm, event) => onKeyDown(event));
expressionInput.addEventListener('keydown', onKeyDown);
```

- [ ] **Step 2: Verify no raw `resultArea.value` or `jsonInput.value` references remain**

```bash
grep -n "resultArea\.value\|jsonInput\.value" testbed/ui/main.js
```

Expected: no output (zero matches).

- [ ] **Step 3: Verify `jsonInput.addEventListener` is gone**

```bash
grep -n "jsonInput\.addEventListener" testbed/ui/main.js
```

Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add testbed/ui/main.js
git commit -m "feat: integrate CodeMirror editors, fix keydown listeners and API calls"
```

---

### Task 6: Smoke test

**Files:** none new

- [ ] **Step 1: Build**

```bash
cd testbed && cargo build 2>&1 | tail -5
```

Expected: `Finished` with no errors. (The Rust code is unchanged so this should be fast.)

- [ ] **Step 2: Run**

```bash
cd testbed && cargo run
```

Expected: window opens. Both the Input JSON and Result columns show CodeMirror editors (monospace font, slightly different background from a plain textarea). Expression and Debug remain plain textareas.

- [ ] **Step 3: Smoke test — syntax highlighting**

1. Paste `{"a": 42, "b": "hello"}` into Input JSON — keys should appear in one colour, string values in another, numbers in another.
2. Type `a` in Expression, press Enter — Result shows `42` with JSON syntax colouring.
3. Type `keys(@)` in Expression, press Enter — Result shows `["a","b"]` with array syntax colouring.

- [ ] **Step 4: Smoke test — Tab behaviour**

4. Click into Input JSON, press Tab — `evaluate()` fires (Result updates) AND focus moves to Expression.
5. Press Tab again from Expression — `evaluate()` fires AND focus moves to Result.
6. Press Tab again — focus moves to Debug Info (readonly CM editor has `nocursor` so focus skips it naturally, or lands on it — either is fine for a dev tool).

- [ ] **Step 5: Smoke test — error state**

7. Type `{bad` into Input JSON, press Enter — Result shows red error text (not syntax-coloured).
8. Fix the JSON back to `{"a": 42}`, press Enter — Result returns to normal JSON colouring.

- [ ] **Step 6: Final commit**

```bash
git add .
git commit -m "feat: testbed improvements complete (Tab behaviour + CodeMirror highlighting)"
```
