#!/usr/bin/env bash

set -euo pipefail

for i in 1 2 3 4; do
  echo "Best=$i"
  sleep 5
done
