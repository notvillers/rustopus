#!/bin/bash

set -e

script_dir=$(dirname "$")
cd $script_dir

cargo build --release
cp ./target/release/rustopus ./
cargo clean