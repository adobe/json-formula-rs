# Sub-Expression Tracing Design

**Date:** 2026-06-26  
**Status:** Approved  

## Problem

The evaluator returns a single result for an expression. When an expression like `A && B` evaluates to `false`, the caller cannot tell whether `A` was false, `B` was false, or both. This makes debugging formula failures opaque.

## Goal

Add an opt-in tracing mode that returns the result of each meaningful sub-expression alongside the final result, without changing the existing API.

## Output Structure

```rust
pub struct SubExprTrace {
    pub expr: String,        // reconstructed expression text, e.g. "A && B"
    pub value: JsonValue,    // evaluated result
    pub children: Vec<SubExprTrace>,
}
```

The trace is a tree that mirrors evaluation structure. Each node shows the reconstructed expression text and its result. Children are sub-expressions that were evaluated to produce the parent result.

**Example:** `A && B` with `A = true`, `B = false`

```
SubExprTrace {
  expr: "A && B",
  value: false,
  children: [
    SubExprTrace { expr: "A", value: true,  children: [] },
    SubExprTrace { expr: "B", value: false, children: [] },
  ]
}
```

**Short-circuit behavior:** Unevaluated branches do not appear. For `false && B`, the trace has one child (`A`), not two.

## Expression Text Reconstruction

`AstNode` gains a `to_expr_string() -> String` method that reconstructs normalized expression text from the AST. The output is not identical to the source but is unambiguous and readable.

Binary sub-expressions wrap in parentheses when nested to preserve precedence clarity:

| Expression | Reconstructed |
|---|---|
| `A&&B` | `A && B` |
| `foo.bar.baz` | `foo.bar.baz` |
| `sum(a, b) > 10` | `sum(a, b) > 10` |
| `(A + B) * C` | `(A + B) * C` |
| `arr[0]` | `arr[0]` |

## Nodes That Emit Trace Entries

Not every node is worth tracing. The following emit entries because their results are non-obvious:

**Traced:**
- Binary expressions: `AndExpression`, `OrExpression`, `AddExpression`, `SubtractExpression`, `MultiplyExpression`, `DivideExpression`, `ConcatenateExpression`, `Comparator`
- Unary expressions: `NotExpression`, `UnaryMinusExpression`
- `Pipe`
- `Function` calls
- `Projection`, `ValueProjection`, `FilterProjection`
- `ChainedExpression` — one entry per step, showing the accumulated path
- `BracketExpression` — one entry for the full bracket access

**Not traced** (self-evident or too noisy):
- `Literal`, `Number`, `Integer`, `String` — value equals expression text
- `Identifier`, `QuotedIdentifier` — leaf nodes
- `Current`, `Global`, `Identity` — contextual references
- `ArrayExpression`, `ObjectExpression` — constructors
- `Index`, `Slice` — structural, subordinate to `BracketExpression`

## Public API

Two new methods on `JsonFormula`, mirroring the existing `run()` and `evaluate()`:

```rust
/// Run a pre-compiled AST with sub-expression tracing.
pub fn run_with_trace(
    &mut self,
    ast: &AstNode,
    json: &JsonValue,
    globals: Option<&JsonValue>,
    language: Option<&str>,
    fields_only: bool,
) -> Result<(JsonValue, SubExprTrace), JsonFormulaError>

/// Compile and run an expression string with sub-expression tracing.
pub fn evaluate_with_trace(
    &mut self,
    expression: &str,
    json: &JsonValue,
    globals: Option<&JsonValue>,
    language: Option<&str>,
    fields_only: bool,
) -> Result<(JsonValue, SubExprTrace), JsonFormulaError>
```

`SubExprTrace` is exported from `src/lib.rs` alongside the existing `EvalOutcome` and `JsonFormula`.

The existing `run()`, `evaluate()`, and `search()` methods are unchanged.

## Internal Implementation

### Interpreter changes (`src/interpreter.rs`)

Add an optional trace output pointer to `Interpreter`:

```rust
trace: Option<*mut Vec<SubExprTrace>>,
```

When tracing is active, `visit()` wraps each traced node type: evaluate as normal, then push a `SubExprTrace` with the node's `to_expr_string()`, the result, and any children collected during evaluation of sub-nodes.

Children are collected by temporarily pushing a fresh `Vec<SubExprTrace>` onto a per-call stack and draining it after the node evaluates.

For `ChainedExpression`, emit one trace entry per chain step with the accumulated path string (e.g. `"foo"`, `"foo.bar"`, `"foo.bar.baz"`).

### No changes to AST node structure

`AstNode` gains only the `to_expr_string()` method — no new fields or variants.

## Files Changed

| File | Change |
|---|---|
| `src/ast.rs` | Add `to_expr_string() -> String` to `AstNode` |
| `src/interpreter.rs` | Add optional trace field; emit trace entries in `visit()` |
| `src/runtime.rs` | Add `SubExprTrace` struct; add `run_with_trace()`, `evaluate_with_trace()` |
| `src/lib.rs` | Export `SubExprTrace` |

## Testing

- `A && B` — both branches traced with correct values
- `false && B` — only `A` traced (short-circuit)
- `A \|\| B` where `A` is truthy — only `A` traced
- `(A + B) > C` — three-level trace tree
- `foo.bar.baz` — chained expression traces each step
- `arr[0]` — bracket expression traced as single entry
- `sum(a, b)` — function call traced with argument values as children
- Existing `evaluate()` / `run()` behavior unchanged
