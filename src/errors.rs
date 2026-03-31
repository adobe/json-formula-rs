/*
Copyright 2025 Adobe. All rights reserved.
This file is licensed to you under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License. You may obtain a copy
of the License at http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed under
the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR REPRESENTATIONS
OF ANY KIND, either express or implied. See the License for the specific language
governing permissions and limitations under the License.
*/

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonFormulaErrorKind {
    SyntaxError,
    TypeError,
    FunctionError,
    EvaluationError,
}

#[derive(Debug, Error, Clone)]
#[error("{kind:?}: {message}")]
pub struct JsonFormulaError {
    pub kind: JsonFormulaErrorKind,
    pub message: String,
}

impl JsonFormulaError {
    pub fn syntax(message: impl Into<String>) -> Self {
        Self {
            kind: JsonFormulaErrorKind::SyntaxError,
            message: message.into(),
        }
    }

    pub fn ty(message: impl Into<String>) -> Self {
        Self {
            kind: JsonFormulaErrorKind::TypeError,
            message: message.into(),
        }
    }

    pub fn function(message: impl Into<String>) -> Self {
        Self {
            kind: JsonFormulaErrorKind::FunctionError,
            message: message.into(),
        }
    }

    pub fn evaluation(message: impl Into<String>) -> Self {
        Self {
            kind: JsonFormulaErrorKind::EvaluationError,
            message: message.into(),
        }
    }
}
