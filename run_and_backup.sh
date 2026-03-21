#!/usr/bin/env bash
# Start the app in background
npm run tauri dev &
APP_PID=$!
echo "App started with PID $APP_PID"

# Wait for initialization (DB creation)
sleep 20

# Kill the app
echo "Stopping app..."
taskkill //F //IM ghub.exe 2>/dev/null || true
taskkill //F //IM npm.exe 2>/dev/null || true

# Run backup
echo "Creating backup..."
cmd //c backup_db.bat

echo "Done."
