#!/bin/bash
# Get-Barcodes RustOpus example

AUTHCODE="AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
OUTFILE="get-barcodes.xml"

curl -s "<url-to-rustopus>/get-barcodes?authcode=$AUTHCODE" > "$OUTFILE"
