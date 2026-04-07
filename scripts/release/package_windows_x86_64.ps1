#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Builds and packages PersonalAgent for Windows x86_64

.DESCRIPTION
    Builds the release binary and creates a distributable ZIP artifact.
    Outputs artifact metadata to artifacts/release/ for the release workflow.

.PARAMETER ReleaseTag
    The release tag (e.g., v1.0.0). If not provided, uses GITHUB_REF_NAME env var.

.EXAMPLE
    ./scripts/release/package_windows_x86_64.ps1 v1.0.0
#>

param(
    [Parameter(Position = 0)]
    [string]$ReleaseTag
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

# Resolve release tag
if (-not $ReleaseTag) {
    $ReleaseTag = $env:GITHUB_REF_NAME
}

if (-not $ReleaseTag) {
    Write-Error "Missing release tag; pass vX.Y.Z or set GITHUB_REF_NAME"
    exit 1
}

if (-not $ReleaseTag.StartsWith('v')) {
    Write-Error "Release tag must start with v (received: $ReleaseTag)"
    exit 1
}

$RepoRoot = $PSScriptRoot | Split-Path -Parent | Split-Path -Parent
Set-Location $RepoRoot

$BinaryPath = "target/release/personal_agent_gpui.exe"
$ArtifactDir = "$RepoRoot/artifacts/release"
$ZipName = "personal-agent-$ReleaseTag-x86_64-pc-windows-msvc.zip"
$ZipPath = "$ArtifactDir/$ZipName"
$ShaFile = "$ArtifactDir/SHA256SUMS.txt"

# Clean and create artifact directory
if (Test-Path $ArtifactDir) {
    Remove-Item -Recurse -Force $ArtifactDir
}
New-Item -ItemType Directory -Path $ArtifactDir -Force | Out-Null

Write-Host "Building release binary..."
cargo build --release --bin personal_agent_gpui

if (-not (Test-Path $BinaryPath)) {
    Write-Error "Expected release binary not found at $BinaryPath"
    exit 1
}

Write-Host "Verifying binary architecture..."
$BinaryInfo = file $BinaryPath 2>$null
if ($BinaryInfo -notmatch 'PE32\+.*x86-64') {
    Write-Host "Release binary info: $BinaryInfo"
    Write-Warning "Could not verify x86_64 PE binary (file command may not be available)"
}

Write-Host "Creating ZIP archive..."
$PackageDir = New-Item -ItemType Directory -Path (New-TemporaryFile | ForEach-Object { Remove-Item $_; $_ })
try {
    Copy-Item $BinaryPath "$PackageDir/personal-agent.exe"
    
    # Copy assets directory if it exists
    if (Test-Path "$RepoRoot/assets") {
        Copy-Item -Recurse "$RepoRoot/assets" "$PackageDir/assets"
    }
    
    Compress-Archive -Path "$PackageDir/*" -DestinationPath $ZipPath -Force
}
finally {
    Remove-Item -Recurse -Force $PackageDir -ErrorAction SilentlyContinue
}

Write-Host "Computing SHA256 checksum..."
$ZipSha = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLower()
"$ZipSha  $ZipName" | Out-File -FilePath $ShaFile -Encoding utf8 -NoNewline

# Write artifact metadata
$ZipName | Out-File -FilePath "$ArtifactDir/zip_name.txt" -Encoding utf8 -NoNewline
$ZipPath | Out-File -FilePath "$ArtifactDir/zip_path.txt" -Encoding utf8 -NoNewline
$ZipSha | Out-File -FilePath "$ArtifactDir/zip_sha256.txt" -Encoding utf8 -NoNewline

Write-Host "Created release artifacts:"
Write-Host "  $ZipPath"
Write-Host "  $ShaFile"
