# Astria 🌪️

A **local-first, semantic-aware Version Control System (VCS)** written in Rust.

Astria is designed from the ground up to combine low-level content-addressable storage (CAS) with semantic awareness, leveraging modern cryptographics, fast compression, deterministic object generation, and AI-powered vector querying.

---

## ✨ Features

- **Content-Addressable Storage (CAS)**: Powered by fast **BLAKE3** hashing for fast, collisions-resistant object identifiers.
- **Zstd Compression**: Efficient commit object, tree, and blob payload compression.
- **Explicit Staging Area**: Traditional two-stage architecture using an index (`.astria/index`) for precise change tracking before committing.
- **Deterministic Tree Generation**: Lexicographically sorted tree entries ensuring identical commit hashes across environments.
- **Ignore Rules (`.astriaignore`)**: Built-in dynamic pattern matching for ignored files, build artifacts (`target/`), and logs.
- **Semantic Vector Querying (`--ai`)**: High-performance semantic diffing and embedding-based search using pgvector/vector embeddings.

---

## 🛠️ Tech Stack

- **Language**: Rust (Edition 2021)
- **Hashing**: `blake3`
- **Compression**: `zstd`
- **Serialization**: `bincode` / `serde`
- **CLI**: `clap`

---

## 🚀 Getting Started

### Prerequisites

- [Rust Toolchain](https://www.rust-lang.org/tools/install) (Cargo & rustc)

### Installation

Clone the repository and build the binary:

```bash
git clone [https://github.com/YOUR_GITHUB_USERNAME/astria.git](https://github.com/YOUR_GITHUB_USERNAME/astria.git)
cd astria
cargo build --release