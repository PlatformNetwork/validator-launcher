# Ansible Playbook for Validator Launcher

This Ansible playbook automates the deployment and updates of validator-launcher.

## Prerequisites

- Ansible installed on the control machine
- SSH access to the target machine (or run on localhost)
- Rust installed (the playbook will install it automatically if missing)

## Installing Ansible

```bash
sudo apt update
sudo apt install -y ansible
```

## Installing Required Collections

```bash
cd ansible
ansible-galaxy collection install -r requirements.yml
```

## Usage

### Local Execution (on target machine)

```bash
cd /home/ubuntu/validator-launcher/ansible
ansible-playbook -i localhost, -c local playbook.yml
```

### Remote Execution

1. Create an `inventory.ini` file:
```ini
[validator_hosts]
your-server-ip ansible_user=ubuntu
```

2. Run the playbook:
```bash
ansible-playbook -i inventory.ini playbook.yml
```

## Variables

Variables can be overridden in a `vars.yml` file or via `-e`:

```bash
ansible-playbook -i localhost, -c local playbook.yml -e "validator_launcher_branch=develop"
```

Available variables:
- `validator_launcher_repo`: Git repository URL (default: https://github.com/PlatformNetwork/validator-launcher.git)
- `validator_launcher_path`: Installation path (default: /home/ubuntu/validator-launcher)
- `validator_launcher_branch`: Branch to use (default: main)
- `service_name`: Systemd service name (default: validator-launcher)
- `build_user`: User for compilation (default: ubuntu)

## Features

- ✅ Automatic repository clone if missing
- ✅ Automatic updates from GitHub
- ✅ Change detection (commit hash)
- ✅ Rebuild only when necessary
- ✅ Binary installation in `/usr/local/bin`
- ✅ Systemd service creation and management
- ✅ Automatic service restart after update

## Verification

After execution, verify the service:

```bash
sudo systemctl status validator-launcher
sudo journalctl -u validator-launcher -f
```

## Auto-Updater Installation (Recommended)

To install the auto-update system that checks for updates every 5 minutes:

```bash
cd /home/ubuntu/validator-launcher/ansible
sudo ./install-updater.sh
```

This will:
- Create a systemd service `validator-launcher-updater.service`
- Create a systemd timer `validator-launcher-updater.timer` that runs every 5 minutes
- Automatically start the timer

## Auto-Updater Verification

```bash
# Check timer status
sudo systemctl status validator-launcher-updater.timer

# View logs from last execution
sudo journalctl -u validator-launcher-updater.service -n 50

# Follow logs in real-time
sudo journalctl -u validator-launcher-updater.service -f

# See when the timer will run next
sudo systemctl list-timers validator-launcher-updater.timer
```

## Automation with Cron (Alternative)

If you prefer using cron instead of systemd timer:

```bash
# Add to crontab (crontab -e)
*/5 * * * * cd /home/ubuntu/validator-launcher/ansible && /usr/bin/ansible-playbook -i localhost, -c local playbook.yml >> /var/log/validator-launcher-update.log 2>&1
```

This will run the playbook every 5 minutes.
