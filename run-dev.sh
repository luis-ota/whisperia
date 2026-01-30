#!/bin/bash

# Whisperia Tauri Development Runner
# This script starts the HTTP server and Tauri dev mode

echo "Starting Whisperia development environment..."

# Kill any existing processes
pkill -f "python3 -m http.server 3000" 2>/dev/null
pkill -f "cargo tauri dev" 2>/dev/null
sleep 2

# Start HTTP server for frontend
echo "Starting HTTP server on port 3000..."
cd /home/luis/dev/whisperia/src
python3 -m http.server 3000 > /tmp/whisperia-http.log 2>&1 &
HTTP_PID=$!
echo "HTTP server PID: $HTTP_PID"

# Wait for server to be ready
sleep 2

# Start Tauri dev
echo "Starting Tauri development mode..."
cd /home/luis/dev/whisperia/src-tauri
cargo tauri dev 2>&1 &
TAURI_PID=$!
echo "Tauri dev PID: $TAURI_PID"

echo ""
echo "Whisperia is starting..."
echo "The app window should appear in a few moments."
echo ""
echo "To stop: kill $HTTP_PID $TAURI_PID"
