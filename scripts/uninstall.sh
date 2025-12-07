#!/bin/bash
# Uninstall script for rspin

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

INSTALL_DIR="$HOME/.local/bin"
DOC_DIR="$HOME/.local/share/doc/rspin"

echo -e "${BLUE}==> Uninstalling rspin...${NC}"

# Remove binary
if [ -f "$INSTALL_DIR/rspin" ]; then
    rm "$INSTALL_DIR/rspin"
    echo -e "${GREEN}✓ Binary removed from $INSTALL_DIR${NC}"
else
    echo -e "${RED}rspin binary not found in $INSTALL_DIR${NC}"
fi

# Remove documentation
if [ -d "$DOC_DIR" ]; then
    rm -rf "$DOC_DIR"
    echo -e "${GREEN}✓ Documentation removed from $DOC_DIR${NC}"
fi

echo ""
echo -e "${GREEN}==> Uninstall complete!${NC}"
echo ""
echo "Note: You may want to remove the PATH entry from your shell configuration"
echo "if you added it during installation."
