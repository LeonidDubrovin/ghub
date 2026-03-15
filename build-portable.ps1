# GHub Build Script - Windows x64
# Output: .\build\win64\

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=== GHub Build (Windows x64) ===" -ForegroundColor Cyan
Write-Host ""

# Go to script directory
Set-Location (Split-Path -Parent $MyInvocation.MyCommand.Path)

# Refresh PATH from system environment (required when running from npm)
$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")

# Check tools
if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Rust/Cargo not found. Install from https://rustup.rs" -ForegroundColor Red
    exit 1
}
if (-not (Get-Command "npm" -ErrorAction SilentlyContinue)) {
    Write-Host "Error: npm not found" -ForegroundColor Red
    exit 1
}

Write-Host "Tools: cargo, npm - OK" -ForegroundColor Green

# Install deps if needed
if (-not (Test-Path ".\node_modules")) {
    Write-Host "Installing dependencies..." -ForegroundColor Yellow
    npm install
}

# Build frontend
Write-Host "Building frontend..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -ne 0) { exit 1 }

# Build Tauri (release, x64)
Write-Host "Building Tauri (release, x64)..." -ForegroundColor Yellow
Write-Host "This may take several minutes..." -ForegroundColor Gray
npx tauri build --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { exit 1 }

# Copy to build folder
$OutputDir = ".\build\win64"
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$ExePath = ".\src-tauri\target\x86_64-pc-windows-msvc\release\ghub.exe"
Copy-Item $ExePath -Destination $OutputDir -Force

$Size = [math]::Round((Get-Item "$OutputDir\ghub.exe").Length / 1MB, 2)

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Output: $((Resolve-Path $OutputDir).Path)\ghub.exe ($Size MB)"
Write-Host ""
