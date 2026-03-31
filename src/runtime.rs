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

use std::collections::HashMap;

use serde_json::Value as JsonValue;

use crate::ast::AstNode;
use crate::errors::JsonFormulaError;
use crate::functions::builtin_functions;
use crate::interpreter::Interpreter;
use crate::parser::Parser;
use crate::types::{DataType, JfValue};
use crate::utils::wrap_fields;

#[derive(Debug, Clone)]
pub struct EvalOutcome {
    pub ok: bool,
    pub value: Option<JsonValue>,
    pub error: Option<JsonFormulaError>,
}

pub struct JsonFormula {
    runtime: Runtime,
    debug: Vec<String>,
}

impl JsonFormula {
    pub fn new() -> Self {
        Self {
            runtime: Runtime::new(),
            debug: Vec::new(),
        }
    }

    pub fn compile(
        &mut self,
        expression: &str,
        allowed_globals: &[String],
    ) -> Result<AstNode, JsonFormulaError> {
        let mut parser = Parser::new(allowed_globals, &mut self.debug);
        parser.parse(expression)
    }

    pub fn search(
        &mut self,
        expression: &str,
        json: &JsonValue,
        globals: Option<&JsonValue>,
        language: Option<&str>,
    ) -> Result<JsonValue, JsonFormulaError> {
        let ast = self.compile(expression, &[])?;
        self.run(&ast, json, globals, language, false)
    }

    pub fn run(
        &mut self,
        ast: &AstNode,
        json: &JsonValue,
        globals: Option<&JsonValue>,
        language: Option<&str>,
        fields_only: bool,
    ) -> Result<JsonValue, JsonFormulaError> {
        let data = JfValue::from_json(json);
        let globals_value = globals.map(JfValue::from_json);
        let data = if fields_only {
            wrap_fields(&data)
        } else {
            data
        };
        let mut interpreter = Interpreter::new(
            &mut self.runtime,
            globals_value,
            language.unwrap_or("en-US"),
            &mut self.debug,
        );
        let result = interpreter.search(ast, &data)?;
        Ok(result.to_json())
    }

    pub fn evaluate(
        &mut self,
        expression: &str,
        json: &JsonValue,
        globals: Option<&JsonValue>,
        language: Option<&str>,
        fields_only: bool,
    ) -> Result<JsonValue, JsonFormulaError> {
        let ast = self.compile(expression, &[])?;
        self.run(&ast, json, globals, language, fields_only)
    }

    pub fn debug(&self) -> &[String] {
        &self.debug
    }

    /// Registers a custom function that evaluates the given expression body.
    /// The function accepts zero or one argument. When called with no argument,
    /// the body is evaluated with the current context (`data`); when called
    /// with one argument, the body is evaluated with that argument as the
    /// current value (so `@` in the body refers to the argument).
    pub fn register_expression(
        &mut self,
        name: &str,
        body: &str,
    ) -> Result<(), JsonFormulaError> {
        let ast = self.compile(body, &[])?;
        let body_ast = ast.clone();
        let entry = FunctionEntry {
            func: Box::new(move |_runtime, args, data, interp| {
                let value = if args.is_empty() {
                    data.clone()
                } else {
                    args[0].clone()
                };
                interp.visit(&body_ast, &value)
            }),
            signature: vec![SignatureArg {
                types: vec![DataType::Any],
                optional: true,
                variadic: false,
            }],
            expref: Some(ast),
        };
        self.runtime.functions.insert(name.to_string(), entry);
        Ok(())
    }
}

#[derive(Clone)]
pub struct SignatureArg {
    pub types: Vec<DataType>,
    pub optional: bool,
    pub variadic: bool,
}

pub type FunctionImpl = Box<
    dyn Fn(&mut Runtime, Vec<JfValue>, &JfValue, &mut Interpreter) -> Result<JfValue, JsonFormulaError>
        + Send
        + Sync,
>;

pub struct FunctionEntry {
    pub func: FunctionImpl,
    pub signature: Vec<SignatureArg>,
    pub expref: Option<AstNode>,
}

pub struct Runtime {
    pub functions: HashMap<String, FunctionEntry>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            functions: builtin_functions(),
        }
    }

    pub fn call_function(
        &mut self,
        name: &str,
        resolved_args: Vec<JfValue>,
        data: &JfValue,
        interpreter: &mut Interpreter,
        resolved: bool,
    ) -> Result<JfValue, JsonFormulaError> {
        let signature = self
            .functions
            .get(name)
            .ok_or_else(|| JsonFormulaError::function(format!("No such function: {}()", name)))?
            .signature
            .clone();
        self.validate_args(name, &resolved_args, &signature, resolved)?;
        let func_ptr = {
            let entry = self
                .functions
                .get(name)
                .ok_or_else(|| JsonFormulaError::function(format!("No such function: {}()", name)))?;
            &*entry.func
                as *const dyn Fn(&mut Runtime, Vec<JfValue>, &JfValue, &mut Interpreter) -> Result<JfValue, JsonFormulaError>
        };
        let func = unsafe { &*func_ptr };
        func(self, resolved_args, data, interpreter)
    }

    fn validate_args(
        &self,
        name: &str,
        args: &[JfValue],
        signature: &[SignatureArg],
        resolved: bool,
    ) -> Result<(), JsonFormulaError> {
        if signature.is_empty() && !args.is_empty() {
            return Err(JsonFormulaError::function(format!(
                "{}() does not accept parameters",
                name
            )));
        }
        if signature.is_empty() {
            return Ok(());
        }

        let args_needed = signature.iter().filter(|arg| !arg.optional).count();
        let last_arg = signature.last().unwrap();
        if last_arg.variadic {
            if args.len() < signature.len() && !last_arg.optional {
                let plural = if signature.len() == 1 { " argument" } else { " arguments" };
                return Err(JsonFormulaError::function(format!(
                    "{}() takes at least {}{} but received {}",
                    name,
                    signature.len(),
                    plural,
                    args.len()
                )));
            }
        } else if args.len() < args_needed || args.len() > signature.len() {
            let plural = if signature.len() == 1 { " argument" } else { " arguments" };
            return Err(JsonFormulaError::function(format!(
                "{}() takes {}{} but received {}",
                name,
                signature.len(),
                plural,
                args.len()
            )));
        }
        if !resolved {
            return Ok(());
        }

        let limit = if last_arg.variadic {
            args.len()
        } else {
            signature.len().min(args.len())
        };
        for i in 0..limit {
            let expected = if i >= signature.len() {
                &signature[signature.len() - 1].types
            } else {
                &signature[i].types
            };
            let arg = args[i].clone();
            crate::types::match_type(expected, arg, name, |v| self.to_number(&v), |v| {
                self.to_string(&v)
            })?;
        }
        Ok(())
    }

    pub fn to_number(&self, value: &JfValue) -> Result<f64, JsonFormulaError> {
        use crate::utils::get_value_of;
        let val = get_value_of(value);
        match val {
            JfValue::Null => Ok(0.0),
            JfValue::Number(n) => Ok(n),
            JfValue::String(s) => strict_string_to_number(&s),
            JfValue::Bool(b) => Ok(if b { 1.0 } else { 0.0 }),
            JfValue::Array(_) => Err(JsonFormulaError::ty("Failed to convert array to number")),
            JfValue::Object(_) => Err(JsonFormulaError::ty("Failed to convert object to number")),
            JfValue::Field { .. } => Err(JsonFormulaError::ty("Failed to convert object to number")),
            JfValue::Expref(_) => Err(JsonFormulaError::ty("Failed to convert expression to number")),
        }
    }

    pub fn to_string(&self, value: &JfValue) -> Result<String, JsonFormulaError> {
        use crate::utils::get_value_of;
        let val = get_value_of(value);
        match val {
            JfValue::Null => Ok(String::new()),
            JfValue::String(s) => Ok(s),
            JfValue::Number(n) => Ok(n.to_string()),
            JfValue::Bool(b) => Ok(b.to_string()),
            JfValue::Array(_) => Err(JsonFormulaError::ty("Failed to convert array to string")),
            JfValue::Object(_) => Err(JsonFormulaError::ty("Failed to convert object to string")),
            JfValue::Field { .. } => Err(JsonFormulaError::ty("Failed to convert object to string")),
            JfValue::Expref(_) => Err(JsonFormulaError::ty("Failed to convert expression to string")),
        }
    }
}

fn strict_string_to_number(input: &str) -> Result<f64, JsonFormulaError> {
    let re = regex::Regex::new(r"^\s*(-|\+)?(\d*)(\.\d+)?(e(\+|-)?\d+)?\s*$")
        .expect("regex should compile");
    if !re.is_match(input) {
        return Err(JsonFormulaError::ty(format!(
            "Failed to convert \"{}\" to number",
            input
        )));
    }
    let val: f64 = input.trim().parse().map_err(|_| {
        JsonFormulaError::ty(format!("Failed to convert \"{}\" to number", input))
    })?;
    if !val.is_finite() {
        return Err(JsonFormulaError::ty(format!(
            "Failed to convert \"{}\" to number",
            input
        )));
    }
    Ok(val)
}
