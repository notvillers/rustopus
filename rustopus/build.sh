#!/bin/bash

set -e

bin_name="rustopus"
build_mode=$1

script_dir=$(dirname "$0")
cd "$script_dir"

if [ "$build_mode" == "debug" ]; then
    if [ -f "$bin_name" ]; then
        echo "Removing '$bin_name'"
        rm "$bin_name"
    fi
    echo "Building '$build_mode'"
    cargo build
    cp "./target/debug/$bin_name" ./
    echo "Cleaning cargo"
    cargo clean
elif [ "$build_mode" == "release" ]; then
    if [ -f "$bin_name" ]; then
        echo "Removing '$bin_name'"
        rm "$bin_name"
    fi
    echo "Building '$build_mode'"
    cargo build --release
    cp "./target/release/$bin_name" ./
    echo "Cleaning cargo"
    cargo clean
else
    echo "Cannot deal with '$build_mode' build mode."
    echo "Options: debug, release"
fi
