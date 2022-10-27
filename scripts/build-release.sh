#!/bin/bash -ex

cd $(dirname $0)/../rust

OPENSSL_DIR=/usr/local/opt/openssl@1.1 cargo build --release --target x86_64-unknown-linux-gnu

