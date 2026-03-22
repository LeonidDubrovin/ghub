#!/bin/bash
# Simple test runner for title extraction tests
cd src-tauri
cargo test title_extraction::tests -- --nocapture 2>&1 | cat