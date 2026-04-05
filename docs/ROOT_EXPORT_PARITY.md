# Root Export Parity

Python root-export parity is tracked against
`reference/openai-agents-python/src/agents/__init__.py::__all__`.

Every Python root export falls into exactly one bucket:

- surfaced directly from [`crates/openai-agents/src/lib.rs`](../crates/openai-agents/src/lib.rs)
- aliased to a Rust-first equivalent listed below
- intentionally omitted because the Python name is only a typing helper or TypedDict helper

All Python root exports not listed below are surfaced directly from the Rust facade.

## Aliased

- `AgentsException` -> `AgentsError`
- `OpenAIResponsesWSModel` -> `OpenAIResponsesWsModel`
- `SessionABC` -> `Session`
- `__version__` -> `VERSION`

## Intentional Rust-First Omissions

- `TContext`: Python type variable helper. Rust uses concrete generic parameters on `RunContextWrapper`.
- `ToolOutputFileContentDict`: Python `TypedDict` helper. Rust uses `ToolOutputFileContent`.
- `ToolOutputImageDict`: Python `TypedDict` helper. Rust uses `ToolOutputImage`.
- `ToolOutputTextDict`: Python `TypedDict` helper. Rust uses `ToolOutputText`.
