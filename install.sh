#!/bin/sh
set -e

# Monowiki installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/femtomc/monowiki/main/install.sh | sh

# Colors for output
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Logging functions
info() {
    printf "${BLUE}info${NC}: %s\n" "$1"
}

success() {
    printf "${GREEN}success${NC}: %s\n" "$1"
}

warn() {
    printf "${YELLOW}warning${NC}: %s\n" "$1"
}

error() {
    printf "${RED}error${NC}: %s\n" "$1" >&2
    exit 1
}

# Detect OS and architecture
detect_platform() {
    local os
    local arch

    # Detect OS
    case "$(uname -s)" in
        Linux*)
            os="linux"
            ;;
        Darwin*)
            os="macos"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            os="windows"
            ;;
        *)
            error "Unsupported operating system: $(uname -s)"
            ;;
    esac

    # Detect architecture
    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        aarch64|arm64)
            arch="aarch64"
            ;;
        *)
            error "Unsupported architecture: $(uname -m)"
            ;;
    esac

    # Construct platform string
    PLATFORM="${os}-${arch}"

    # Validate platform against available releases
    case "$PLATFORM" in
        linux-x86_64|macos-x86_64|macos-aarch64)
            ;;
        windows-x86_64)
            error "Windows is not supported by this installer. Please download manually from: https://github.com/femtomc/monowiki/releases"
            ;;
        *)
            error "No pre-built binary available for platform: $PLATFORM"
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    info "Fetching latest release version..."

    # Try to get latest release from GitHub API
    if command -v curl >/dev/null 2>&1; then
        LATEST_VERSION=$(curl -fsSL https://api.github.com/repos/femtomc/monowiki/releases/latest | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        LATEST_VERSION=$(wget -qO- https://api.github.com/repos/femtomc/monowiki/releases/latest | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
    else
        error "Neither curl nor wget found. Please install one of them and try again."
    fi

    if [ -z "$LATEST_VERSION" ]; then
        error "Failed to fetch latest version from GitHub"
    fi

    info "Latest version: $LATEST_VERSION"
}

# Download and extract binary
download_and_install() {
    local download_url="https://github.com/femtomc/monowiki/releases/latest/download/monowiki-${PLATFORM}.tar.gz"
    local tmp_dir

    # Create temporary directory
    tmp_dir=$(mktemp -d 2>/dev/null || mktemp -d -t 'monowiki-install')

    info "Downloading monowiki from: $download_url"

    # Download the tarball
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$download_url" -o "$tmp_dir/monowiki.tar.gz" || error "Failed to download monowiki"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$tmp_dir/monowiki.tar.gz" "$download_url" || error "Failed to download monowiki"
    fi

    info "Extracting binary..."
    tar -xzf "$tmp_dir/monowiki.tar.gz" -C "$tmp_dir" || error "Failed to extract tarball"

    # Determine install location
    if [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    elif [ -n "$HOME" ]; then
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
    else
        error "Could not determine installation directory"
    fi

    info "Installing monowiki to $INSTALL_DIR..."

    # Try to move the binary
    if [ -w "$INSTALL_DIR" ]; then
        mv "$tmp_dir/monowiki" "$INSTALL_DIR/monowiki" || error "Failed to install monowiki"
    else
        # Need sudo
        info "Installation requires elevated privileges..."
        if command -v sudo >/dev/null 2>&1; then
            sudo mv "$tmp_dir/monowiki" "$INSTALL_DIR/monowiki" || error "Failed to install monowiki"
        else
            error "sudo not found and $INSTALL_DIR is not writable"
        fi
    fi

    # Make executable
    if [ -w "$INSTALL_DIR/monowiki" ]; then
        chmod +x "$INSTALL_DIR/monowiki"
    else
        sudo chmod +x "$INSTALL_DIR/monowiki"
    fi

    # Cleanup
    rm -rf "$tmp_dir"
}

# Verify installation
verify_installation() {
    if command -v monowiki >/dev/null 2>&1; then
        local installed_version
        installed_version=$(monowiki --version 2>/dev/null | head -n1 || echo "unknown")
        success "monowiki installed successfully!"
        info "Installed version: $installed_version"
        info "Location: $(command -v monowiki)"
    else
        warn "monowiki installed but not found in PATH"
        warn "Please add $INSTALL_DIR to your PATH"
        warn ""
        warn "Add this to your shell configuration file (~/.bashrc, ~/.zshrc, etc.):"
        warn "  export PATH=\"\$PATH:$INSTALL_DIR\""
    fi
}

# Print usage instructions
print_next_steps() {
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    success "Installation complete!"
    echo ""
    echo "Get started with:"
    echo "  ${GREEN}monowiki init${NC}     # Initialize a new wiki"
    echo "  ${GREEN}monowiki dev${NC}      # Start development server"
    echo "  ${GREEN}monowiki build${NC}    # Build static site"
    echo ""
    echo "Documentation: https://github.com/femtomc/monowiki"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
}

# Main installation flow
main() {
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "  ${BLUE}Monowiki Installer${NC}"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    detect_platform
    info "Detected platform: $PLATFORM"

    get_latest_version
    download_and_install
    verify_installation
    print_next_steps
}

# Run main installation
main
