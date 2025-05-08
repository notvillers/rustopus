#!/bin/bash

set -e

script_dir=$(dirname "$")
cd $script_dir

cargo clean
cargo build --release