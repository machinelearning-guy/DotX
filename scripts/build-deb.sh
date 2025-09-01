#!/bin/bash
# DotX DEB Package Builder Script

set -e

# Configuration
PACKAGE_NAME="dotx"
VERSION="${1:-1.0.0}"
ARCHITECTURE="amd64"
BUILD_DIR="target/debian"
PACKAGE_DIR="${BUILD_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}"

echo "Building DotX DEB package v${VERSION}..."

# Ensure we're in the project root
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Must run from project root directory"
    exit 1
fi

# Clean up previous builds
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

# Build release binaries
echo "Building release binaries..."
cargo build --release --bin dotx
cargo build --release --bin dotx-gui

# Create package directory structure
echo "Creating package structure..."
mkdir -p "${PACKAGE_DIR}/DEBIAN"
mkdir -p "${PACKAGE_DIR}/opt/dotx/bin"
mkdir -p "${PACKAGE_DIR}/usr/share/applications"
mkdir -p "${PACKAGE_DIR}/usr/share/icons/hicolor/128x128/apps"
mkdir -p "${PACKAGE_DIR}/usr/share/mime/packages"
mkdir -p "${PACKAGE_DIR}/usr/share/doc/${PACKAGE_NAME}"
mkdir -p "${PACKAGE_DIR}/usr/share/pixmaps"

# Copy binaries
echo "Copying binaries..."
cp target/release/dotx "${PACKAGE_DIR}/opt/dotx/bin/"
cp target/release/dotx-gui "${PACKAGE_DIR}/opt/dotx/bin/"

# Make binaries executable
chmod +x "${PACKAGE_DIR}/opt/dotx/bin/dotx"
chmod +x "${PACKAGE_DIR}/opt/dotx/bin/dotx-gui"

# Copy control files
echo "Copying control files..."
cp packaging/debian/control "${PACKAGE_DIR}/DEBIAN/"
cp packaging/debian/postinst "${PACKAGE_DIR}/DEBIAN/"
cp packaging/debian/prerm "${PACKAGE_DIR}/DEBIAN/"
cp packaging/debian/postrm "${PACKAGE_DIR}/DEBIAN/"
cp packaging/debian/copyright "${PACKAGE_DIR}/usr/share/doc/${PACKAGE_NAME}/"

# Set permissions for control files
chmod 755 "${PACKAGE_DIR}/DEBIAN/postinst"
chmod 755 "${PACKAGE_DIR}/DEBIAN/prerm"
chmod 755 "${PACKAGE_DIR}/DEBIAN/postrm"
chmod 644 "${PACKAGE_DIR}/DEBIAN/control"

# Update version in control file
sed -i "s/Version: .*/Version: ${VERSION}/" "${PACKAGE_DIR}/DEBIAN/control"

# Copy desktop files and icons (will be created in next step)
if [ -f "packaging/linux/dotx.desktop" ]; then
    cp packaging/linux/dotx.desktop "${PACKAGE_DIR}/usr/share/applications/"
fi

if [ -f "packaging/linux/dotx.xml" ]; then
    cp packaging/linux/dotx.xml "${PACKAGE_DIR}/usr/share/mime/packages/"
fi

if [ -f "packaging/linux/dotx.png" ]; then
    cp packaging/linux/dotx.png "${PACKAGE_DIR}/usr/share/icons/hicolor/128x128/apps/"
    cp packaging/linux/dotx.png "${PACKAGE_DIR}/usr/share/pixmaps/"
fi

# Copy documentation
echo "Copying documentation..."
cp README.md "${PACKAGE_DIR}/usr/share/doc/${PACKAGE_NAME}/"
if [ -f "BUILDING.md" ]; then
    cp BUILDING.md "${PACKAGE_DIR}/usr/share/doc/${PACKAGE_NAME}/"
fi

# Calculate installed size
INSTALLED_SIZE=$(du -sk "${PACKAGE_DIR}" | cut -f1)
sed -i "/^Architecture:/a Installed-Size: ${INSTALLED_SIZE}" "${PACKAGE_DIR}/DEBIAN/control"

# Build the DEB package
echo "Building DEB package..."
dpkg-deb --build --root-owner-group "${PACKAGE_DIR}"

# Move to final location
DEB_FILE="${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}.deb"
mv "${PACKAGE_DIR}.deb" "${DEB_FILE}"

echo "DEB package created successfully: ${DEB_FILE}"
echo "Package size: $(du -h "${DEB_FILE}" | cut -f1)"
echo ""
echo "To install:"
echo "  sudo dpkg -i ${DEB_FILE}"
echo "  sudo apt-get install -f  # if dependencies missing"
echo ""
echo "To test:"
echo "  dpkg-deb -I ${DEB_FILE}    # show package info"
echo "  dpkg-deb -c ${DEB_FILE}    # show package contents"