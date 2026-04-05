#!/bin/sh
set -eu

if cargo fetch --locked >/dev/null 2>&1; then
  exit 0
fi

cargo fetch >/dev/null 2>&1
