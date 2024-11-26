#!/usr/bin/env bash
# Script for building your rust projects.
set -e

source ci/common.bash

# $1 {path} = Path to cross/cargo executable
CROSS="$1"

required_arg "$CROSS" 'CROSS'

RUSTDOCFLAGS='--deny warnings' "$CROSS" doc --workspace --all-features --no-deps
