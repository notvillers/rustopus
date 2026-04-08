#!/bin/bash
# Get-Images RustOpus example

AUTHCODE="AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
PID=1234
OUTFILE="get-bulk.xml"

curl -s "<url-to-rustopus>/get-bulk?authcode=$AUTHCODE&pid=$PID0" > "$OUTFILE"
