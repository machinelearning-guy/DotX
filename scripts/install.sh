#!/bin/bash
# DotX Universal Linux installer script

set -e

# Configuration
REPO_URL="https://github.com/dotx-bio/dotx"
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

# Install dependencies
install_dependencies() {
    log_info "Installing dependencies..."
    
    case "$DISTRO" in
        ubuntu|debian)
            apt-get update
            apt-get install -y wget curl libgtk-3-0 libssl3
            
            # Try to install minimap2
            if apt-cache search minimap2 | grep -q minimap2; then
                apt-get install -y minimap2
            else
                log_warn "minimap2 not found in repositories, will need to be installed manually"
            fi
            ;;
        fedora|centos|rhel)
            dnf install -y wget curl gtk3 openssl
            log_warn "minimap2 may need to be installed manually on this distribution"
            ;;
        *)
            log_warn "Unsupported distribution, proceeding with manual dependency check"
            ;;
    esac
}

# Download and install DotX
install_dotx() {
    log_info "Downloading DotX..."
    
    # Get latest release info
    LATEST_RELEASE=$(curl -s "${REPO_URL}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$LATEST_RELEASE" ]; then
        log_error "Could not determine latest release"
        exit 1
    fi
    
    log_info "Latest version: $LATEST_RELEASE"
    
    # Download appropriate archive
    ARCHIVE_NAME="dotx-linux-x64.tar.gz"
    DOWNLOAD_URL="${REPO_URL}/releases/download/${LATEST_RELEASE}/${ARCHIVE_NAME}"
    
    cd "${TEMP_DIR}"
    wget -O "${ARCHIVE_NAME}" "${DOWNLOAD_URL}" || {
        log_error "Failed to download DotX"
        exit 1
    }
    
    # Extract archive
    log_info "Extracting DotX..."
    tar -xzf "${ARCHIVE_NAME}" || {
        log_error "Failed to extract archive"
        exit 1
    }
    
    # Create installation directory
    log_info "Installing to ${INSTALL_DIR}..."
    mkdir -p "${INSTALL_DIR}/bin"
    
    # Copy binaries
    cp dotx "${INSTALL_DIR}/bin/"
    cp dotx-gui "${INSTALL_DIR}/bin/"
    chmod +x "${INSTALL_DIR}/bin/dotx"
    chmod +x "${INSTALL_DIR}/bin/dotx-gui"
    
    # Create symlinks
    ln -sf "${INSTALL_DIR}/bin/dotx" "${BIN_DIR}/dotx"
    ln -sf "${INSTALL_DIR}/bin/dotx-gui" "${BIN_DIR}/dotx-gui"
    
    # Copy documentation
    if [ -f "README.md" ]; then
        mkdir -p "${INSTALL_DIR}/doc"
        cp README.md "${INSTALL_DIR}/doc/"
    fi
}

# Install desktop integration
install_desktop_files() {
    log_info "Installing desktop integration..."
    
    # Create desktop file
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

    # Create MIME type
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
    install_dotx
    install_desktop_files
    
    echo ""
    log_info "Installation completed successfully!"
    echo ""
    echo "You can now:"
    echo "  - Launch the GUI: dotx-gui"
    echo "  - Use the CLI: dotx --help"
    echo "  - Find DotX in Applications menu"
    echo ""
    log_info "To uninstall, run: rm -rf ${INSTALL_DIR} ${BIN_DIR}/dotx*"
}

# Run main function
main "$@"