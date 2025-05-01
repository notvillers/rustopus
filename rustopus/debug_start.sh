#!/bin/bash

set -e

script_dir=$(dirname "$")
cd $script_dir

./target/debug/rustopus
