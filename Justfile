set shell := ["sh", "-eu", "-c"]

default: verify
verify:
    cargo fmt --check
    cargo test
    cargo clippy --all-targets --all-features -- -D warnings
    git diff --check

check: verify

demo:
    cargo run --bin trust-boundaries-demo

services-up:
    docker compose up -d postgres

services-down:
    docker compose down
