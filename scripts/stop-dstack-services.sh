#!/bin/bash
# Script to stop dstack services (KMS, Gateway, VMM)

set -e

# Function to stop a service
stop_service() {
    local service_name=$1
    
    if pgrep -f "$service_name" > /dev/null; then
        echo "Stopping $service_name..."
        pkill -f "$service_name" || true
        sleep 1
        
        # Force kill if still running
        if pgrep -f "$service_name" > /dev/null; then
            echo "Force stopping $service_name..."
            pkill -9 -f "$service_name" || true
        fi
        
        rm -f "/tmp/${service_name}.pid"
        echo "✓ $service_name stopped"
    else
        echo "✓ $service_name is not running"
    fi
}

# Stop services in reverse order
stop_service "dstack-vmm"
stop_service "dstack-gateway"
stop_service "dstack-kms"

echo "All dstack services stopped"

