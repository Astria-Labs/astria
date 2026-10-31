# Astria

Astria is an experimental, local-first, semantic-aware version control system written in Rust. The CLI binary is named `ast` and combines content-addressed storage, staging and commits, structural Rust analysis, local vector search, background indexing, and QUIC synchronization.

> Astria is under active development. Repository formats, network protocol details, and command behavior may still change. Test with non-critical repositories before relying on it for important data.

## Why Astria

Traditional version control understands files and lines. Astria also builds a semantic view of Rust code, allowing it to find symbols, compare structural changes, and search code by meaning while keeping repository data local by default.

Current benefits include:

- BLAKE3-addressed, Zstd-compressed blobs, trees, and commits.
- Deterministic tree ordering for stable object generation.
- Explicit staging before commits.
- AST-aware symbol queries and structural diffs for Rust.
- Local embeddings with FastEmbed and vector search with LanceDB.
- Background semantic indexing after commits.
- Full, lazy, and ondemand synchronization modes over QUIC.
- Scoped server access tokens with Argon2-hashed credentials.
- No Python runtime or external vector database required for local semantic search.

## Current scope and limitations

- AST parsing and semantic indexing currently support Rust source files only.
- `query --ai` means local natural-language vector search; it does not currently call Claude, Codex, OpenAI, or another external LLM.
- `merge` currently performs a fast-forward pointer update and materializes Rust files. It is not yet a general three-way merge engine.
- `checkout` updates HEAD or the selected branch reference; it does not currently perform a complete working-tree checkout.
- The QUIC client accepts the development server's self-signed certificate without verification. Do not treat the current transport setup as production hardened.
- Local-path network modes are development simulations; real synchronization uses `quic://HOST:PORT`.

## Installation

### Build from source

Requirements:

- Rust toolchain with Cargo.
- A platform supported by the native dependencies used by LanceDB and FastEmbed.

```sh
git clone <repository-url> astria
cd astria
cargo build --release
```

The executable is created at `target/release/ast`. Copy it to a directory on your `PATH` if desired:

```sh
install -m 755 target/release/ast /usr/local/bin/ast
```

During development, replace `ast` in the examples below with `cargo run --`:

```sh
cargo run -- status
```

## Quick start

```sh
mkdir demo
cd demo
ast init
ast config user.name "Ada Lovelace"
ast config user.email "ada@example.com"

# Create or copy files into the repository.
ast add .
ast status
ast commit -m "Initial commit"
ast log
```

The first semantic operation may download the local FastEmbed model and place model data in `.fastembed_cache/`. A successful commit queues semantic indexing and starts the background worker automatically.

## CLI reference

Run `ast --help` or `ast <command> --help` for the Clap-generated reference.

### Repository lifecycle

#### `ast init`

Initializes the current directory and creates `.astria/`, `.astriaignore`, object/vector directories, configuration files, and `HEAD`.

```sh
ast init
```

#### `ast add <paths...>`

Stages one or more files or directories. Use `.` to recursively stage eligible files from the repository root.

```sh
ast add src/main.rs
ast add src tests Cargo.toml
ast add .
```

#### `ast reset <path>`

Removes one path from the staging index without changing the working file.

```sh
ast reset src/main.rs
```

#### `ast status` (`ast st`)

Shows the current branch or detached HEAD plus staged, modified, and untracked Rust files.

```sh
ast status
ast st
```

#### `ast commit [-m|--message <message>]` (`ast ci`)

Creates blobs for staged files, a deterministically ordered tree, and a commit. It updates HEAD, clears the staging index, queues semantic indexing, and starts the worker. If omitted, the message defaults to `Auto commit from workspace scan`.

```sh
ast commit -m "Add authentication"
ast ci --message "Refactor parser"
```

Author resolution uses this precedence:

1. `ASTRIA_AUTHOR_NAME` and `ASTRIA_AUTHOR_EMAIL`.
2. `GIT_AUTHOR_NAME` and `GIT_AUTHOR_EMAIL`.
3. Local `.astria/config`.
4. Global `~/.astriaconfig` or `~/.config/astria/config`.
5. `Unknown <unknown@astria.org>`.

#### `ast log`

Walks the first-parent commit history from HEAD.

```sh
ast log
```

#### `ast show <hash>`

Displays a commit, tree, or blob object. A unique abbreviated hash is accepted.

```sh
ast show 2cf30ab
ast show <full-object-hash>
```

### Branches and history

#### `ast branch [name]` (`ast br`)

Lists branches when no name is supplied, or creates a branch at the current HEAD.

```sh
ast branch
ast br feature/search
```

#### `ast checkout <branch-or-commit>` (`ast co`)

Moves HEAD to an existing branch or exact commit hash. Current behavior updates repository references but does not fully restore all working-tree files.

```sh
ast checkout main
ast co <full-commit-hash>
```

#### `ast merge <branch-or-commit>`

Fast-forwards the current branch or detached HEAD to the target, then materializes Rust files from the target commit.

```sh
ast merge feature/search
ast merge <full-commit-hash>
```

### Structural diff and code queries

#### `ast diff [target] [target]`

Performs an AST structural diff over Rust symbols and reports added, removed, modified, and renamed functions, structs, enums, and traits.

```sh
# HEAD versus workspace
ast diff

# One working file versus HEAD
ast diff src/main.rs

# A commit versus workspace
ast diff <commit-a>

# Two commits
ast diff <commit-a> <commit-b>
```

#### `ast query [search] [options]`

Searches Rust symbols in the workspace or a commit.

```sh
# List matching symbols by name
ast query authentication

# Filter symbol categories
ast query --func
ast query --fn parse
ast query --struct
ast query --interface

# Combine filters
ast query parse --func --pub-only
ast query parse --func -p

# Restrict to a file or commit
ast query parse --file src/parser.rs
ast query parse -f src/parser.rs
ast query parse --commit <commit-hash>
ast query parse --ci <commit-hash>
```

Available query options:

| Option | Meaning |
| --- | --- |
| `--func`, `--fn` | Functions and methods |
| `--struct` | Structs, enums, classes, or data types |
| `--interface` | Traits or interfaces |
| `--pub-only`, `-p` | Public symbols only |
| `--file`, `-f <path>` | Restrict results to a path |
| `--commit`, `--ci <hash>` | Query a committed snapshot |
| `--ai <text>` | Natural-language semantic search |

For full and lazy repositories, `--ai` searches the local LanceDB index:

```sh
ast query --ai "where are access tokens validated?"
```

For an ondemand clone, the same command queries the server, downloads the matched blob objects, and materializes only the relevant files.

### Configuration

#### `ast config [--global] <key> <value>`

Sets author, checkout, or remote configuration.

```sh
ast config user.name "Ada Lovelace"
ast config user.email "ada@example.com"
ast config --global user.name "Ada Lovelace"
ast config --global user.email "ada@example.com"

ast config checkout.mode full
ast config checkout.mode lazy
ast config checkout.mode ondemand

ast config remote.origin.url "quic://127.0.0.1:4433"
ast config remote.origin.repository "demo"
ast config remote.origin.auth_token "<token>"
```

Repository settings and transport settings are intentionally separate:

- `.astria/config` contains `[user]` and `[checkout]` settings.
- `.astria/remotes` contains remote URL, repository name, and authentication token.

Do not commit either credentials or runtime `.astria/` data.

### Remotes and synchronization

#### `ast remote add <name> <url>`

```sh
ast remote add origin quic://127.0.0.1:4433
```

This sets the URL. Configure the logical repository name and token separately with `ast config remote.<name>...`.

#### `ast remote set-url <name> <url>`

```sh
ast remote set-url origin quic://10.0.0.20:4433
```

#### `ast remote list` (`ast remote ls`)

```sh
ast remote list
ast remote ls
```

#### `ast remote remove <name>` (`ast remote rm`)

```sh
ast remote remove origin
ast remote rm backup
```

#### `ast fetch [remote]`

Downloads missing objects without changing branch state. The remote defaults to `origin`.

```sh
ast fetch
ast fetch backup
```

#### `ast pull [remote]`

Synchronizes missing objects from the selected remote using the configured checkout mode. The remote defaults to `origin`.

```sh
ast pull
ast pull origin
```

#### `ast push [remote]`

Uploads local objects and the current commit to a remote requiring `write` or `admin` permission. The remote defaults to `origin`.

```sh
ast push
ast push origin
```

#### `ast clone [--lazy|--ondemand] <url> [target-dir]`

Clones over QUIC. Set `ASTRIA_AUTH_TOKEN` before running the command. The target directory name is also used as the logical repository name in the generated origin configuration.

```sh
ASTRIA_AUTH_TOKEN="<token>" ast clone quic://127.0.0.1:4433 demo
ASTRIA_AUTH_TOKEN="<token>" ast clone --lazy quic://127.0.0.1:4433 demo
ASTRIA_AUTH_TOKEN="<token>" ast clone --ondemand quic://127.0.0.1:4433 demo
```

`--lazy` and `--ondemand` are mutually exclusive.

Checkout modes:

| Mode | Initial transfer | Materialization behavior |
| --- | --- | --- |
| `full` | Commits, trees, and blobs | All available objects are transferred |
| `lazy` | Commits, trees, and critical blobs | Remaining blobs enter the virtual index and are hydrated by the worker |
| `ondemand` | Commits and trees | Matching blobs are fetched when `query --ai` expresses intent |

Critical lazy files include Rust source/tests, Cargo manifests and lockfiles, `.cargo/*`, toolchain files, `AGENTS.md`, README files, and `.astriaignore`.

### Server and authentication

The server operates on the Astria repository in its current working directory and stores token records in `astria_server.db` there.

#### `ast server [--addr <address>] start`

Starts the QUIC server. The default address is `127.0.0.1:4433`; omitting `start` also starts the server.

```sh
cd /path/to/server-repository
ast server start
ast server --addr 0.0.0.0:4433 start
```

#### `ast server token create`

Creates a scoped personal access token. Permissions are `read`, `write`, or `admin`; expiration accepts values such as `30d`, `90d`, or `never`.

```sh
ast server token create --user alice demo read --expires 30d
ast server token create --user ci demo write --expires 90d
ast server token create --user admin demo admin --expires never
```

The raw token is shown only when created. Save it securely. A `write` token also satisfies read operations; `admin` satisfies both read and write operations.

### Background worker

#### `ast worker`

Runs the repository worker in the foreground. It processes semantic-index and lazy-hydration tasks from `.astria/queue`, prevents duplicate workers with `.astria/worker.lock`, permits up to four concurrent hydrations, and exits after approximately 30 seconds without work.

```sh
ast worker
```

Normal commits and lazy synchronization start the worker automatically, so this command is mainly useful for debugging or manually processing a pending queue.

## Architecture and layers

Astria keeps command orchestration separate from storage, semantic analysis, background work, and transport:

```text
CLI (`ast` / Clap)
    -> command orchestration
        -> repository and configuration services
        -> object model and content-addressed storage
        -> AST and semantic search
        -> task queue and background worker
        -> virtual checkout index
        -> QUIC client/server and authentication
```

### 1. CLI and command layer

`src/main.rs` installs global paths, initializes the Rustls ring provider, parses Clap arguments, and dispatches through `src/cli/commands/mod.rs`. Each module under `src/cli/commands/` coordinates one user-facing operation.

Benefit: command parsing and terminal behavior remain separate from reusable repository logic, making commands easier to test and evolve.

### 2. Configuration layer

`src/config/` separates user/repository settings from transport remotes:

- `user.rs` handles author identity and checkout mode.
- `remotes.rs` handles URL, logical repository name, and token, including migration from the legacy Bincode remote map.

Benefit: local behavior can evolve without coupling it to network credentials or remote layout.

### 3. Object and storage layer

`src/objects/` defines blobs, trees, commits, object types, and the Astria object envelope. Objects are serialized with Bincode, compressed with Zstd, and addressed by a BLAKE3 hash under `.astria/objects`. Tree entries are sorted before persistence.

Benefit: immutable, integrity-checkable objects support deduplication, deterministic trees, and efficient transfer by hash.

### 4. Staging and history layer

The active staging format is a Bincode-encoded map at `.astria/index`. Commits consume staged paths, persist their blobs and tree, record parent hashes, update HEAD/branch references, and enqueue semantic work.

Benefit: repository writes stay fast because semantic indexing is decoupled from the critical commit path.

### 5. AST and semantic layer

`src/parser.rs` uses tree-sitter Rust to extract functions, structs, enums, and traits. `src/vector.rs` uses FastEmbed's `AllMiniLML6V2` model and LanceDB/Arrow records to store and search semantic chunks.

Benefit: Astria supports both exact structural navigation and natural-language retrieval without sending source code to a hosted AI provider.

### 6. Queue and worker layer

`src/queue.rs` persists JSON tasks under `.astria/queue`. `src/worker.rs` processes semantic-index tasks and lazy blob hydration outside the initiating CLI command.

Benefit: commits and network commands can return without waiting for expensive embedding or hydration work, while persisted tasks survive process boundaries.

### 7. Virtual checkout layer

`src/virtual_index.rs` records paths whose blobs exist remotely but are not yet materialized. Lazy mode hydrates these files in the background; ondemand mode waits for a semantic query before fetching relevant blobs.

Benefit: large repositories can minimize initial transfer and disk usage while retaining a repository-wide logical view.

### 8. Network and authentication layer

`src/network/` implements Bincode-framed messages over QUIC with Zstd transport compression and BLAKE3 integrity verification. The protocol supports full/lazy/ondemand sync, push, object requests, and semantic queries. `src/server/db.rs` stores Argon2-hashed tokens in SQLite and enforces `read`, `write`, and `admin` permissions.

Benefit: object hashes make incremental transfer straightforward, while sync modes let clients trade eager availability for bandwidth and disk savings.

## Repository data

Runtime state lives under `.astria/`:

```text
.astria/
├── HEAD              # branch reference or detached commit
├── config            # user and checkout settings
├── remotes           # remote URL/repository/token settings
├── index             # staging index, when present
├── objects/          # compressed content-addressed objects
├── vectors/          # LanceDB semantic index
├── queue/            # pending worker tasks
├── virtual_index     # lazy/ondemand paths, when present
└── worker.lock       # active worker PID, when present
```

Generated local data such as `.astria/`, `.fastembed_cache/`, `target/`, `test_clone/`, and `astria_server.db` should not be committed.

## Development

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features
```

Useful development flows:

```sh
cargo run -- init
cargo run -- add src/main.rs
cargo run -- commit -m "Test commit"
cargo run -- query --ai "where does command dispatch happen?"
```

Network, authentication, lazy, ondemand, and protocol changes should also be validated with a local QUIC server and a real token.

## Technology

- Rust 2021 and Tokio
- Clap
- BLAKE3, Bincode, Serde, and Zstd
- tree-sitter and tree-sitter-rust
- FastEmbed, LanceDB, and Apache Arrow
- Quinn, Rustls, and rcgen
- SQLite via rusqlite and Argon2

## Roadmap

Planned features and cross-cutting improvements are tracked in [`TODO.md`](TODO.md). It currently covers extensible AI providers, unified error handling with `AstriaError`, and a scriptable UI layer powered by Rhai.

## License

Apache-2.0. See the package metadata and repository license file for details.
