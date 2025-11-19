#!/bin/bash
# Script to start dstack services (KMS, Gateway, VMM) in background
# This script is called before starting validator-launcher

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DSTACK_DIR="${DSTACK_DIR:-/home/ubuntu/meta-dstack/build}"
LOG_DIR="${LOG_DIR:-/var/log/dstack}"

# Create log directory if it doesn't exist
mkdir -p "$LOG_DIR"

# Check if dstack directory exists
if [ ! -d "$DSTACK_DIR" ]; then
    echo "Error: Dstack directory not found at $DSTACK_DIR"
    echo "Please set DSTACK_DIR environment variable or ensure dstack is installed"
    exit 1
fi

# Check if dstack binaries exist
if [ ! -f "$DSTACK_DIR/dstack-kms" ] || [ ! -f "$DSTACK_DIR/dstack-gateway" ] || [ ! -f "$DSTACK_DIR/dstack-vmm" ]; then
    echo "Warning: Some dstack binaries not found in $DSTACK_DIR"
    echo "Expected files: dstack-kms, dstack-gateway, dstack-vmm"
fi

# Function to check if a process is running
is_running() {
    pgrep -f "$1" > /dev/null
}

# Function to start a service
start_service() {
    local service_name=$1
    local command=$2
    local log_file="$LOG_DIR/${service_name}.log"
    
    if is_running "$service_name"; then
        echo "✓ $service_name is already running"
        return 0
    fi
    
    echo "Starting $service_name..."
    cd "$DSTACK_DIR"
    nohup $command >> "$log_file" 2>&1 &
    local pid=$!
    
    # Wait a bit to check if it started successfully
    sleep 2
    if is_running "$service_name"; then
        echo "✓ $service_name started (PID: $pid)"
        echo "$pid" > "/tmp/${service_name}.pid"
        return 0
    else
        echo "✗ Failed to start $service_name. Check $log_file for details."
        return 1
    fi
}

# Start KMS
start_service "dstack-kms" "./dstack-kms -c kms.toml" || exit 1

# Wait a bit for KMS to initialize
sleep 2

# Start Gateway
start_service "dstack-gateway" "sudo ./dstack-gateway -c gateway.toml" || exit 1

# Wait a bit for Gateway to initialize
sleep 2

# Start VMM
start_service "dstack-vmm" "./dstack-vmm -c vmm.toml" || exit 1

# Wait a bit for VMM to initialize
sleep 3

echo "All dstack services started successfully"
exit 0

