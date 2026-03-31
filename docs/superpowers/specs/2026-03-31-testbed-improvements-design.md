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

Remove `'Tab'` from the `onKeyDown` trigger condition. Only `Enter` fires evaluation. Tab will move browser focus naturally through the four fields in DOM order: Input JSON → Expression → Result → Debug Info.

**Before:**
```js
if (event.key === 'Enter' || event.key === 'Tab') {
```

**After:**
```js
if (event.key === 'Enter') {
```

Also remove the comment that referenced Tab's suppressed default behavior.

---

## Change 2: JSON Syntax Highlighting (CodeMirror 5)

### Vendored Files

Download into `testbed/ui/vendor/` (create directory):

| File | Source |
|------|--------|
| `codemirror.min.js` | CodeMirror 5.65.16 core |
| `codemirror.min.css` | CodeMirror 5.65.16 core CSS |
| `javascript.min.js` | CodeMirror 5.65.16 JavaScript/JSON mode |

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
```

Remove the existing `textarea:focus-visible` rule that applied to the JSON input and result fields — those are now handled by `.CodeMirror-focused`. The rule remains in effect for Expression and Debug textareas.

### main.js

**Initialization** (at top of script, after DOM references):

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
  readOnly: true,
});
```

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
```

**`evaluate` function:** Replace `.value` / `.value =` references:

| Old | New |
|-----|-----|
| `jsonInput.value.trim()` | `jsonEditor.getValue().trim()` |
| `resultArea.value = result.result ?? ''` | `resultEditor.setValue(result.result ?? '')` |
| `resultArea.style.color = ''` | *(handled by `clearError`)* |

The `debugArea.value` reference is unchanged — Debug Info remains a plain textarea.

---

## Out of Scope

- Syntax highlighting on Expression or Debug Info fields.
- CodeMirror themes (default theme matches the existing white/monospace aesthetic).
- Line numbers.
- Auto-complete or linting.
