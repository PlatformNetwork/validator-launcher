# Validator Auto-Updater

A Rust-based service that automatically monitors and updates validator VMs in dstack VMM when compose configuration changes.

## Overview

The Validator Auto-Updater continuously polls the Platform Network API for validator VM configuration updates. When changes are detected (via compose hash comparison), it gracefully updates the running VM with the new configuration.

## Features

- **Automatic polling**: Checks for configuration updates every 5 seconds
- **Change detection**: Compares compose content hash including image version
- **Graceful updates**: Stops existing VM with 60s timeout before recreation
- **Environment management**: Reads and encrypts environment variables
- **Encryption**: Uses X25519 key exchange + AES-256-GCM for secure env var transmission
- **Auto-configuration**: Detects and validates required environment variables
- **Idempotent**: Only updates when configuration actually changes

## Configuration

### VMM URL

Set the VMM URL via environment variable:

```bash
export VMM_URL="http://localhost:16850"
```

Default: `http://localhost:16850`

### Platform Configuration

Create `/etc/platform-validator/config.json` with your environment variables:

```json
{
  "dstack_vmm_url": "http://10.0.2.2:16850/",
  "validator_hotkey_passphrase": "your-secret-passphrase-here",
  "env": {
    "CUSTOM_VAR_1": "value1",
    "API_KEY": "your-api-key",
    "NODE_ENV": "production"
  }
}
```

**Fields:**
- `dstack_vmm_url` (optional): VMM URL accessible from the VM (default: `http://10.0.2.2:16850/`)
- `env` (optional): Map of environment variables to inject into the VM (including `HOTKEY_PASSPHRASE` and other API-required keys)

**Required Environment Variables:**

The Platform API specifies **required environment variable keys** (e.g., `HOTKEY_PASSPHRASE`, `DSTACK_VMM_URL`, `VALIDATOR_BASE_URL`). You must provide the **values** for these keys using the `config` command:

1. The API sends only the **keys** that are required
2. You set the **values** using `validator-auto-updater config set-env <key> <value>`
3. The auto-updater merges API keys with your local values
4. VM creation is blocked if required keys are missing values

See `platform-validator-config.example.json` for a full example.

## Usage

### Start the Auto-Updater Service

```bash
cd /home/ubuntu/validator-auto-updater
sudo cargo run --release -- run
```

Or with custom VMM URL:

```bash
sudo VMM_URL="http://localhost:16850" cargo run --release -- run
```

### Configuration Management (CLI)

Manage platform configuration using CLI commands:

```bash
cd /home/ubuntu/validator-auto-updater
sudo cargo run --release -- config <command>
```

**Available Commands:**

- `show` - Show current configuration
- `set-vmm-url <url>` - Set VMM URL
- `set-passphrase <passphrase>` - Set validator hotkey passphrase
- `set-env <key> <value>` - Set an environment variable
- `remove-env <key>` - Remove an environment variable
- `list-env` - List all environment variables
- `get-env <key>` - Get a specific environment variable value

**Examples:**

```bash
# Show current config
sudo cargo run --release -- config show

# Set VMM URL
sudo cargo run --release -- config set-vmm-url "http://10.0.2.2:16850/"

# Set environment variables (for API-required keys)
# Example: Set HOTKEY_PASSPHRASE (required by API)
sudo cargo run --release -- config set-env HOTKEY_PASSPHRASE "your-12-word-mnemonic-passphrase"
sudo cargo run --release -- config set-env VALIDATOR_BASE_URL "http://10.0.2.2:18080"
sudo cargo run --release -- config set-env CUSTOM_VAR "custom-value"

# List all environment variables
sudo cargo run --release -- config list-env

# Remove an environment variable
sudo cargo run --release -- config remove-env CUSTOM_VAR
```

## Logging

Set log level via `RUST_LOG` environment variable:

```bash
sudo RUST_LOG=debug cargo run --release -- run
```

## Development

### Building

```bash
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy -- -D warnings
```

## Project Structure

```
validator-auto-updater/
├── src/
│   ├── main.rs          # Main application logic
│   └── config_tui.rs    # Configuration CLI commands
├── ansible/             # Ansible playbooks for deployment
│   ├── playbook.yml     # Main deployment playbook
│   ├── install-updater.sh  # Auto-updater installation script
│   ├── requirements.yml # Ansible collection requirements
│   └── README.md        # Ansible documentation
├── Cargo.toml           # Rust dependencies
├── LICENSE              # Apache 2.0 license
├── CONTRIBUTING.md      # Contribution guidelines
└── README.md            # This file
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

