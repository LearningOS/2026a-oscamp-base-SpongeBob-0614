#!/bin/bash
set -e

# Install riscv64 cross-compilation tools and QEMU
sudo apt-get update
sudo apt-get install -y gcc-riscv64-linux-gnu qemu-user jq

# Add riscv64 target
rustup target add riscv64gc-unknown-linux-gnu

# Build the oscamp CLI tool
cargo install --path cli
