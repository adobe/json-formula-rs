# json-formula-rs

Native Rust implementation of the [Adobe json-formula specification](https://opensource.adobe.com/json-formula/).
The library parses and evaluates json-formula expressions directly against `serde_json::Value`
data, matching the behavior of the official reference implementation.

## Usage

```rust
use json_formula_rs::JsonFormula;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = JsonFormula::new();
    let data = json!({ "items": [{ "price": 3.5 }, { "price": 2.0 }] });
    let result = engine.search("sum(items[*].price)", &data, None, Some("en-US"))?;
    println!("Result: {}", result);

    Ok(())
}
```

## Interactive Testbed

A desktop GUI testbed is available in the `testbed/` directory. Built with [Tauri](https://tauri.app/), it provides a four-panel interface for interactively exploring expressions:

- **Input JSON** — paste or type your JSON data (syntax highlighted)
- **Expression** — enter a json-formula expression
- **Result** — evaluated output (syntax highlighted)
- **Debug Info** — step-by-step evaluation trace

Press **Enter** or **Tab** to trigger evaluation. Tab also advances focus between fields.

### Running the testbed

```bash
cd testbed
cargo tauri dev
```

> Requires the [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites) and a Rust toolchain.

## Testing

The official json-formula JSON test fixtures are stored under `tests/fixtures` and are executed
via the Rust test harness:

```bash
cargo test
```

### Official Suite

To run just the official json-formula fixtures:

```bash
cargo test --test official_suite
```

Note: precedence expectations in `tests/fixtures/precedence.json` are validated by the parser test
helpers; failures will surface as part of the test run.

## Development

```bash
# Run the full test suite
cargo test -- --nocapture

# Run just the official fixtures
cargo test --test official_suite
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations.

## License

This project is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the full text. It includes test fixtures derived from the Adobe json-formula project; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for details.
