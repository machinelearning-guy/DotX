#!/bin/bash
# DotX Universal Linux installer script

set -e

# Configuration
REPO_URL="https://github.com/machinelearning-guy/DotX"
INSTALL_DIR="/opt/dotx"
BIN_DIR="/usr/local/bin"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Utility functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

cleanup() {
    rm -rf "${TEMP_DIR}"
}

trap cleanup EXIT

# Check if running as root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

# Detect Linux distribution
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO=$ID
        VERSION=$VERSION_ID
    else
        log_error "Cannot detect Linux distribution"
        exit 1
    fi
    
    log_info "Detected: $PRETTY_NAME"
}

# Install Rust toolchain
install_rust() {
    log_info "Checking for Rust installation..."
    
    # Check both system-wide and user-local Rust installations
    if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        log_info "Rust already installed: $(rustc --version)"
        return 0
    fi
    
    # Try to source user's Rust environment if it exists
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
        if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
            log_info "Found user Rust installation: $(rustc --version)"
            return 0
        fi
    fi
    
    log_info "Installing Rust toolchain..."
    
    # Install Rust as the invoking user, not root
    if [ -n "${SUDO_USER}" ]; then
        # If run with sudo, install as the original user
        sudo -u "${SUDO_USER}" bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
        RUST_HOME="/home/${SUDO_USER}/.cargo"
    else
        # Install as current user
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        RUST_HOME="$HOME/.cargo"
    fi
    
    # Source the environment
    if [ -f "${RUST_HOME}/env" ]; then
        source "${RUST_HOME}/env"
    fi
    
    # Add to PATH for current session
    export PATH="${RUST_HOME}/bin:$PATH"
    
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Failed to install Rust toolchain"
        log_error "Please install Rust manually: https://rustup.rs/"
        exit 1
    fi
    
    log_info "Rust installed successfully: $(rustc --version)"
}

# Install dependencies
install_dependencies() {
    log_info "Installing dependencies..."
    
    case "$DISTRO" in
        ubuntu|debian)
            # Update package lists, but don't fail on broken repositories
            log_info "Updating package lists..."
            if ! apt-get update; then
                log_warn "Package update had some issues (possibly broken PPAs), but continuing..."
            fi
            
            log_info "Installing required packages..."
            if apt-get install -y wget curl git build-essential libgtk-3-dev libssl-dev pkg-config; then
                log_info "Successfully installed build dependencies"
            else
                log_error "Failed to install required packages"
                exit 1
            fi
            
            # Try to install minimap2
            log_info "Checking for minimap2 availability..."
            if apt-cache search minimap2 | grep -q minimap2; then
                if apt-get install -y minimap2; then
                    log_info "Successfully installed minimap2"
                else
                    log_warn "minimap2 installation failed, but continuing"
                fi
            else
                log_warn "minimap2 not found in repositories, will need to be installed manually"
            fi
            ;;
        fedora|centos|rhel)
            log_info "Installing packages for Fedora/CentOS/RHEL..."
            if dnf install -y wget curl git gcc gcc-c++ gtk3-devel openssl-devel pkgconfig; then
                log_info "Successfully installed build dependencies"
            else
                log_error "Failed to install required packages"
                exit 1
            fi
            log_warn "minimap2 may need to be installed manually on this distribution"
            ;;
        *)
            log_warn "Unsupported distribution, proceeding with manual dependency check"
            ;;
    esac
}

# Clone and build DotX from source
install_dotx() {
    log_info "Cloning DotX repository from ${REPO_URL}..."
    
    cd "${TEMP_DIR}"
    if git clone "${REPO_URL}" dotx-source; then
        log_info "Successfully cloned repository"
    else
        log_error "Failed to clone repository"
        log_error "Please check your internet connection and repository access"
        exit 1
    fi
    
    cd dotx-source
    
    log_info "Building DotX from source..."
    
    # Ensure cargo is available - try multiple locations
    if [ -n "${SUDO_USER}" ]; then
        RUST_HOME="/home/${SUDO_USER}/.cargo"
    else
        RUST_HOME="$HOME/.cargo"
    fi
    
    export PATH="${RUST_HOME}/bin:$PATH"
    
    # Source Rust environment if available
    if [ -f "${RUST_HOME}/env" ]; then
        source "${RUST_HOME}/env"
    fi
    
    # Verify cargo is available
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Cargo not found in PATH. Please ensure Rust is properly installed."
        exit 1
    fi
    
    log_info "Using Rust: $(rustc --version)"
    log_info "Using Cargo: $(cargo --version)"
    
    # Build the project in release mode
    log_info "Starting build process (this may take several minutes)..."
    if cargo build --release; then
        log_info "Successfully built DotX"
    else
        log_error "Failed to build DotX"
        log_error "Please check the build logs above for details"
        log_error "Common issues: missing dependencies, network problems during crate downloads"
        exit 1
    fi
    
    # Create installation directory
    log_info "Installing to ${INSTALL_DIR}..."
    mkdir -p "${INSTALL_DIR}/bin"
    
    # Check what binaries were built
    log_info "Checking build output..."
    ls -la target/release/ | grep -E "dotx(-gui)?$" || log_warn "No dotx binaries found in expected location"
    
    # Copy binaries from target/release
    INSTALLED_BINARIES=0
    if [ -f "target/release/dotx" ]; then
        cp "target/release/dotx" "${INSTALL_DIR}/bin/dotx-cli"
        chmod +x "${INSTALL_DIR}/bin/dotx-cli"
        ln -sf "${INSTALL_DIR}/bin/dotx-cli" "${BIN_DIR}/dotx-cli"
        log_info "✓ Installed dotx CLI as 'dotx-cli'"
        INSTALLED_BINARIES=$((INSTALLED_BINARIES + 1))
    else
        log_warn "✗ dotx CLI binary not found in build output"
    fi
    
    if [ -f "target/release/dotx-gui" ]; then
        cp "target/release/dotx-gui" "${INSTALL_DIR}/bin/"
        chmod +x "${INSTALL_DIR}/bin/dotx-gui"
        # Make 'dotx' command launch the GUI
        ln -sf "${INSTALL_DIR}/bin/dotx-gui" "${BIN_DIR}/dotx"
        ln -sf "${INSTALL_DIR}/bin/dotx-gui" "${BIN_DIR}/dotx-gui"
        log_info "✓ Installed dotx GUI (accessible via 'dotx' and 'dotx-gui')"
        INSTALLED_BINARIES=$((INSTALLED_BINARIES + 1))
    else
        log_warn "✗ dotx-gui binary not found in build output"
    fi
    
    if [ $INSTALLED_BINARIES -eq 0 ]; then
        log_error "No binaries were installed! Build may have failed."
        exit 1
    fi
    
    # Copy documentation
    if [ -f "README.md" ]; then
        mkdir -p "${INSTALL_DIR}/doc"
        cp README.md "${INSTALL_DIR}/doc/"
    fi
    
    # Copy desktop files and icons if they exist
    if [ -d "packaging/linux" ]; then
        mkdir -p "${INSTALL_DIR}/share"
        cp -r packaging/linux/* "${INSTALL_DIR}/share/" 2>/dev/null || true
    fi
}

# Install desktop integration
install_desktop_files() {
    log_info "Installing desktop integration..."
    
    # Install icon if available
    if [ -f "${INSTALL_DIR}/share/dotx.png" ]; then
        mkdir -p /usr/share/pixmaps
        cp "${INSTALL_DIR}/share/dotx.png" /usr/share/pixmaps/dotx.png
        log_info "Installed application icon"
    fi
    
    # Install desktop file
    if [ -f "${INSTALL_DIR}/share/dotx.desktop" ]; then
        cp "${INSTALL_DIR}/share/dotx.desktop" /usr/share/applications/
        # Update the Exec path to point to our installation
        sed -i "s|Exec=.*|Exec=${INSTALL_DIR}/bin/dotx-gui %F|" /usr/share/applications/dotx.desktop
        log_info "Installed desktop file"
    else
        # Create desktop file if not found
        cat > /usr/share/applications/dotx.desktop << 'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=DotX
Comment=Extreme-scale dot plot visualization for bioinformatics
Exec=/opt/dotx/bin/dotx-gui %F
Icon=dotx
Terminal=false
StartupNotify=true
Categories=Science;Biology;Education;
MimeType=application/x-dotx;
EOF
        log_info "Created desktop file"
    fi

    # Install MIME type
    if [ -f "${INSTALL_DIR}/share/dotx.xml" ]; then
        mkdir -p /usr/share/mime/packages
        cp "${INSTALL_DIR}/share/dotx.xml" /usr/share/mime/packages/
        log_info "Installed MIME type"
    else
        # Create MIME type if not found
        mkdir -p /usr/share/mime/packages
        cat > /usr/share/mime/packages/dotx.xml << 'EOF'
<?xml version="1.0"?>
<mime-info xmlns='http://www.freedesktop.org/standards/shared-mime-info'>
    <mime-type type="application/x-dotx">
        <comment>DotX Project File</comment>
        <glob pattern="*.dotx"/>
    </mime-type>
</mime-info>
EOF
        log_info "Created MIME type"
    fi

    # Update databases
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications/
    fi
    
    if command -v update-mime-database >/dev/null 2>&1; then
        update-mime-database /usr/share/mime/
    fi
}

# Main installation process
main() {
    echo ""
    log_info "DotX Linux Installer"
    log_info "===================="
    echo ""
    
    check_root
    detect_distro
    install_dependencies
    install_rust
    install_dotx
    install_desktop_files
    
    echo ""
    log_info "Installation completed successfully!"
    echo ""
    
    # Verify installation
    log_info "Verifying installation..."
    if command -v dotx >/dev/null 2>&1; then
        echo "  ✓ dotx GUI installed and available in PATH"
        echo "    (This launches the graphical interface)"
    else
        echo "  ✗ dotx command not found in PATH"
    fi
    
    if command -v dotx-cli >/dev/null 2>&1; then
        echo "  ✓ dotx-cli installed and available in PATH"
        echo "    Version: $(dotx-cli --version 2>/dev/null || echo 'Unknown')"
    else
        echo "  ✗ dotx-cli not found in PATH"
    fi
    
    if command -v dotx-gui >/dev/null 2>&1; then
        echo "  ✓ dotx-gui also available as alternative GUI command"
    fi
    
    echo ""
    echo "Usage:"
    echo "  - Launch the GUI: dotx"
    echo "  - Use the CLI: dotx-cli --help"
    echo "  - Alternative GUI command: dotx-gui"
    echo "  - Find DotX in Applications menu"
    echo ""
    log_info "Installation directory: ${INSTALL_DIR}"
    log_info "To uninstall, run: rm -rf ${INSTALL_DIR} ${BIN_DIR}/dotx ${BIN_DIR}/dotx-cli ${BIN_DIR}/dotx-gui"
}

# Run main function
main "$@"