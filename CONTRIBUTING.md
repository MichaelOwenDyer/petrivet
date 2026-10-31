# Contributing to petrivet

Thank you for your interest in contributing! Bug reports, documentation fixes, new analysis
procedures and new examples are all welcome. For anything larger than a small fix, please open
an issue first so we can agree on the approach before you invest the time.

## Development environment

The easiest way to get a working setup is [Nix](https://nixos.org) with flakes enabled.
The flake provides Rust (cargo, rustc, clippy, rustfmt, rust-analyzer), Z3 and pkg-config:

```sh
nix develop
cargo build
cargo test
cargo clippy --all-targets
cargo doc --open
```

If you use [direnv](https://direnv.net), add a `.envrc` containing `use flake`
to enter the shell automatically.

Before opening a pull request, run the same checks as CI:

```sh
nix flake check -L
```

### Without Nix

You need a Rust toolchain (1.88 or newer, e.g. via [rustup](https://rustup.rs)),
the [Z3](https://github.com/Z3Prover/z3) SMT solver, and `pkg-config`, which is how the build finds Z3:

- macOS: `brew install z3 pkg-config`
- Debian/Ubuntu: `apt install libz3-dev pkg-config`

Alternatively, build with the pure-Rust solver OxiZ, which needs no native dependencies at all:

```sh
cargo test --no-default-features --features oxiz,pnml
```

If your change touches the SMT encoding, run the tests with both backends.

## Ground rules

**Soundness comes first.** `petrivet` must never return a wrong verdict.
- If a procedure cannot decide a question, it returns `None` ("unknown"). Never guess, and
  never turn "I could not find X" into "X does not exist" unless a theorem says so.
- A procedure that is only sufficient (or only necessary) for a property may only produce the
  verdict that the theorem actually supports.

**No unsafe code.** The crate is `#![forbid(unsafe_code)]`, and this will not change.

**Lints.** `cargo clippy` runs with `pedantic` and `nursery` enabled. New code should be
warning-free; if you silence a lint, add a short comment explaining why.

**Cite your sources.** Every analysis procedure documents the theorem or algorithm it implements,
with a precise reference (author, year, theorem or section number). Add new sources to the
literature index in `src/literature.rs` and link to it from the doc comment.

## Adding an analysis procedure

1. **Core logic** goes in `src/core`, working on the index-based `DenseNet` representation.
   The public API in `src/api` translates between user-facing types and indices.
2. **Dispatch:** insert the procedure into the property's dispatch, which tries procedures
   in order of increasing cost. Say in the PR why it belongs at that position.
3. **Effort:** procedures that are not polynomial-time must not run when the caller asked for
   efficient procedures only.
4. **Deadline:** any loop whose running time depends on the size of the net or the state space
   must check the deadline regularly, and return `None` when it has passed.
5. **Provenance:** add a variant to the property's method/provenance enum
   (e.g. `LivenessProvenance`), so that callers can see which procedure decided their query.
6. **Cache:** if the property has a cache, record what the procedure learned, including partial
   results such as lower or upper bounds.
7. **Tests:** add unit tests for the cases the theorem distinguishes, including the cases where
   it does *not* apply. For small nets, cross-check the verdict against the reachability graph
   explorer, which is the reference oracle for bounded nets.
8. **Docs:** document the procedure, its complexity, and its reference.

## Pull requests

- Keep PRs focused: one change per PR is easier to review than several.
- Describe what changed and why, and link the issue it addresses.
- `nix flake check` must pass. CI runs it on Linux and macOS for every pull request.
- `cargo clippy --all-targets` should not report new warnings.
- Add a regression test for every bug fix.

## AI-assisted contributions

Contributions written with the help of AI agents are welcome, under the same rules as any
other contribution. You are responsible for every line you submit: read it, understand it,
and run the tests. Please mention in the PR description if an AI agent wrote substantial
parts of the change.

## License

By contributing, you agree that your contributions are licensed under the
GNU LGPLv3, the license of this project.
