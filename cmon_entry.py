#!/usr/bin/env python3
"""Standalone entry point for cmon CLI application (PyInstaller compatible)."""

import sys
import os

# Add the src directory to Python path for imports
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'src'))

def main():
    """Main entry point for PyInstaller."""
    try:
        from cmon.cli import app
        app()
    except ImportError as e:
        print(f"Import error: {e}")
        print("Available modules:")
        print(sys.path)
        sys.exit(1)

if __name__ == "__main__":
    main()