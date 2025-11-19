#!/bin/bash
# Installation script for Validator Launcher
# This script uses Ansible to install the validator-launcher

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANSIBLE_PLAYBOOK="$SCRIPT_DIR/ansible/install-playbook.yml"

echo "=========================================="
echo "Validator Launcher Installation"
echo "=========================================="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

# Check if we're in the correct directory
if [ ! -f "$SCRIPT_DIR/Cargo.toml" ]; then
    echo "Error: Cargo.toml not found. Are you in the correct directory?"
    exit 1
fi

# Check if Ansible playbook exists
if [ ! -f "$ANSIBLE_PLAYBOOK" ]; then
    echo "Error: Ansible playbook not found at $ANSIBLE_PLAYBOOK"
    exit 1
fi

# Check if ansible-playbook is available, install Ansible if not
if ! command -v ansible-playbook &> /dev/null; then
    echo "Ansible not found. Installing Ansible..."
    apt-get update
    apt-get install -y ansible
    echo "✓ Ansible installed"
fi

# Install Ansible collections if needed (will be done by playbook if Ansible was just installed)
if command -v ansible-playbook &> /dev/null && [ -f "$SCRIPT_DIR/ansible/requirements.yml" ]; then
    echo "Installing Ansible collections..."
    ansible-galaxy collection install -r "$SCRIPT_DIR/ansible/requirements.yml" || true
fi

# Run the Ansible playbook
echo "Running installation playbook..."
cd "$SCRIPT_DIR"
ansible-playbook -i localhost, -c local "$ANSIBLE_PLAYBOOK"

echo ""
echo "Installation completed successfully!"
