# DotX Windows Build Script
# This script builds DotX for Windows and creates an installer

param(
    [switch]$Release,
    [switch]$Installer,
    [string]$OutputDir = "dist"
)

Write-Host "Building DotX for Windows..." -ForegroundColor Green

# Ensure we're in the project root
if (-not (Test-Path "Cargo.toml")) {
    Write-Error "Must run from project root directory"
    exit 1
}

# Create output directory
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# Build configuration
$buildMode = if ($Release) { "release" } else { "debug" }
$buildFlags = if ($Release) { @("--release") } else { @() }

Write-Host "Building in $buildMode mode..." -ForegroundColor Yellow

# Build CLI binary
Write-Host "Building CLI binary..." -ForegroundColor Yellow
& cargo build --bin dotx @buildFlags --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to build CLI binary"
    exit 1
}

# Build GUI binary
Write-Host "Building GUI binary..." -ForegroundColor Yellow  
& cargo build --bin dotx-gui @buildFlags --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to build GUI binary"
    exit 1
}

# Copy binaries to output directory
$targetDir = if ($Release) { "target/x86_64-pc-windows-msvc/release" } else { "target/x86_64-pc-windows-msvc/debug" }

Copy-Item "$targetDir/dotx.exe" -Destination "$OutputDir/" -Force
Copy-Item "$targetDir/dotx-gui.exe" -Destination "$OutputDir/" -Force

# Copy additional files
Copy-Item "README.md" -Destination "$OutputDir/" -Force
if (Test-Path "LICENSE") {
    Copy-Item "LICENSE" -Destination "$OutputDir/" -Force
}

Write-Host "Binaries built successfully!" -ForegroundColor Green
Write-Host "Output directory: $OutputDir" -ForegroundColor Yellow

# Create installer if requested
if ($Installer) {
    Write-Host "Creating Windows installer..." -ForegroundColor Yellow
    
    # Check if WiX Toolset is installed
    $wixPath = Get-Command "heat.exe" -ErrorAction SilentlyContinue
    if (-not $wixPath) {
        Write-Warning "WiX Toolset not found. Please install WiX Toolset v3 to create installer."
        Write-Host "Download from: https://wixtoolset.org/releases/" -ForegroundColor Cyan
    } else {
        # Create installer using WiX (implementation would go here)
        Write-Host "WiX Toolset found. Installer creation not yet implemented." -ForegroundColor Yellow
    }
}

Write-Host "Build completed!" -ForegroundColor Green