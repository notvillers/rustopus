#!/bin/bash
# Get-Images RustOpus example

AUTHCODE="AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
OUTFILE="get-images.xml"

curl -s "<url-to-rustopus>/get-images?authcode=$AUTHCODE" > "$OUTFILE"
