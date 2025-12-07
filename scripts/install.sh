#!/bin/bash
# Install script for rspin
# This script installs rspin to ~/.local/bin and ensures it's in PATH

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

INSTALL_DIR="$HOME/.local/bin"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo -e "${BLUE}==> Installing rspin...${NC}"

# Create installation directory if it doesn't exist
if [ ! -d "$INSTALL_DIR" ]; then
    echo -e "${YELLOW}Creating directory: $INSTALL_DIR${NC}"
    mkdir -p "$INSTALL_DIR"
fi

# Copy binary
echo -e "${BLUE}==> Copying binary to $INSTALL_DIR${NC}"
if [ -f "$SCRIPT_DIR/rspin" ]; then
    cp "$SCRIPT_DIR/rspin" "$INSTALL_DIR/rspin"
    chmod +x "$INSTALL_DIR/rspin"
    echo -e "${GREEN}✓ Binary installed${NC}"
else
    echo -e "${RED}Error: rspin binary not found in $SCRIPT_DIR${NC}"
    exit 1
fi

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}⚠ $INSTALL_DIR is not in your PATH${NC}"
    echo ""
    echo "Add the following line to your shell configuration file:"
    echo ""
    
    # Detect shell
    if [ -n "$BASH_VERSION" ]; then
        SHELL_RC="$HOME/.bashrc"
        echo -e "${BLUE}  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc${NC}"
    elif [ -n "$ZSH_VERSION" ]; then
        SHELL_RC="$HOME/.zshrc"
        echo -e "${BLUE}  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc${NC}"
    else
        SHELL_RC="$HOME/.profile"
        echo -e "${BLUE}  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.profile${NC}"
    fi
    
    echo ""
    read -p "Would you like to add it automatically? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "export PATH=\"\$HOME/.local/bin:\$PATH\"" >> "$SHELL_RC"
        echo -e "${GREEN}✓ PATH updated in $SHELL_RC${NC}"
        echo -e "${YELLOW}Please restart your shell or run: source $SHELL_RC${NC}"
    fi
else
    echo -e "${GREEN}✓ $INSTALL_DIR is already in PATH${NC}"
fi

# Copy documentation
DOC_DIR="$HOME/.local/share/doc/rspin"
if [ ! -d "$DOC_DIR" ]; then
    mkdir -p "$DOC_DIR"
fi

if [ -f "$SCRIPT_DIR/README.md" ]; then
    cp "$SCRIPT_DIR/README.md" "$DOC_DIR/"
    echo -e "${GREEN}✓ Documentation installed to $DOC_DIR${NC}"
fi

echo ""
echo -e "${GREEN}==> Installation complete!${NC}"
echo ""
echo "Run 'rspin --help' to get started"
echo "Example: rspin image.png"
echo ""
echo "For clipboard support, install one of:"
echo "  • wl-clipboard (Wayland)"
echo "  • xclip (X11/Xwayland)"
