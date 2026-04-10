#!/usr/bin/env bash

set -euo pipefail

echo "starting"
sleep 2
echo "warning: something looks off"
sleep 2
echo "error: fake failure"
exit 1
