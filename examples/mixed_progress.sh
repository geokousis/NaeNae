#!/usr/bin/env bash

set -euo pipefail

for i in 1 2 3 4; do
  echo "[try $i] Best=$i | gain +$i"
  sleep 5
done
