#!/bin/bash
script_dir=$(dirname "$")
cd $script_dir

cd target/release

ditto -c -k --sequesterRsrc --keepParent "Rustopus Client.app" "Rustopus Client.zip"