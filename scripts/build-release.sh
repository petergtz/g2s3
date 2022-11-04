#!/bin/bash -ex

cd $(dirname $0)/../rust

export CC_x86_64_unknown_linux_gnu=x86_64-unknown-linux-gnu-gcc
export CXX_x86_64_unknown_linux_gnu=x86_64-unknown-linux-gnu-g++
export AR_x86_64_unknown_linux_gnu=x86_64-unknown-linux-gnu-ar
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-unknown-linux-gnu-gcc

OPENSSL_DIR=/usr/local/opt/openssl@1.1 cargo build --release --target x86_64-unknown-linux-gnu --bin back-up-drive-folder

