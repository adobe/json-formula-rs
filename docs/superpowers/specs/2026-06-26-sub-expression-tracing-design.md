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

**Example:** `(X > 1) && (Y > 2)` with `X = 5`, `Y = 0`

```
SubExprTrace {
  expr: "(X > 1) && (Y > 2)",
  value: false,
  children: [
    SubExprTrace { expr: "X > 1", value: true,  children: [...] },
    SubExprTrace { expr: "Y > 2", value: false, children: [...] },
  ]
}
```

**Short-circuit behavior:** Unevaluated branches do not appear. For `(X > 1) && (Y > 2)` where `X > 1` is false, only the left child appears — `Y > 2` is never evaluated and has no trace entry.

## Expression Text Reconstruction

`AstNode` gains a `to_expr_string() -> String` method that reconstructs normalized expression text from the AST. The output is not identical to the source but is unambiguous and readable.

Binary sub-expressions wrap in parentheses when nested to preserve precedence clarity. Round-trip fidelity is not required — prefer readability over exactness. The guiding rule: an implementer who reads the reconstructed string should be able to identify which part of the original expression it corresponds to.

Illustrative examples:

| Expression | Reconstructed |
|---|---|
| `A&&B` | `A && B` |
| `foo.bar.baz` | `foo.bar.baz` |
| `sum(a, b) > 10` | `sum(a, b) > 10` |
| `(A + B) * C` | `(A + B) * C` |
| `arr[0]` | `arr[0]` |
| `arr[1:3]` | `arr[1:3]` |
| `arr[*]` | `arr[*]` |
| `arr[?x > 1].y` | `arr[?x > 1].y` |
| `{key: val}` | `{key: val}` |
| `[a, b, c]` | `[a, b, c]` |

All `AstNode` variants must have a `to_expr_string()` implementation. For any variant not covered by the table above, the implementer may use a concise placeholder (e.g. `"<expr>"`) as long as it does not produce an empty string.

## Nodes That Emit Trace Entries

Not every node is worth tracing. The following emit entries because their results are non-obvious:

**Traced:**
- Binary expressions: `AndExpression`, `OrExpression`, `AddExpression`, `SubtractExpression`, `MultiplyExpression`, `DivideExpression`, `ConcatenateExpression`, `UnionExpression`, `Comparator`
- Unary expressions: `NotExpression`, `UnaryMinusExpression`, `Flatten`
- `Pipe`
- `Function` calls — children are the trace entries produced by evaluating each argument expression in order
- `Projection`, `ValueProjection`, `FilterProjection` — each emits a single trace entry whose value is the final result array; per-element evaluation does not produce child entries (doing so would generate one entry per array element, which is too noisy for the debugging use case)
- `ChainedExpression` — one entry per step, showing the accumulated path string (e.g. `"foo"`, `"foo.bar"`, `"foo.bar.baz"`); inner nodes within a chain step that are themselves traced emit independent child entries under that step's trace entry
- `BracketExpression` — one entry for the full bracket access

**Not traced** (self-evident or too noisy):
- `Literal`, `Number`, `Integer`, `String` — value equals expression text
- `Identifier`, `QuotedIdentifier` — leaf nodes
- `Current`, `Global`, `Identity` — contextual references
- `ArrayExpression`, `ObjectExpression` — constructors
- `ExpressionReference` — packages an AST node as a `JfValue::Expref`; the node is not evaluated at this point and emits no trace entry here. Tracing does not follow into deferred evaluation inside function bodies.
- `KeyValuePair` — subsumed by `ObjectExpression`; not reachable as a standalone top-level dispatch target
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

`SubExprTrace` is defined in `src/runtime.rs` and exported from `src/lib.rs` alongside the existing `EvalOutcome` and `JsonFormula`.

The existing `run()`, `evaluate()`, and `search()` methods are unchanged.

## Internal Implementation

### Interpreter changes (`src/interpreter.rs`)

Add a child-collection stack to `Interpreter`:

```rust
trace_stack: Option<Vec<Vec<SubExprTrace>>>,
```

When tracing is active, `visit()` wraps each traced node type as follows:

1. Push a fresh `Vec<SubExprTrace>` onto `trace_stack` before recursing into child nodes.
2. Evaluate the node (which recurses into children; each child's traced result is pushed onto the top of the stack).
3. Pop the top `Vec<SubExprTrace>` — these are the children of the current node.
4. Build `SubExprTrace { expr: node.to_expr_string(), value: result.to_json(), children }` — `JfValue::to_json()` already exists in the codebase and converts a `JfValue` to `serde_json::Value`.
5. Push it onto the new top of the stack (the parent's collection layer).

The root call initializes `trace_stack` with one empty `Vec`. After `visit()` returns at the root, that `Vec` contains exactly one entry — the top-level `SubExprTrace`.

For `ChainedExpression`, emit one trace entry per chain step. The accumulated path string grows as each step is processed (e.g. `"foo"`, `"foo.bar"`, `"foo.bar.baz"`). Inner nodes within a step that are themselves traced emit as children of that step's entry using the same stack discipline.

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

- `(X > 1) && (Y > 2)` — both branches traced with correct values
- `(X > 1) && (Y > 2)` where `X > 1` is false — only left branch traced (short-circuit)
- `(X > 1) \|\| (Y > 2)` where `X > 1` is truthy — only left branch traced
- `(A + B) > C` — three-level trace tree
- `foo.bar.baz` — chained expression traces each step
- `arr[0]` — bracket expression traced as single entry
- `sum(a, b)` — function call traced with argument values as children
- `items[*].price` — projection traced as single entry with result array value; no per-element children
- Existing `evaluate()` / `run()` behavior unchanged
