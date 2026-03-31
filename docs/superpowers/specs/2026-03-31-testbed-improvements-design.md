# Testbed Improvements — Design Spec

**Date:** 2026-03-31
**Branch:** `feature/tauri-testbed`
**Status:** Approved

---

## Goal

Two targeted improvements to the Tauri testbed GUI:
1. Tab key moves focus between fields instead of triggering evaluation.
2. Input JSON and Result fields display JSON syntax highlighting via CodeMirror 5.

---

## Change 1: Tab Key Behavior

**File:** `testbed/ui/main.js`

Remove `'Tab'` from the `onKeyDown` trigger condition. Only `Enter` fires evaluation. Tab moves browser focus naturally through the four fields in DOM order: Input JSON → Expression → Result → Debug Info.

**Before:**
```js
// Enter and Tab both trigger evaluation. Tab's default (focus-next) is intentionally
// suppressed so the key acts as "evaluate" rather than leaving the field.
if (event.key === 'Enter' || event.key === 'Tab') {
  event.preventDefault();
```

**After:**
```js
if (event.key === 'Enter') {
```

Remove `event.preventDefault()` as well — with only `Enter` matched, the default action for Enter (newline in textarea) is still suppressed but Tab no longer needs special handling.

---

## Change 2: JSON Syntax Highlighting (CodeMirror 5)

### Vendored Files

Download into `testbed/ui/vendor/` (create directory):

| File | Download URL |
|------|-------------|
| `codemirror.min.js` | `https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/codemirror.min.js` |
| `codemirror.min.css` | `https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/codemirror.min.css` |
| `javascript.min.js` | `https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/mode/javascript/javascript.min.js` |

These files are loaded via `<script>` and `<link>` tags — no npm, no build step.

### index.html

Add `<link>` for `vendor/codemirror.min.css` in `<head>`.
Add `<script>` tags for `vendor/codemirror.min.js` and `vendor/javascript.min.js` before `main.js`.

Wrap the `#json-input` textarea and the `#result` textarea each in a `<div class="editor-wrap">`:

```html
<div class="editor-wrap">
  <textarea id="json-input" ...></textarea>
</div>
```

The Expression (`#expression-input`) and Debug Info (`#debug`) fields remain plain textareas — no change.

### style.css

Add rules for the CodeMirror editors:

```css
/* Editor wrapper fills the flex column */
.editor-wrap {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* CodeMirror fills the wrapper */
.editor-wrap .CodeMirror {
  height: 100%;
  font-family: monospace;
  font-size: 0.85rem;
  border: 1px solid #ccc;
}

/* Focus border matches existing textarea:focus-visible style */
.editor-wrap .CodeMirror-focused {
  border-color: #0078d4;
  box-shadow: 0 0 0 1px #0078d4;
}

/* Error state: red text on result editor */
.editor-wrap.error .CodeMirror {
  color: red;
}

/* Hide the original textarea that CodeMirror wraps — CM sets display:none
   but being explicit also prevents the textarea { flex: 1 } rule from
   contributing to layout calculations inside .editor-wrap. */
.editor-wrap textarea {
  display: none;
}
```

The existing `textarea:focus-visible` rule stays in place unchanged — after CodeMirror wraps the two textareas, that rule will only apply to the Expression and Debug fields (the remaining visible textareas), which is the correct behaviour.

### main.js

**Initialization** runs synchronously at top-level script load, after the DOM reference declarations and before any event handlers fire. This guarantees `jsonEditor` and `resultEditor` are always defined when `showError` or `evaluate` is called.

```js
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
```

`readOnly: 'nocursor'` prevents CodeMirror from placing a text cursor in the result panel, which is appropriate for a display-only field.

**`showError` / `clearError`:** Toggle `.error` on the result editor's wrapper element and switch mode between plain text (for error messages) and JSON (for results):

```js
function showError(message) {
  resultEditor.getWrapperElement().classList.add('error');
  resultEditor.setOption('mode', 'text/plain');
  resultEditor.setValue(message);
}

function clearError() {
  resultEditor.getWrapperElement().classList.remove('error');
  resultEditor.setOption('mode', { name: 'javascript', json: true });
}
// clearError intentionally does not call setValue('') — the evaluate call site
// immediately calls resultEditor.setValue(result.result ?? '') afterward.
```

**`evaluate` function:** Replace all textarea `.value` reads/writes:

| Location | Old | New |
|----------|-----|-----|
| Read JSON input | `jsonInput.value.trim()` | `jsonEditor.getValue().trim()` |
| Clear result (empty-JSON guard) | `resultArea.value = ''` | `resultEditor.setValue('')` |
| Set result on success | `resultArea.value = result.result ?? ''` | `resultEditor.setValue(result.result ?? '')` |

`debugArea.value` is unchanged — Debug Info remains a plain textarea.

**`keydown` listener migration:** After `fromTextArea`, the original `jsonInput` element is hidden by CodeMirror and no longer receives focus events. Migrate the keydown listener from the textarea to the editor instance:

```js
// Before:
jsonInput.addEventListener('keydown', onKeyDown);

// After:
jsonEditor.on('keydown', (_cm, event) => onKeyDown(event));
expressionInput.addEventListener('keydown', onKeyDown);  // unchanged
```

---

## Out of Scope

- Syntax highlighting on Expression or Debug Info fields.
- CodeMirror themes (default theme matches the existing white/monospace aesthetic).
- Line numbers.
- Auto-complete or linting.
