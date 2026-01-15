#!/usr/bin/env bash
set -e  # Exit on error


    #Paths.
    SCRIPT_DIR="$(dirname "$(realpath "$0")")"
    DOTNET_ROOT=$HOME/.dotnet

    DepotDownloaderMod(){
    $DOTNET_ROOT/dotnet $SCRIPT_DIR/DepotDownloaderMod.dll "$@"
    }
    DepotDownloaderMod "$@"
