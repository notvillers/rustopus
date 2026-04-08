#!/bin/bash

AUTHCODE="AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
OUTFILE="get-products.xml"

curl -s "<url-to-rustopus>/get-products?authcode=$AUTHCODE" > "$OUTFILE"
