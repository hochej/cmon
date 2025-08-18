#!/bin/bash
set -e

echo "Building cmon binary..."

# Ensure PATH includes uv
export PATH="$HOME/.cargo/bin:$PATH"

# Install build dependencies
echo "Installing build dependencies..."
uv sync --extra build

# Create binary with PyInstaller
echo "Creating standalone binary..."
uv run pyinstaller \
    --onefile \
    --console \
    --name cmon \
    --distpath dist \
    --workpath build \
    --specpath build \
    --paths src \
    --hidden-import cmon \
    --hidden-import cmon.cli \
    --hidden-import cmon.slurm \
    --hidden-import cmon.models \
    --hidden-import cmon.display \
    --hidden-import cmon.cli_common \
    --hidden-import typer \
    --hidden-import rich \
    --hidden-import pydantic \
    cmon_entry.py

echo "Binary created at: dist/cmon"
echo "File size: $(du -h dist/cmon | cut -f1)"
echo ""
echo "Test the binary:"
echo "  ./dist/cmon --help"
echo ""
echo "Install to your PATH:"
echo "  cp dist/cmon ~/bin/"
echo "  # Ensure ~/bin is in your PATH"