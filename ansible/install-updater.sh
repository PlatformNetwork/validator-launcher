#!/bin/bash
# Install script for validator-launcher auto-updater
# This script sets up a systemd timer to run the Ansible playbook every 5 minutes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANSIBLE_DIR="$SCRIPT_DIR"

echo "Installing validator-launcher auto-updater..."

# Create systemd service
sudo tee /etc/systemd/system/validator-launcher-updater.service > /dev/null <<EOF
[Unit]
Description=Validator Launcher Auto-Updater
After=network.target

[Service]
Type=oneshot
User=root
WorkingDirectory=$ANSIBLE_DIR
ExecStart=/usr/bin/ansible-playbook -i localhost, -c local playbook.yml
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Create systemd timer
sudo tee /etc/systemd/system/validator-launcher-updater.timer > /dev/null <<EOF
[Unit]
Description=Run Validator Launcher Auto-Updater every 5 minutes
Requires=validator-launcher-updater.service

[Timer]
OnBootSec=5min
OnUnitActiveSec=5min
Unit=validator-launcher-updater.service

[Install]
WantedBy=timers.target
EOF

# Reload systemd
sudo systemctl daemon-reload

# Enable and start the timer
sudo systemctl enable validator-launcher-updater.timer
sudo systemctl start validator-launcher-updater.timer

echo "✓ Validator launcher auto-updater installed successfully"
echo ""
echo "Status:"
sudo systemctl status validator-launcher-updater.timer --no-pager -l
echo ""
echo "To check logs:"
echo "  sudo journalctl -u validator-launcher-updater.service -f"
echo ""
echo "To check timer status:"
echo "  sudo systemctl status validator-launcher-updater.timer"

