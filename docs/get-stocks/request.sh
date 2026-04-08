#!/bin/bash

AUTHCODE="AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
OUTFILE="get-stocks.xml"

curl -s "<url-to-rustopus>/get-stocks?authcode=$AUTHCODE" > "$OUTFILE"
