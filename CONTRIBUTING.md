# Contributing to Validator Auto-Updater

Thank you for your interest in contributing to Validator Auto-Updater!

## Development Setup

1. Get the source code:
```bash
cd validator-auto-updater
```

2. Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

3. Build the project:
```bash
cargo build --release
```

## Code Style

- Follow Rust standard formatting: `cargo fmt`
- Run clippy before submitting: `cargo clippy -- -D warnings`
- Ensure all tests pass: `cargo test`

## Code Changes

- Use clear, descriptive commit messages if using version control
- Reference issue numbers when applicable
- Ensure all tests pass before submitting
- Update documentation if needed

## Testing

Run tests with:
```bash
cargo test
```

Run with verbose output:
```bash
cargo test -- --nocapture
```

## Questions?

Feel free to open an issue for questions or discussions.

