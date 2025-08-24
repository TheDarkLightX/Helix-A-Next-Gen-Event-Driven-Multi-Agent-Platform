# Project Codename: Helix

Self-hosted personal event-automation (like IFTTT, but smarter).

See `docs/specification.md` for detailed requirements.

## Quick Start

*(Instructions to be added)*

```bash
# Build the core library
cargo build --workspace

# Run tests
cargo test --workspace
```

## Quint Translator

The `helix-llm` crate includes a small CLI tool that leverages a language model to translate plain English descriptions into [Quint](https://quint-lang.org) specifications.

Set an API key for your preferred provider and run. The translator checks for `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, or a generic `LLM_API_KEY` (with optional `LLM_BASE_URL`). The `--model`, `--temperature`, `--output`, and `--base-url` flags offer additional control:

```bash
# Using OpenRouter
OPENROUTER_API_KEY=your_key_here \
cargo run -p helix-llm --bin quint_translator -- "describe a simple counter that increments"

# Using OpenAI
OPENAI_API_KEY=your_key_here \
cargo run -p helix-llm --bin quint_translator -- "describe a simple counter that increments"
```

The model will respond with a Quint specification based on the provided prompt.

Before issuing a request to the model, the translator performs a lightweight **intent-facet** analysis to highlight potential ambiguities. If aspects like temporal scope, quantifier, or guard keywords (e.g. `if`, `when`, `unless`, `only if`) are missing, clarifying questions are printed to help refine the prompt.

You can customize the model or write output to a file:

```bash
OPENAI_API_KEY=your_key_here \
cargo run -p helix-llm --bin quint_translator --model gpt-4o --temperature 0.1 --output counter.qnt -- "describe a simple counter that increments"
```

You can also read a prompt from a file instead of the command line:

```bash
OPENAI_API_KEY=your_key_here \
cargo run -p helix-llm --bin quint_translator --prompt-file spec.txt
```

If no API key is detected the translator now exits with a non-zero status so automated scripts can surface configuration issues.

## Project Structure

- `/crates`: Core Rust libraries and potentially Rust-based plugins.
  - `/helix-core`: The main runtime logic, agent traits, event definitions.
- `/plugins`: WASM-compiled plugins (TypeScript, Rust, etc.).
- `/ui`: Frontend React application.
- `/ops`: DevOps configurations (Helm, Docker, etc.).
- `/docs`: Project documentation, including the specification.

## Contributing

*(Contribution guidelines TBD)*
