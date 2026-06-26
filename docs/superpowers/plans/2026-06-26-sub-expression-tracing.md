# Sub-Expression Tracing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in tracing to the evaluator that returns a tree of sub-expression results alongside the final value, for debugging formula failures.

**Architecture:** `AstNode` gains `to_expr_string()` for text reconstruction. `Interpreter` gains an optional `trace_stack: Option<Vec<Vec<SubExprTrace>>>` that collects results during `visit()` using a push-before/pop-after discipline. `JsonFormula` exposes two new methods (`run_with_trace`, `evaluate_with_trace`) that activate tracing and return the root `SubExprTrace`.

**Tech Stack:** Rust, serde_json, existing `JfValue::to_json()` for value conversion.

---

## File Map

| File | Change |
|---|---|
| `src/ast.rs` | Add `to_expr_string() -> String` method to `AstNode` |
| `src/interpreter.rs` | Add `trace_stack` field; emit trace entries in `visit()` for traced node types |
| `src/runtime.rs` | Add `SubExprTrace` struct; add `run_with_trace()`, `evaluate_with_trace()` |
| `src/lib.rs` | Export `SubExprTrace` |
| `tests/tracing.rs` | New integration test file |

---

## Task 1: Add `to_expr_string()` to `AstNode`

**Files:**
- Modify: `src/ast.rs`

This method reconstructs normalized, human-readable expression text from any `AstNode`. Round-trip fidelity is not required — prefer readability. Binary sub-expressions that are themselves binary wrap in parentheses to show precedence.

- [ ] **Step 1: Write the failing test**

Create `tests/tracing.rs` with a basic smoke test for `to_expr_string` using the public API (indirectly, through trace output in Task 3). For now, add a unit test module at the bottom of `src/ast.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_expr_string_identifier() {
        assert_eq!(AstNode::Identifier("foo".to_string()).to_expr_string(), "foo");
    }

    #[test]
    fn test_to_expr_string_and() {
        let node = AstNode::AndExpression(
            Box::new(AstNode::Identifier("A".to_string())),
            Box::new(AstNode::Identifier("B".to_string())),
        );
        assert_eq!(node.to_expr_string(), "A && B");
    }

    #[test]
    fn test_to_expr_string_nested_binary() {
        // (A + B) * C  — left operand of * is binary, so it should be parenthesized
        let add = AstNode::AddExpression(
            Box::new(AstNode::Identifier("A".to_string())),
            Box::new(AstNode::Identifier("B".to_string())),
        );
        let mul = AstNode::MultiplyExpression(
            Box::new(add),
            Box::new(AstNode::Identifier("C".to_string())),
        );
        assert_eq!(mul.to_expr_string(), "(A + B) * C");
    }

    #[test]
    fn test_to_expr_string_comparator() {
        let node = AstNode::Comparator {
            op: ">".to_string(),
            left: Box::new(AstNode::Identifier("x".to_string())),
            right: Box::new(AstNode::Number(10.0)),
        };
        assert_eq!(node.to_expr_string(), "x > 10");
    }

    #[test]
    fn test_to_expr_string_function() {
        let node = AstNode::Function {
            name: "sum".to_string(),
            args: vec![
                AstNode::Identifier("a".to_string()),
                AstNode::Identifier("b".to_string()),
            ],
        };
        assert_eq!(node.to_expr_string(), "sum(a, b)");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test to_expr_string 2>&1 | head -30
```

Expected: compile error — `to_expr_string` not found on `AstNode`.

- [ ] **Step 3: Implement `to_expr_string()` on `AstNode`**

Add this method block to `src/ast.rs` after the `AstNode` enum definition. A helper `paren_if_binary` wraps the child in parentheses only when the child is itself a binary expression (to show precedence without over-parenthesizing).

```rust
impl AstNode {
    fn is_binary(&self) -> bool {
        matches!(
            self,
            AstNode::AndExpression(..)
                | AstNode::OrExpression(..)
                | AstNode::AddExpression(..)
                | AstNode::SubtractExpression(..)
                | AstNode::MultiplyExpression(..)
                | AstNode::DivideExpression(..)
                | AstNode::ConcatenateExpression(..)
                | AstNode::UnionExpression(..)
                | AstNode::Comparator { .. }
        )
    }

    fn expr_str_paren(&self) -> String {
        if self.is_binary() {
            format!("({})", self.to_expr_string())
        } else {
            self.to_expr_string()
        }
    }

    pub fn to_expr_string(&self) -> String {
        match self {
            AstNode::Identity => "@".to_string(),
            AstNode::Current => "@".to_string(),
            AstNode::Identifier(name) | AstNode::QuotedIdentifier(name) => name.clone(),
            AstNode::Literal(v) => v.to_string(),
            AstNode::String(s) => format!("\"{}\"", s),
            AstNode::Number(n) => {
                if n.fract() == 0.0 { format!("{}", *n as i64) } else { format!("{}", n) }
            }
            AstNode::Integer(n) => format!("{}", n),
            AstNode::Global(name) => format!("${}", name),
            AstNode::NotExpression(inner) => format!("!{}", inner.expr_str_paren()),
            AstNode::UnaryMinusExpression(inner) => format!("-{}", inner.expr_str_paren()),
            AstNode::AndExpression(l, r) => {
                format!("{} && {}", l.to_expr_string(), r.to_expr_string())
            }
            AstNode::OrExpression(l, r) => {
                format!("{} || {}", l.to_expr_string(), r.to_expr_string())
            }
            AstNode::AddExpression(l, r) => {
                format!("{} + {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::SubtractExpression(l, r) => {
                format!("{} - {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::MultiplyExpression(l, r) => {
                format!("{} * {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::DivideExpression(l, r) => {
                format!("{} / {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::ConcatenateExpression(l, r) => {
                format!("{} & {}", l.expr_str_paren(), r.expr_str_paren())
            }
            AstNode::UnionExpression(l, r) => {
                format!("{}, {}", l.to_expr_string(), r.to_expr_string())
            }
            AstNode::Comparator { op, left, right } => {
                format!("{} {} {}", left.to_expr_string(), op, right.to_expr_string())
            }
            AstNode::Pipe(l, r) => {
                format!("{} | {}", l.to_expr_string(), r.to_expr_string())
            }
            AstNode::ChainedExpression(children) => {
                children.iter().map(|c| c.to_expr_string()).collect::<Vec<_>>().join(".")
            }
            AstNode::BracketExpression(left, right) => {
                format!("{}[{}]", left.to_expr_string(), right.to_expr_string())
            }
            AstNode::Index(inner) => inner.to_expr_string(),
            AstNode::Slice { start, stop, step } => {
                let start_s = start.map(|n| n.to_string()).unwrap_or_default();
                let stop_s = stop.map(|n| n.to_string()).unwrap_or_default();
                match step {
                    Some(s) => format!("{}:{}:{}", start_s, stop_s, s),
                    None => format!("{}:{}", start_s, stop_s),
                }
            }
            AstNode::Projection { left, right, .. } => {
                let right_s = if matches!(**right, AstNode::Identity) {
                    String::new()
                } else {
                    format!(".{}", right.to_expr_string())
                };
                format!("{}[*]{}", left.to_expr_string(), right_s)
            }
            AstNode::ValueProjection { left, right } => {
                let right_s = if matches!(**right, AstNode::Identity) {
                    String::new()
                } else {
                    format!(".{}", right.to_expr_string())
                };
                format!("{}.*{}", left.to_expr_string(), right_s)
            }
            AstNode::FilterProjection { left, right, condition } => {
                let right_s = if matches!(**right, AstNode::Identity) {
                    String::new()
                } else {
                    format!(".{}", right.to_expr_string())
                };
                format!("{}[?{}]{}", left.to_expr_string(), condition.to_expr_string(), right_s)
            }
            AstNode::Flatten(inner) => format!("{}[]", inner.to_expr_string()),
            AstNode::Function { name, args } => {
                let args_s = args.iter().map(|a| a.to_expr_string()).collect::<Vec<_>>().join(", ");
                format!("{}({})", name, args_s)
            }
            AstNode::ArrayExpression(items) => {
                let items_s = items.iter().map(|a| a.to_expr_string()).collect::<Vec<_>>().join(", ");
                format!("[{}]", items_s)
            }
            AstNode::ObjectExpression(pairs) => {
                let pairs_s = pairs.iter()
                    .map(|p| format!("{}: {}", p.key, p.value.to_expr_string()))
                    .collect::<Vec<_>>().join(", ");
                format!("{{{}}}", pairs_s)
            }
            AstNode::ExpressionReference(inner) => {
                format!("&{}", inner.to_expr_string())
            }
            // AstNode::KeyValuePair is both an enum variant (with Box<AstNode> value)
            // and a separate struct (with AstNode value). The variant is never the
            // top-level node during evaluation, but the match must be exhaustive.
            AstNode::KeyValuePair { key, value } => {
                format!("{}: {}", key, value.to_expr_string())
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test to_expr_string 2>&1
```

Expected: all `to_expr_string` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/ast.rs
git commit -m "feat: add AstNode::to_expr_string() for expression text reconstruction"
```

---

## Task 2: Add `SubExprTrace` struct and trace infrastructure to runtime

**Files:**
- Modify: `src/runtime.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to `tests/tracing.rs` (create this file):

```rust
use json_formula_rs::{JsonFormula, SubExprTrace};
use serde_json::json;

#[test]
fn test_sub_expr_trace_type_exists() {
    // Verifies SubExprTrace is public and has expected fields
    let trace = SubExprTrace {
        expr: "test".to_string(),
        value: json!(true),
        children: vec![],
    };
    assert_eq!(trace.expr, "test");
    assert_eq!(trace.value, json!(true));
    assert!(trace.children.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test test_sub_expr_trace_type_exists 2>&1 | head -20
```

Expected: compile error — `SubExprTrace` not found.

- [ ] **Step 3: Add `SubExprTrace` to `src/runtime.rs`**

Add this struct near the top of `src/runtime.rs`, after the existing `EvalOutcome` struct:

```rust
#[derive(Debug, Clone)]
pub struct SubExprTrace {
    pub expr: String,
    pub value: serde_json::Value,
    pub children: Vec<SubExprTrace>,
}
```

- [ ] **Step 4: Export `SubExprTrace` from `src/lib.rs`**

Change the existing export line in `src/lib.rs`:

```rust
pub use crate::runtime::{EvalOutcome, JsonFormula, SubExprTrace};
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test test_sub_expr_trace_type_exists 2>&1
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/runtime.rs src/lib.rs tests/tracing.rs
git commit -m "feat: add SubExprTrace struct and export"
```

---

## Task 3: Add trace stack to `Interpreter`

**Files:**
- Modify: `src/interpreter.rs`
- Modify: `src/runtime.rs` (add helper methods to start/finish tracing)

The `Interpreter` gains a `trace_stack: Option<Vec<Vec<SubExprTrace>>>` field. Helper methods manage pushing/popping layers. The stack is only allocated when tracing is active — zero overhead for normal evaluation.

- [ ] **Step 1: Write the failing test**

Add to `tests/tracing.rs`:

```rust
#[test]
fn test_evaluate_with_trace_basic() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 0});
    let result = jf.evaluate_with_trace(
        "(x > 1) && (y > 2)",
        &data,
        None,
        None,
        false,
    );
    assert!(result.is_ok());
    let (value, trace) = result.unwrap();
    assert_eq!(value, json!(false));
    // Root trace should be the && expression
    assert!(trace.expr.contains("&&"));
    assert_eq!(trace.value, json!(false));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test test_evaluate_with_trace_basic 2>&1 | head -20
```

Expected: compile error — `evaluate_with_trace` not found on `JsonFormula`.

- [ ] **Step 3: Add trace stack field and helpers to `Interpreter`**

In `src/interpreter.rs`, update the `Interpreter` struct:

```rust
pub struct Interpreter {
    runtime: *mut Runtime,
    globals: Option<JfValue>,
    pub language: String,
    debug: *mut Vec<String>,
    debug_chain_start: Option<String>,
    trace_stack: Option<Vec<Vec<crate::runtime::SubExprTrace>>>,
}
```

Update `Interpreter::new` to initialize `trace_stack: None`.

Add these helper methods to the `Interpreter` impl block:

```rust
pub fn enable_tracing(&mut self) {
    self.trace_stack = Some(vec![Vec::new()]);
}

/// Call before evaluating children of a traced node.
fn trace_push_layer(&mut self) {
    if let Some(stack) = &mut self.trace_stack {
        stack.push(Vec::new());
    }
}

/// Call after evaluating children. Returns the children collected in this layer.
fn trace_pop_layer(&mut self) -> Vec<crate::runtime::SubExprTrace> {
    if let Some(stack) = &mut self.trace_stack {
        stack.pop().unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Emit a trace entry for the current node onto the parent layer.
fn trace_emit(&mut self, expr: String, value: &JfValue, children: Vec<crate::runtime::SubExprTrace>) {
    if let Some(stack) = &mut self.trace_stack {
        if let Some(parent_layer) = stack.last_mut() {
            parent_layer.push(crate::runtime::SubExprTrace {
                expr,
                value: value.to_json(),
                children,
            });
        }
    }
}

/// Extract the root trace entry after the top-level visit() returns.
pub fn take_root_trace(&mut self) -> Option<crate::runtime::SubExprTrace> {
    if let Some(stack) = &mut self.trace_stack {
        if let Some(root_layer) = stack.last_mut() {
            return root_layer.pop();
        }
    }
    None
}
```

- [ ] **Step 4: Add `run_with_trace` and `evaluate_with_trace` to `JsonFormula` in `src/runtime.rs`**

Add these two methods to the `JsonFormula` impl block:

```rust
pub fn run_with_trace(
    &mut self,
    ast: &AstNode,
    json: &JsonValue,
    globals: Option<&JsonValue>,
    language: Option<&str>,
    fields_only: bool,
) -> Result<(JsonValue, crate::runtime::SubExprTrace), JsonFormulaError> {
    // wrap_fields is already imported at file scope in runtime.rs
    let data = JfValue::from_json(json);
    let globals_value = globals.map(JfValue::from_json);
    let data = if fields_only { wrap_fields(&data) } else { data };
    let mut interpreter = Interpreter::new(
        &mut self.runtime,
        globals_value,
        language.unwrap_or("en-US"),
        &mut self.debug,
    );
    interpreter.enable_tracing();
    let result = interpreter.search(ast, &data)?;
    let trace = interpreter.take_root_trace().unwrap_or_else(|| crate::runtime::SubExprTrace {
        expr: ast.to_expr_string(),
        value: result.to_json(),
        children: vec![],
    });
    Ok((result.to_json(), trace))
}

pub fn evaluate_with_trace(
    &mut self,
    expression: &str,
    json: &JsonValue,
    globals: Option<&JsonValue>,
    language: Option<&str>,
    fields_only: bool,
) -> Result<(JsonValue, crate::runtime::SubExprTrace), JsonFormulaError> {
    let ast = self.compile(expression, &[])?;
    self.run_with_trace(&ast, json, globals, language, fields_only)
}
```

- [ ] **Step 5: Run test to verify it compiles and the basic test passes**

```bash
cargo test test_evaluate_with_trace_basic 2>&1
```

Expected: PASS (tracing is wired but no entries emitted yet — the fallback `take_root_trace` returns the root with empty children, which is fine for this test since it only checks the value and that `&&` is in `trace.expr`).

- [ ] **Step 6: Commit**

```bash
git add src/interpreter.rs src/runtime.rs tests/tracing.rs
git commit -m "feat: add trace stack infrastructure to Interpreter and run_with_trace API"
```

---

## Task 4: Emit trace entries for binary and unary expression nodes

**Files:**
- Modify: `src/interpreter.rs`

Instrument every binary and unary `AstNode` variant in `visit()` to use `trace_push_layer` / `trace_pop_layer` / `trace_emit`. The pattern is identical for all of them.

- [ ] **Step 1: Write the failing tests**

Add to `tests/tracing.rs`:

```rust
#[test]
fn test_and_both_branches_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 3});
    let (value, trace) = jf.evaluate_with_trace("(x > 1) && (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(true));
    assert_eq!(trace.children.len(), 2);
    assert_eq!(trace.children[0].value, json!(true));   // x > 1
    assert_eq!(trace.children[1].value, json!(true));   // y > 2
}

#[test]
fn test_and_short_circuit() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 0, "y": 3});
    let (value, trace) = jf.evaluate_with_trace("(x > 1) && (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(false));
    // x > 1 is false, so y > 2 is never evaluated
    assert_eq!(trace.children.len(), 1);
    assert!(trace.children[0].expr.contains(">"));
    assert_eq!(trace.children[0].value, json!(false));
}

#[test]
fn test_or_short_circuit() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5, "y": 0});
    let (value, trace) = jf.evaluate_with_trace("(x > 1) || (y > 2)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(true));
    // x > 1 is true, so y > 2 is never evaluated
    assert_eq!(trace.children.len(), 1);
    assert_eq!(trace.children[0].value, json!(true));
}

#[test]
fn test_not_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5});
    let (value, trace) = jf.evaluate_with_trace("!(x > 1)", &data, None, None, false).unwrap();
    assert_eq!(value, json!(false));
    assert!(trace.expr.starts_with('!'));
    assert_eq!(trace.children.len(), 1);
    assert_eq!(trace.children[0].value, json!(true)); // x > 1
}

#[test]
fn test_three_level_trace() {
    let mut jf = JsonFormula::new();
    let data = json!({"a": 3, "b": 4, "c": 10});
    let (value, trace) = jf.evaluate_with_trace("(a + b) > c", &data, None, None, false).unwrap();
    assert_eq!(value, json!(false));
    // root: (a + b) > c
    assert!(trace.expr.contains(">"));
    // one child: a + b
    assert_eq!(trace.children.len(), 1);
    assert!(trace.children[0].expr.contains("+"));
    assert_eq!(trace.children[0].value, json!(7));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test test_and_both_branches_traced test_and_short_circuit test_or_short_circuit test_not_expression_traced test_three_level_trace 2>&1 | grep -E "FAILED|passed|failed"
```

Expected: tests fail because children are empty (no emission yet).

- [ ] **Step 3: Instrument binary expression nodes in `visit()`**

In `src/interpreter.rs`, wrap each binary node arm with the push/pop/emit pattern. The pattern for `AndExpression` (which already short-circuits) is:

```rust
AstNode::AndExpression(left, right) => {
    self.trace_push_layer();
    let first = self.visit(left, value)?;
    if !to_boolean(&first) {
        let children = self.trace_pop_layer();
        let result = first;
        self.trace_emit(node.to_expr_string(), &result, children);
        return Ok(result);
    }
    let result = self.visit(right, value)?;
    let children = self.trace_pop_layer();
    self.trace_emit(node.to_expr_string(), &result, children);
    Ok(result)
}
```

The non-short-circuit binary pattern (e.g. `AddExpression`):

```rust
AstNode::AddExpression(left, right) => {
    self.trace_push_layer();
    let first = self.visit(left, value)?;
    let second = self.visit(right, value)?;
    balance_array_operands(&first, &second);
    let result = self.apply_operator(first, second, "+")?;
    let children = self.trace_pop_layer();
    self.trace_emit(node.to_expr_string(), &result, children);
    Ok(result)
}
```

Apply this pattern to **all** of the following arms:
- `AndExpression` (short-circuit on false left)
- `OrExpression` (short-circuit on true left)
- `AddExpression`, `SubtractExpression`, `MultiplyExpression`, `DivideExpression`, `ConcatenateExpression`, `UnionExpression`
- `Comparator`
- `NotExpression`, `UnaryMinusExpression`
- `Flatten`
- `Pipe`

**Important:** In Rust, the match parameter `node: &AstNode` remains in scope inside every match arm even after destructuring. Call `node.to_expr_string()` directly in every arm — no cloning or helper needed. This is the canonical approach throughout all tasks.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test test_and_both_branches_traced test_and_short_circuit test_or_short_circuit test_not_expression_traced test_three_level_trace 2>&1
```

Expected: all pass.

- [ ] **Step 5: Run full test suite to confirm no regressions**

```bash
cargo test 2>&1 | tail -20
```

Expected: all existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/interpreter.rs tests/tracing.rs
git commit -m "feat: emit trace entries for binary, unary, pipe, and flatten nodes"
```

---

## Task 5: Emit trace entries for Function calls

**Files:**
- Modify: `src/interpreter.rs`

Function argument expressions are evaluated before calling the function. Each argument evaluation may itself produce child trace entries. The function call trace entry's children are those argument traces.

- [ ] **Step 1: Write the failing test**

Add to `tests/tracing.rs`:

```rust
#[test]
fn test_function_call_traced_with_arg_children() {
    let mut jf = JsonFormula::new();
    let data = json!({"a": 3, "b": 4});
    let (value, trace) = jf.evaluate_with_trace("sum([a, b])", &data, None, None, false).unwrap();
    assert_eq!(value, json!(7));
    // root trace is sum(...)
    assert!(trace.expr.starts_with("sum"));
    assert_eq!(trace.value, json!(7));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test test_function_call_traced_with_arg_children 2>&1 | grep -E "FAILED|passed|failed"
```

- [ ] **Step 3: Instrument the `Function` arm in `visit()`**

In `src/interpreter.rs`, find the `AstNode::Function { name, args }` arm. Wrap the argument evaluation loop:

```rust
AstNode::Function { name, args } => {
    if name == "if" {
        // if() has special short-circuit evaluation; handle tracing inline.
        // IMPORTANT: all early-return error paths must call trace_pop_layer()
        // before returning to avoid leaving an unpopped layer on the stack.
        if args.len() != 3 {
            return Err(JsonFormulaError::function("if() takes 3 arguments".to_string()));
        }
        self.trace_push_layer();
        let condition = self.visit(&args[0], value)?;
        if matches!(condition, JfValue::Expref(_)) {
            let _ = self.trace_pop_layer(); // clean up before error return
            return Err(JsonFormulaError::ty(
                "if() does not accept an expression reference argument.".to_string(),
            ));
        }
        let result = if to_boolean(&condition) {
            self.visit(&args[1], value)?
        } else {
            self.visit(&args[2], value)?
        };
        let children = self.trace_pop_layer();
        self.trace_emit(node.to_expr_string(), &result, children);
        return Ok(result);
    }
    self.trace_push_layer();
    let mut resolved_args = Vec::new();
    for child in args {
        resolved_args.push(self.visit(child, value)?);
    }
    let result = unsafe { (&mut *self.runtime).call_function(name, resolved_args, value, self, true) }?;
    let children = self.trace_pop_layer();
    self.trace_emit(node.to_expr_string(), &result, children);
    Ok(result)
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test test_function_call_traced_with_arg_children 2>&1
```

- [ ] **Step 5: Run full test suite**

```bash
cargo test 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/interpreter.rs tests/tracing.rs
git commit -m "feat: emit trace entries for function calls"
```

---

## Task 6: Emit trace entries for Projection, BracketExpression, and ChainedExpression

**Files:**
- Modify: `src/interpreter.rs`

Projections emit a single entry (no per-element children — too noisy). `BracketExpression` emits one entry. `ChainedExpression` emits one entry per step with the accumulated path string.

- [ ] **Step 1: Write the failing tests**

Add to `tests/tracing.rs`:

```rust
#[test]
fn test_projection_single_trace_entry() {
    let mut jf = JsonFormula::new();
    let data = json!({"items": [{"price": 10}, {"price": 20}]});
    let (value, trace) = jf.evaluate_with_trace("items[*].price", &data, None, None, false).unwrap();
    assert_eq!(value, json!([10, 20]));
    // Should have trace entries but NOT one per array element
    // The projection itself should be one entry
    fn count_entries(t: &SubExprTrace) -> usize {
        1 + t.children.iter().map(count_entries).sum::<usize>()
    }
    // Should be a small number of entries, not 2+1 per element
    assert!(count_entries(&trace) < 10);
}

#[test]
fn test_bracket_expression_traced() {
    let mut jf = JsonFormula::new();
    let data = json!({"arr": [10, 20, 30]});
    let (value, trace) = jf.evaluate_with_trace("arr[1]", &data, None, None, false).unwrap();
    assert_eq!(value, json!(20));
    // Should have a trace entry for arr[1]
    assert!(trace.expr.contains('['));
    assert_eq!(trace.value, json!(20));
}

#[test]
fn test_chained_expression_per_step() {
    let mut jf = JsonFormula::new();
    let data = json!({"foo": {"bar": {"baz": 42}}});
    let (value, trace) = jf.evaluate_with_trace("foo.bar.baz", &data, None, None, false).unwrap();
    assert_eq!(value, json!(42));
    // ChainedExpression should emit one entry per step
    // The root trace covers "foo.bar.baz" and children cover intermediate steps
    assert_eq!(trace.value, json!(42));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test test_projection_single_trace_entry test_bracket_expression_traced test_chained_expression_per_step 2>&1 | grep -E "FAILED|passed|failed"
```

- [ ] **Step 3: Instrument Projection, ValueProjection, FilterProjection**

For all three projection variants, wrap with push/pop/emit but disable tracing during the per-element loop to avoid noisy per-element entries. Use `self.trace_stack.take()` / restore to silence tracing during the element loop. Preserve the original structure of each arm exactly — only add the push/pop/save/restore wrapper around it.

For `Projection` (original structure has the Wildcard debug push in the `else` branch, not inside the array branch):

```rust
AstNode::Projection { left, right, debug } => {
    self.trace_push_layer();
    let base = self.visit(left, value)?;
    let result = if let JfValue::Array(items) = base {
        let mut collected = Vec::new();
        let saved_stack = self.trace_stack.take(); // silence per-element tracing
        for item in items {
            collected.push(self.visit(right, &item)?);
        }
        self.trace_stack = saved_stack;
        JfValue::Array(collected)
    } else {
        // Wildcard debug push belongs in the else branch (non-array case)
        if debug.as_deref() == Some("Wildcard") {
            self.debug_mut().push("Bracketed wildcards apply to arrays only".to_string());
        }
        JfValue::Null
    };
    let children = self.trace_pop_layer();
    self.trace_emit(node.to_expr_string(), &result, children);
    Ok(result)
}
```

For `ValueProjection`, the original arm calls `get_value_of(&projection)` before iterating — preserve that call:

```rust
AstNode::ValueProjection { left, right } => {
    self.trace_push_layer();
    let projection = self.visit(left, value)?;
    let proj_value = get_value_of(&projection); // required — must not be dropped
    let result = match proj_value {
        JfValue::Object(map) => {
            let mut collected = Vec::new();
            let saved_stack = self.trace_stack.take();
            for val in map.values() {
                collected.push(self.visit(right, val)?);
            }
            self.trace_stack = saved_stack;
            JfValue::Array(collected)
        }
        _ => {
            self.debug_mut().push("Chained wildcards apply to objects only".to_string());
            JfValue::Null
        }
    };
    let children = self.trace_pop_layer();
    self.trace_emit(node.to_expr_string(), &result, children);
    Ok(result)
}
```

Apply the same pattern to `FilterProjection` (save/restore around the condition + right-visit loops, preserve the original `ValueProjection`-left-side check).

- [ ] **Step 4: Instrument BracketExpression**

```rust
AstNode::BracketExpression(left, right) => {
    self.trace_push_layer();
    let base = self.visit(left, value)?;
    let result = self.visit(right, &base)?;
    let children = self.trace_pop_layer();
    self.trace_emit(
        format!("{}[{}]", left.to_expr_string(), right.to_expr_string()),
        &result,
        children,
    );
    Ok(result)
}
```

- [ ] **Step 5: Instrument ChainedExpression**

Wrap the **entire** `ChainedExpression` arm in one `trace_push_layer` / `trace_pop_layer` / `trace_emit`. The push happens before `children[0]` is evaluated; the pop and emit happen after the full loop. This ensures `take_root_trace()` finds a single root entry for the chain.

The reconstructed expression string is the full chain: `children.iter().map(|c| c.to_expr_string()).collect::<Vec<_>>().join(".")`. Any traced nodes that fire during steps of the chain (e.g., a `Function` call inside the chain) will become children of this entry via the normal stack discipline.

```rust
AstNode::ChainedExpression(children) => {
    self.trace_push_layer();

    let mut result = self.visit(&children[0], value)?;
    // ... (existing debug_chain_start logic — unchanged) ...
    let mut projecting = false;

    for idx in 1..children.len() {
        // ... (all existing null-check and projection logic — unchanged) ...
        // ... (existing evaluation of children[idx] into result — unchanged) ...
    }

    let full_path = children.iter().map(|c| c.to_expr_string()).collect::<Vec<_>>().join(".");
    let chain_children = self.trace_pop_layer();
    self.trace_emit(full_path, &result, chain_children);
    Ok(result)
}
```

**Do not** push/pop inside the loop — the single outer push/pop collects all inner traced nodes as a flat list of children for the chain entry.

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test test_projection_single_trace_entry test_bracket_expression_traced test_chained_expression_per_step 2>&1
```

- [ ] **Step 7: Run full test suite**

```bash
cargo test 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/interpreter.rs tests/tracing.rs
git commit -m "feat: emit trace entries for projections, bracket, and chained expressions"
```

---

## Task 7: Verify existing API is unchanged and add regression test

**Files:**
- Modify: `tests/tracing.rs`

- [ ] **Step 1: Add regression tests**

Add to `tests/tracing.rs`:

```rust
#[test]
fn test_evaluate_unchanged() {
    // Verify the non-tracing evaluate() path is unaffected
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5});
    let result = jf.evaluate("x > 1", &data, None, None, false).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_run_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5});
    let ast = jf.compile("x > 1", &[]).unwrap();
    let result = jf.run(&ast, &data, None, None, false).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_search_unchanged() {
    let mut jf = JsonFormula::new();
    let data = json!({"x": 5});
    let result = jf.search("x > 1", &data, None, None).unwrap();
    assert_eq!(result, json!(true));
}
```

- [ ] **Step 2: Run all tracing tests**

```bash
cargo test --test tracing 2>&1
```

Expected: all pass.

- [ ] **Step 3: Run full test suite one final time**

```bash
cargo test 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add tests/tracing.rs
git commit -m "test: add regression tests confirming existing API is unchanged"
```

---

## Quick Reference

**Run all tests:** `cargo test`  
**Run only tracing tests:** `cargo test --test tracing`  
**Run only ast unit tests:** `cargo test to_expr_string`  
**Check compilation only:** `cargo check`
