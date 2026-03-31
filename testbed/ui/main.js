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
