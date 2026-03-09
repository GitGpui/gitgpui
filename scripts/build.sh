#!/usr/bin/env bash
set -euo pipefail

RUSTFLAGS="-C symbol-mangling-version=v0" cargo build -p gitcomet-app --all-targets --profile=release-with-debug
