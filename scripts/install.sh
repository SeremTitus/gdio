#!/usr/bin/env bash
set -euo pipefail

# gdio installer for Linux, macOS, and Windows (Git Bash/MSYS2/Cygwin)
# Usage: curl -fsSL https://gdio.seremtitus.co.ke/install.sh | bash

APP=gdio
GITHUB_REPO="SeremTitus/gdio"
INSTALL_DIR="${GDIO_INSTALL_DIR:-$HOME/.gdio/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
MUTED='\033[0;2m'
NC='\033[0m'

info()  { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}!${NC} $*"; }
error() { echo -e "${RED}✗${NC} $*" >&2; exit 1; }

# Detect OS
raw_os=$(uname -s)
case "$raw_os" in
    Darwin*) os="macos" ;;
    Linux*)  os="linux" ;;
    MINGW*|MSYS*|CYGWIN*) os="windows" ;;
    *)       error "Unsupported OS: $raw_os" ;;
esac

# Detect WSL
if grep -qi microsoft /proc/version 2>/dev/null; then
    warn "WSL detected. Use the PowerShell command if a Windows install was intended."
fi

# Detect architecture (matches release naming)
arch=$(uname -m)
case "$arch" in
    x86_64|amd64)   arch="x86_64" ;;
    i686|i386)      arch="x86_32" ;;
    aarch64|arm64)  arch="aarch64" ;;
    armv7l|armhf)   arch="armv7" ;;
    *)              error "Unsupported architecture: $arch" ;;
esac

# Validate OS/Arch combination
combo="$os-$arch"
case "$combo" in
    linux-x86_64|linux-aarch64|linux-armv7|linux-x86_32|macos-x86_64|macos-aarch64|windows-x86_64|windows-x86_32|windows-arm64)
        ;;
    *)
        error "Unsupported OS/Arch: $os/$arch"
        ;;
esac

# Determine archive extension
archive_ext=".tar.gz"
if [ "$os" = "windows" ]; then
    archive_ext=".zip"
fi

# Check required tools
if [ "$os" = "windows" ]; then
    if ! command -v powershell.exe >/dev/null 2>&1; then
        error "PowerShell is required but not found."
    fi
else
    if ! command -v tar >/dev/null 2>&1; then
        error "'tar' is required but not installed."
    fi
fi

mkdir -p "$INSTALL_DIR"

# Check installed version
installed_version=""
if [ -f "$INSTALL_DIR/$APP" ] || [ -f "$INSTALL_DIR/${APP}.exe" ]; then
    installed_version=$("$INSTALL_DIR/$APP" --version 2>/dev/null | head -1 | sed -n 's/.*v\{0,1\}\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\(-[a-zA-Z0-9][a-zA-Z0-9]*\)*\).*/\1/p' || true)
fi

# Get latest version
requested_version=${VERSION:-}
filename="${APP}-${os}-${arch}${archive_ext}"

if [ -z "$requested_version" ]; then
    url="https://github.com/${GITHUB_REPO}/releases/latest/download/${filename}"
    specific_version=$(curl -s https://api.github.com/repos/${GITHUB_REPO}/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
    if [ -z "$specific_version" ]; then
        error "Failed to fetch latest version from GitHub"
    fi
else
    requested_version="${requested_version#v}"
    url="https://github.com/${GITHUB_REPO}/releases/download/v${requested_version}/${filename}"
    specific_version=$requested_version
fi

info "Detected: ${os} ${arch}"

# Check if already installed
if [ -n "$installed_version" ] && [ "$installed_version" = "$specific_version" ]; then
    info "Version $specific_version already installed"
    exit 0
fi

if [ -n "$installed_version" ]; then
    info "Installed version: $installed_version"
fi
info "Latest version: $specific_version"

# Download
tmp_dir="${TMPDIR:-/tmp}/gdio_install_$$"
mkdir -p "$tmp_dir"

echo -e "${MUTED}Downloading ${APP} v${specific_version} for ${os} ${arch}...${NC}"
curl -# -L -o "$tmp_dir/${filename}" "$url"

# Extract
if [ "$os" = "windows" ]; then
    powershell.exe -NoProfile -Command "Expand-Archive -Path '$(cygpath -w "$tmp_dir/${filename}")' -DestinationPath '$(cygpath -w "$tmp_dir")' -Force" || error "Failed to extract archive"
else
    tar -xzf "$tmp_dir/${filename}" -C "$tmp_dir" || error "Failed to extract archive"
fi

# Find binary
binary_name="${APP}"
if [ "$os" = "windows" ]; then
    binary_name="${APP}.exe"
fi

binary=$(find "$tmp_dir" -name "$binary_name" -type f 2>/dev/null | head -1)
[ -n "$binary" ] || error "Binary not found in archive"

# Install
mv "$binary" "$INSTALL_DIR/$binary_name"
chmod 755 "$INSTALL_DIR/$binary_name"
rm -rf "$tmp_dir"

info "Installed ${APP} to $INSTALL_DIR/$binary_name"

# Add to PATH
current_shell=$(basename "${SHELL:-sh}")
case $current_shell in
    fish)     config_file="$HOME/.config/fish/config.fish" ;;
    zsh)      config_file="${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash)     config_file="$HOME/.bashrc" ;;
    *)        config_file="$HOME/.profile" ;;
esac

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    if [ -f "$config_file" ] && grep -qF "$INSTALL_DIR" "$config_file" 2>/dev/null; then
        info "Already in PATH"
    elif [ -f "$config_file" ]; then
        case $current_shell in
            fish)
                echo -e "\n# gdio" >> "$config_file"
                echo "fish_add_path $INSTALL_DIR" >> "$config_file"
                ;;
            *)
                echo -e "\n# gdio" >> "$config_file"
                echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$config_file"
                ;;
        esac
        info "Added to PATH in $config_file"
    else
        warn "No config file found. Manually add to your shell config:"
        warn "  export PATH=\"$INSTALL_DIR:\$PATH\""
    fi
else
    info "Already in PATH"
fi

# Install shell completions
if "$INSTALL_DIR/$binary_name" completions --install 2>/dev/null; then
    info "Shell completions installed"
else
    warn "Could not install shell completions (non-critical)"
fi

echo ""

# Print centered ASCII art
ASCII_ART='
                                     @@@@@                  @@@@@                                    
                                 @@@@@@@@@@                @@@@@@@@@@                                
                             @@@@@@@@@@@@@@@              @@@@@@@@@@@@@@@                            
                          @@@@@@@@@@@@@@@@@@@            @@@@@@@@@@@@@@@@@@@                         
                         @@@@@@@@@@@@@@@@@@@@@@        @@@@@@@@@@@@@@@@@@@@@@                        
                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        
                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        
                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        
                         @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                        
        @                @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                @       
      @@@@@@            @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@            @@@@@@     
    @@@@@@@@@@        @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@        @@@@@@@@@@   
    @@@@@@@@@@@@@   @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   @@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@@@@@          @@@@@@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@                  @@@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@                    @@@@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@                        @@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                         @@@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                          @@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@                           @@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@                           @@@@@@@@@@@@@@@@   
    @@@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@                           @@@@@@@@@@@@@@@@   
     @@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                          @@@@@@@@@@@@@@@    
     @@@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@                         @@@@@@@@@@@@@@@@    
      @@@@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@                       @@@@@@@@@@@@@@@@     
        @@@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@                    @@@@@@@@@@@@@@@       
         @@@@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@@                @@@@@@@@@@@@@@@@        
           @@@@@@@@@@@@@@@@             @@@@@@@@@@@@@@@@@@@@@@@@         @@@@@@@@@@@@@@@@@@          
              @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@             
                 @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                
                     @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                    
                          @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                         
                                 @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@                              '

IFS=$'\n' read -r -d '' -a art_lines <<< "$ASCII_ART"
max_width=0
for line in "${art_lines[@]}"; do
    (( ${#line} > max_width )) && max_width=${#line}
done
term_width=$(tput cols 2>/dev/null || echo 80)
for line in "${art_lines[@]}"; do
    printf "%*s\n" $(( (term_width + ${#line}) / 2 )) "$line"
done

echo ""
echo -e "${GREEN}Successfully installed gdio v${specific_version}!${NC}"
echo "  Run '${APP} --help' to get started."
