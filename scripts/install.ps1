# gdio installer for Windows
# Usage: powershell -c "irm https://gdio.seremtitus.co.ke/install.ps1 | iex"

$ErrorActionPreference = "Stop"

$GITHUB_REPO = "SeremTitus/gdio"
$INSTALL_DIR = if ($env:GDIO_INSTALL_DIR) { $env:GDIO_INSTALL_DIR } else { "$env:LOCALAPPDATA\gdio\bin" }

# Colors
function Write-Info  { param($Msg) Write-Host "[OK] $Msg" -ForegroundColor Green }
function Write-Warn  { param($Msg) Write-Host "[!] $Msg" -ForegroundColor Yellow }
function Write-Error { param($Msg) Write-Host "[X] $Msg" -ForegroundColor Red; exit 1 }

# Detect architecture
function Get-Arch {
    $arch = $env:PROCESSOR_ARCHITECTURE
    $process = Get-Process -Id $PID
    if ($process.architecture -eq "X86") {
        return "x86_32"
    }
    switch ($arch) {
        "AMD64"   { return "x86_64" }
        "ARM64"   { return "aarch64" }
        "x86"     { return "x86_32" }
        default   { Write-Error "Unsupported architecture: $arch" }
    }
}

# Get latest release version
function Get-LatestVersion {
    $url = "https://api.github.com/repos/$GITHUB_REPO/releases/latest"
    try {
        $release = Invoke-RestMethod -Uri $url -UseBasicParsing
        return $release.tag_name -replace '^v', ''
    } catch {
        Write-Error "Failed to fetch latest version from GitHub"
    }
}

# Download binary
function Download-Binary {
    param($Version, $Arch)

    $filename = "gdio-windows-$Arch.zip"
    $url = "https://github.com/$GITHUB_REPO/releases/download/v$Version/$filename"
    $tempFile = Join-Path $env:TEMP "gdio.zip"

    Write-Host "Downloading gdio $Version for windows $Arch..."
    try {
        Invoke-WebRequest -Uri $url -OutFile $tempFile -UseBasicParsing
    } catch {
        Write-Error "Failed to download binary: $_"
    }

    return $tempFile
}

# Install binary
function Install-Binary {
    param($TempFile)

    # Create install directory
    if (-not (Test-Path $INSTALL_DIR)) {
        New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
    }

    # Extract
    $extractDir = Join-Path $env:TEMP "gdio_extract"
    Expand-Archive -Path $TempFile -DestinationPath $extractDir -Force

    # Find the binary
    $binary = Get-ChildItem -Path $extractDir -Recurse -Filter "gdio.exe" | Select-Object -First 1
    if (-not $binary) {
        Write-Error "gdio.exe not found in archive"
    }

    # Move to install directory
    Copy-Item $binary.FullName "$INSTALL_DIR\gdio.exe" -Force

    # Cleanup
    Remove-Item $TempFile -Force -ErrorAction SilentlyContinue
    Remove-Item $extractDir -Recurse -Force -ErrorAction SilentlyContinue

    Write-Info "Installed gdio to $INSTALL_DIR\gdio.exe"
}

# Add to PATH
function Setup-Path {
    # Check if already in PATH
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -like "*$INSTALL_DIR*") {
        Write-Info "Already in PATH"
        return
    }

    # Add to user PATH
    $newPath = "$INSTALL_DIR;$currentPath"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    $env:Path = "$INSTALL_DIR;$env:Path"

    Write-Info "Added $INSTALL_DIR to user PATH"
    Write-Warn "Restart your terminal or run: `$env:Path = `"$INSTALL_DIR;`$env:Path`""
}

# Get installed version
function Get-InstalledVersion {
    $exePath = Join-Path $INSTALL_DIR "gdio.exe"
    if (-not (Test-Path $exePath)) {
        return $null
    }
    try {
        $output = & $exePath --version 2>&1 | Select-Object -First 1
        if ($output -match 'v?([0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)*)') {
            return $Matches[1]
        }
    } catch {}
    return $null
}

# Main
function Main {
    Write-Host "gdio installer" -ForegroundColor Cyan
    Write-Host ""

    $arch = Get-Arch
    $installedVersion = Get-InstalledVersion
    $latestVersion = Get-LatestVersion

    Write-Info "Detected: windows $arch"

    # Check if already installed
    if ($installedVersion -eq $latestVersion) {
        Write-Info "Version $latestVersion already installed"
        return
    }

    if ($installedVersion) {
        Write-Info "Installed version: $installedVersion"
    }
    Write-Info "Latest version: $latestVersion"

    $tempFile = Download-Binary $latestVersion $arch
    Install-Binary $tempFile
    Setup-Path

    # Install shell completions
    & "$INSTALL_DIR\gdio.exe" completions --install 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Info "Shell completions installed"
    } else {
        Write-Warn "Could not install shell completions (non-critical)"
    }

    Write-Host ""

    # Print centered ASCII art
    $asciiArt = @(
        ""
        "                                     @@@@@                  @@@@@                                    "
        "                                 @@@@@@@@@@                @@@@@@@@@@                                "
        "                             @@@@@@@@@@@@@@@              @@@@@@@@@@@@@@@                            "
        "                          @@@@@@@@@@@@@@@@@@@            @@@@@@@@@@@@@@@@@@@                         "
        "                         @@@@@@@@@@@@@@@@@@@@@@        @@@@@@@@@@@@@@@@@@@@@@                        "
        "                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        "
        "                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        "
        "                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        "
        "                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        "
        "        @                @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                @       "
        "      @@@@@@            @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@            @@@@@@     "
        "    @@@@@@@@@@        @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@        @@@@@@@@@@   "
        "    @@@@@@@@@@@@@   @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   @@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@@@@@          @@@@@@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@                  @@@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@                    @@@@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@                        @@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                         @@@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                          @@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@                           @@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@                           @@@@@@@@@@@@@@@@   "
        "    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@                           @@@@@@@@@@@@@@@@   "
        "     @@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                          @@@@@@@@@@@@@@@    "
        "     @@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                         @@@@@@@@@@@@@@@@    "
        "      @@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@                       @@@@@@@@@@@@@@@@     "
        "        @@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@                    @@@@@@@@@@@@@@@       "
        "         @@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@@                @@@@@@@@@@@@@@@@        "
        "           @@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@@@@@         @@@@@@@@@@@@@@@@@@          "
        "              @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@             "
        "                 @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                "
        "                     @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                    "
        "                          @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                         "
        "                                 @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                              "
        ""
    )

    $maxWidth = ($asciiArt | ForEach-Object { $_.Length } | Measure-Object -Maximum).Maximum
    $termWidth = $Host.UI.RawUI.WindowSize.Width
    if (-not $termWidth -or $termWidth -lt $maxWidth) { $termWidth = 120 }
    foreach ($line in $asciiArt) {
        $pad = [Math]::Max(0, [Math]::Floor(($termWidth + $line.Length) / 2) - $line.Length)
        Write-Host (' ' * $pad + $line)
    }

    Write-Host ""
    Write-Host "Successfully installed gdio v$latestVersion!" -ForegroundColor Green
    Write-Host "  Run 'gdio --help' to get started."
}

Main
