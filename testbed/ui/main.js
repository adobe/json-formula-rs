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
