#!/bin/bash

while true; do
  timestamp="[$(date)]"
  echo "$timestamp"
  echo "$timestamp" >> curl_output.txt

  curl -s https://api.orinkhungary.hu/get-test | tee -a curl_output.txt
  echo ""

  sleep 1  # Optional delay between requests
done
