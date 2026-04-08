#!/bin/bash
# Get-Prices RustOpus example

AUTHCODE="AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
PID=1234
OUTFILE="get-prices.xml"

curl -s "<url-to-rustopus>/get-prices?authcode=$AUTHCODE&pid=$PID" > "$OUTFILE"
