#!/bin/bash
set -e

systemctl --user stop api.orinkhungary.hu.service
rm rustopus
cp target/release/rustopus .
systemctl --user start api.orinkhungary.hu.service

