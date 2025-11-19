# Validator Launcher

A Rust-based service that automatically monitors and updates validator VMs in dstack VMM when compose configuration changes.

## Overview

The Validator Launcher continuously polls the Platform Network API for validator VM configuration updates. When changes are detected (via compose hash comparison), it gracefully updates the running VM with the new configuration.

## Features

- **Automatic polling**: Checks for configuration updates every 5 seconds
- **Change detection**: Compares compose content hash including image version
- **Graceful updates**: Stops existing VM with 60s timeout before recreation
- **Environment management**: Reads and encrypts environment variables
- **Encryption**: Uses X25519 key exchange + AES-256-GCM for secure env var transmission
- **Auto-configuration**: Detects and validates required environment variables
- **Idempotent**: Only updates when configuration actually changes
- **CLI alias**: Install as `platform` command for easy access

## Quick Start

### Installation

1. **Clone the repository:**
```bash
git clone https://github.com/PlatformNetwork/validator-launcher.git
cd validator-launcher
```

2. **Run the installation script:**
```bash
sudo ./install.sh
```

This will:
- Install Ansible if not already present
- Install required Ansible collections
- Install build dependencies (Rust, build tools, etc.)
- Build the release binary
- Install it to `/usr/local/bin/validator-launcher`
- Create a `platform` CLI alias
- Install dstack services startup scripts (KMS, Gateway, VMM)
- Create a systemd service that automatically starts dstack services before validator-launcher

**Note:** The installation script uses Ansible for a consistent and idempotent installation process.

**Dstack Services:** The validator-launcher service automatically starts the following dstack services in order before launching:
- `dstack-kms` (KMS service)
- `dstack-gateway` (Gateway service)  
- `dstack-vmm` (VMM service)

These services are started from `/home/ubuntu/meta-dstack/build/` directory. Logs are written to `/var/log/dstack/`.

### Configuration

1. **Set required environment variables:**
```bash
# Set validator hotkey passphrase (required)
sudo platform config set-env HOTKEY_PASSPHRASE "your-12-word-mnemonic-passphrase"

# Set validator base URL (required)
sudo platform config set-env VALIDATOR_BASE_URL "http://10.0.2.2:18080"

# Set VMM URL (optional, defaults to http://10.0.2.2:10300/)
sudo platform config set-vmm-url "http://10.0.2.2:10300/"

# List all configured environment variables
sudo platform config list-env
```

2. **Verify configuration:**
```bash
sudo platform config show
```

### Starting the Service

1. **Ensure dstack services are available:**
```bash
# Verify dstack binaries exist
ls -la /home/ubuntu/meta-dstack/build/dstack-{kms,gateway,vmm}

# Verify configuration files exist
ls -la /home/ubuntu/meta-dstack/build/{kms,gateway,vmm}.toml
```

2. **Enable and start the service:**
```bash
sudo systemctl enable validator-launcher
sudo systemctl start validator-launcher
```

The service will automatically:
- Start `dstack-kms` in background
- Start `dstack-gateway` in background (with sudo)
- Start `dstack-vmm` in background
- Then start `validator-launcher`

3. **Check service status:**
```bash
sudo systemctl status validator-launcher
```

4. **View logs:**
```bash
# Follow validator-launcher logs in real-time
sudo journalctl -u validator-launcher -f

# View last 100 lines
sudo journalctl -u validator-launcher -n 100

# View dstack services logs
sudo tail -f /var/log/dstack/*.log

# Check if dstack services are running
ps aux | grep dstack-
```

### Restarting the Service

If you need to restart the service after configuration changes:

```bash
# Restart the service
sudo systemctl restart validator-launcher

# Check status
sudo systemctl status validator-launcher
```

## Auto-Update Setup

The validator-launcher can automatically update itself from GitHub when new commits are pushed.

### Enable Auto-Update

1. **Install Ansible (if not already installed):**
```bash
sudo apt update
sudo apt install -y ansible
```

2. **Install Ansible collections:**
```bash
cd validator-launcher/ansible
ansible-galaxy collection install -r requirements.yml
```

3. **Install the auto-updater:**
```bash
cd validator-launcher/ansible
sudo ./install-updater.sh
```

This creates a systemd timer that:
- Checks for updates every 5 minutes
- Automatically rebuilds and restarts the service when updates are detected
- Only rebuilds when the code actually changes (commit hash comparison)

### Verify Auto-Update

```bash
# Check timer status
sudo systemctl status validator-launcher-updater.timer

# View auto-updater logs
sudo journalctl -u validator-launcher-updater.service -f

# See when next update check will run
sudo systemctl list-timers validator-launcher-updater.timer
```

### Manual Update

To manually trigger an update:

```bash
cd validator-launcher/ansible
sudo ansible-playbook -i localhost, -c local playbook.yml
```

## Configuration Details

### VMM URL

Set the VMM URL via environment variable or config:

```bash
export VMM_URL="http://localhost:10300"
```

Or use the config command:
```bash
sudo platform config set-vmm-url "http://localhost:10300"
```

Default: `http://localhost:10300`

### Platform Configuration File

The launcher reads environment variables from `/etc/platform-validator/config.json`:

```json
{
  "dstack_vmm_url": "http://10.0.2.2:10300/",
  "env": {
    "HOTKEY_PASSPHRASE": "your-secret-passphrase-here",
    "VALIDATOR_BASE_URL": "http://10.0.2.2:18080",
    "CUSTOM_VAR": "value"
  }
}
```

**Fields:**
- `dstack_vmm_url` (optional): VMM URL accessible from the VM (default: `http://10.0.2.2:10300/`)
- `env` (optional): Map of environment variables to inject into the VM

**Required Environment Variables:**

The Platform API specifies **required environment variable keys** (e.g., `HOTKEY_PASSPHRASE`, `DSTACK_VMM_URL`, `VALIDATOR_BASE_URL`). You must provide the **values** for these keys using the `config` command:

1. The API sends only the **keys** that are required
2. You set the **values** using `platform config set-env <key> <value>`
3. The launcher merges API keys with your local values
4. VM creation is blocked if required keys are missing values

## CLI Commands

The `platform` command provides the following subcommands:

### Configuration Management

```bash
# Show current configuration
sudo platform config show

# Set VMM URL
sudo platform config set-vmm-url "http://10.0.2.2:10300/"

# Set environment variables
sudo platform config set-env HOTKEY_PASSPHRASE "your-passphrase"
sudo platform config set-env VALIDATOR_BASE_URL "http://10.0.2.2:18080"

# List all environment variables
sudo platform config list-env

# Get a specific environment variable
sudo platform config get-env HOTKEY_PASSPHRASE

# Remove an environment variable
sudo platform config remove-env CUSTOM_VAR
```

### Running the Service

```bash
# Start the launcher service
sudo platform run

# Or use systemd
sudo systemctl start validator-launcher
```

## Logging

Set log level via `RUST_LOG` environment variable:

```bash
# Debug logging
sudo RUST_LOG=debug platform run

# Or modify systemd service
sudo systemctl edit validator-launcher
# Add:
# [Service]
# Environment="RUST_LOG=debug"
sudo systemctl restart validator-launcher
```

Available log levels: `error`, `warn`, `info`, `debug`, `trace`

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
validator-launcher/
├── src/
│   ├── main.rs          # Main application logic
│   └── config_tui.rs    # Configuration CLI commands
├── scripts/             # Service management scripts
│   ├── start-dstack-services.sh  # Start KMS, Gateway, VMM services
│   └── stop-dstack-services.sh   # Stop dstack services
├── ansible/             # Ansible playbooks for deployment
│   ├── playbook.yml     # Main deployment playbook
│   ├── install-playbook.yml  # Installation playbook
│   ├── install-updater.sh  # Auto-updater installation script
│   ├── requirements.yml # Ansible collection requirements
│   ├── templates/       # Ansible templates
│   │   └── validator-launcher.service.j2
│   └── README.md        # Ansible documentation
├── install.sh           # Main installation script
├── Cargo.toml           # Rust dependencies
├── LICENSE              # Apache 2.0 license
├── CONTRIBUTING.md      # Contribution guidelines
└── README.md            # This file
```

## Troubleshooting

### Service won't start

1. Check service status:
```bash
sudo systemctl status validator-launcher
```

2. Check logs:
```bash
sudo journalctl -u validator-launcher -n 50
```

3. Check if dstack services started:
```bash
# Check dstack services logs
sudo tail -50 /var/log/dstack/*.log

# Check if dstack processes are running
ps aux | grep dstack-

# Manually test dstack services startup
sudo /usr/local/bin/start-dstack-services.sh
```

4. Verify dstack binaries and configs exist:
```bash
ls -la /home/ubuntu/meta-dstack/build/dstack-{kms,gateway,vmm}
ls -la /home/ubuntu/meta-dstack/build/{kms,gateway,vmm}.toml
```

5. Verify configuration:
```bash
sudo platform config show
```

6. Check if required environment variables are set:
```bash
sudo platform config list-env
```

### VM creation fails

1. Verify all required environment variables are set
2. Check VMM connectivity:
```bash
curl http://localhost:10300/prpc/Status?json
```

3. Check logs for specific error messages

### Auto-update not working

1. Check timer status:
```bash
sudo systemctl status validator-launcher-updater.timer
```

2. Check auto-updater logs:
```bash
sudo journalctl -u validator-launcher-updater.service -n 50
```

3. Verify Ansible is installed:
```bash
ansible --version
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
