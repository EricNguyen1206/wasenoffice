# Gate commands — THE one home (review #6 setup follow-up, 05.10): verify.sh
# AND dev-check.sh both source this file, so a "local check" can never drift
# from what the gate runs (04.10 incident: a fmt check without --all passed
# vacuously twice; a `cargo check -p <crate>` missed a shell-crate
# initializer until gate 15 tripped). Strings only — no logic; `eval` at the
# call site. Sourced by both zsh (verify.sh) and bash (dev-check.sh).
GATE_CARGO_CHECK='(cd src-tauri && cargo check --workspace --quiet)'
GATE_FMT='(cd src-tauri && cargo fmt --check --all)'
GATE_CLIPPY='(cd src-tauri && cargo clippy --workspace --quiet -- -D warnings)'
GATE_BUILD='pnpm build --logLevel error'
GATE_LAYOUT='./harness/scripts/test-layout.sh'
# T-049 gate 16: multi-format save round-trip via real x2t (no app boot).
GATE_SAVE_MULTIFORMAT='(cd src-tauri && cargo test --quiet save_multiformat_)'
