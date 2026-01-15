#!/usr/bin/env bash
set -e  # Exit on error

# Steamless CLI Wrapper
# Usage: ./Steamless.sh /path/to/game.exe

SCRIPT_DIR="$(dirname "$(realpath "$0")")"
DOTNET_ROOT=$HOME/.dotnet

Steamless() {
    $DOTNET_ROOT/dotnet "$SCRIPT_DIR/steamless-net/Steamless.CLI.dll" "$@"
}

Steamless "$@"
