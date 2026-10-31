# Astria Roadmap

This document tracks planned features and cross-cutting improvements for Astria. Each feature should remain unchecked until its implementation and tests are complete.

## Feature 1: Extensible AI Providers

- [ ] Add an `AiProvider` abstraction independent from semantic indexing.
- [ ] Implement an OpenAI-compatible HTTP `ApiProvider` as the next feature.
- [ ] Keep the current FastEmbed + LanceDB search as the default local retrieval layer.
- [ ] Feed semantic matches and their source code to the selected provider as grounded context.
- [ ] Configure endpoint, model, and API-key environment-variable name in `.astria/config`.
- [ ] Keep API credentials out of repository files and logs.
- [ ] Preserve the current `query --ai` behavior when no external provider is configured.
- [ ] Add clear timeout, authentication, rate-limit, malformed-response, and unavailable-provider errors.
- [ ] Add unit tests with a mock HTTP server; tests must not call a real paid API.
- [ ] Support AI answers after ondemand remote hydration.
- [ ] Add a native Anthropic/Claude request adapter.
- [ ] Add a JSON stdin/stdout `CommandProvider` for custom local integrations.
- [ ] Add configurable context limits and token-aware truncation.
- [ ] Add streaming responses and cancellation.
- [ ] Document provider examples for OpenAI, compatible local servers, Claude, and custom tools.

### Proposed architecture

```text
User query
    -> local semantic search (FastEmbed + LanceDB)
    -> relevant source context
    -> AiProvider
        -> ApiProvider (OpenAI-compatible HTTP API)
        -> future AnthropicProvider
        -> future CommandProvider
```

### Proposed configuration

```toml
[ai]
provider = "api"
endpoint = "https://api.example.com/v1/chat/completions"
model = "model-name"
api_key_env = "ASTRIA_AI_API_KEY"
```

The first implementation should target the OpenAI-compatible chat-completions request/response format. Provider-specific formats, including Anthropic's API, should use separate adapters rather than conditional request construction inside `ApiProvider`.

## Feature 2: Unified `AstriaError`

Introduce a project-wide error type to normalize internal errors and user-facing CLI messages.

- [ ] Add a central `AstriaError` enum and an `AstriaResult<T>` alias.
- [ ] Define variants for repository, configuration, object storage, index, parsing, semantic search, network, authentication, protocol, and I/O failures.
- [ ] Preserve the original error as a source where useful instead of converting errors prematurely to strings.
- [ ] Implement consistent `Display` messages in clear English.
- [ ] Centralize CLI error rendering so command modules return errors instead of printing ad hoc error messages.
- [ ] Distinguish user-facing context from low-level diagnostic details.
- [ ] Normalize process exit codes for invalid input, missing repository/data, authentication, network, and internal failures.
- [ ] Add `From` conversions only when the source error maps unambiguously to one `AstriaError` variant.
- [ ] Replace `Box<dyn Error>`, repeated `std::io::Error::new`, and string errors incrementally across modules.
- [ ] Remove production-path `unwrap`/`expect` calls where the failure is recoverable.
- [ ] Ensure sensitive values such as tokens are never included in error messages or debug output.
- [ ] Add tests for error conversion, source chaining, display text, CLI rendering, and exit codes.
- [ ] Document conventions for adding new variants without exposing unstable dependency-specific errors through the public API.

### Proposed shape

```rust
pub type AstriaResult<T> = Result<T, AstriaError>;

pub enum AstriaError {
    Repository(/* context and source */),
    Config(/* context and source */),
    Storage(/* context and source */),
    SemanticSearch(/* context and source */),
    Network(/* context and source */),
    Authentication(/* context */),
    InvalidInput(/* context */),
    Io(/* source */),
}
```

The exact fields and variants should be based on existing failure paths before implementation. Avoid a generic string-only catch-all for errors that callers need to classify or handle.

## Feature 3: Scriptable UI Layer with Rhai

Add an optional interactive interface that improves the CLI experience and uses Rhai as its customization engine. Rhai should control presentation and approved workflows; the actual GUI/TUI renderer must remain a separate Rust component because Rhai is a scripting engine, not a visual toolkit.

- [ ] Define the initial interface target: terminal UI, desktop GUI, or a shared presentation model supporting both.
- [ ] Keep every existing CLI command usable without the interactive interface or Rhai scripts.
- [ ] Introduce a Rust-owned UI model for repository status, history, branches, diffs, search results, progress, and errors.
- [ ] Embed Rhai as an optional scripting engine for themes, layouts, formatting, key bindings, aliases, and command workflows.
- [ ] Expose a small, versioned Astria scripting API instead of making internal Rust types directly available to scripts.
- [ ] Support repository-local scripts and global user scripts with an explicit precedence order.
- [ ] Define conventional script locations such as `.astria/ui.rhai` and the global Astria configuration directory.
- [ ] Require explicit opt-in before executing repository-provided scripts from an untrusted checkout.
- [ ] Sandbox scripts by default: no unrestricted filesystem, process, environment, credential, or network access.
- [ ] Apply execution limits for operations, call depth, data size, and runtime to prevent scripts from freezing the interface.
- [ ] Make read-only repository APIs the default and gate mutating operations behind explicit capabilities and confirmation.
- [ ] Route script failures through `AstriaError` and provide actionable file, line, and function diagnostics.
- [ ] Ensure scripts cannot read or print authentication tokens and other secrets.
- [ ] Add a safe-mode option that starts the interface without loading custom scripts.
- [ ] Add example scripts for themes, status formatting, custom shortcuts, and semantic-search workflows.
- [ ] Test API compatibility, sandbox boundaries, resource limits, configuration precedence, and fallback to the standard CLI.

### Proposed architecture

```text
Astria core services
    -> stable UI data/actions API
    -> Rust GUI/TUI renderer
    -> embedded Rhai engine
        -> themes and formatting
        -> key bindings and aliases
        -> approved command workflows
```

The renderer must remain responsible for drawing, input handling, accessibility, and lifecycle management. Rhai scripts should customize behavior through the stable UI API without replacing core repository, storage, network, or authentication logic.

## Feature 4: Parser Factory and Multi-language Analysis

Replace the Rust-specific parser entry point with a factory-based parser layer that selects a language implementation from file metadata while returning a normalized Astria representation. Rust remains the first and reference integration, followed by Kof, Python, PHP, Java, and JavaScript.

- [ ] Define a language-neutral `LanguageParser` trait for symbol extraction and semantic chunk generation.
- [ ] Define normalized symbol kinds and metadata shared by all languages, including name, qualified name, visibility, file, byte range, line range, signature, and content hash.
- [ ] Introduce a `ParserFactory` or parser registry that resolves implementations by language identifier and file extension.
- [ ] Return an explicit unsupported-language result instead of silently treating unknown files as Rust.
- [ ] Keep parser construction independent from CLI commands, storage, vector indexing, and network code.
- [ ] Migrate the existing tree-sitter Rust implementation into `RustParser` without changing current Rust query, diff, or indexing behavior.
- [ ] Use Rust as the conformance reference for parser contracts and shared test fixtures.
- [ ] Add Kof support based on [`Kof4j`](https://github.com/KofLang/Kof4j) after validating its language specification, supported file extensions, grammar availability, and integration boundary.
- [ ] Add Python support.
- [ ] Add PHP support.
- [ ] Add Java support.
- [ ] Add JavaScript support, with an explicit decision on whether TypeScript/JSX require separate parser identifiers.
- [ ] Prefer maintained tree-sitter grammars when available, while keeping grammar-specific node names inside each parser adapter.
- [ ] Allow more than one parser implementation to coexist without global mutable parser state.
- [ ] Reuse parser instances safely where beneficial, without sharing non-thread-safe parser state across workers.
- [ ] Update workspace scanning, semantic indexing, `query`, `diff`, worker, lazy, and ondemand flows to use the factory.
- [ ] Include the language identifier in semantic chunks and vector records to support filtering and prevent cross-language ambiguity.
- [ ] Define behavior for mixed-language repositories and files with ambiguous or missing extensions.
- [ ] Route parser selection and grammar failures through `AstriaError`.
- [ ] Add contract tests that every parser must pass, plus language-specific fixtures for declarations, visibility, nesting, methods, interfaces/traits, enums, and malformed source.
- [ ] Document how contributors can add a language parser without changing command-layer code.

### Planned integration order

1. Rust — migrate and stabilize the existing implementation.
2. Kof — integrate against the verified Kof4j language contract.
3. Python.
4. PHP.
5. Java.
6. JavaScript.

### Proposed architecture

```text
Source file
    -> language detection
    -> ParserFactory / parser registry
        -> RustParser
        -> KofParser
        -> PythonParser
        -> PhpParser
        -> JavaParser
        -> JavaScriptParser
    -> normalized symbols and semantic chunks
        -> query and structural diff
        -> vector indexing
        -> network semantic results
```

The factory should select implementations, not contain parsing rules. Each adapter owns its grammar-specific traversal and converts results into the shared Astria model, keeping downstream features independent from individual language grammars.
