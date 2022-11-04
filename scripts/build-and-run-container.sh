#!/bin/bash -ex

cd $(dirname $0)

./build-release.sh && ./build-container.sh && ./run-container.sh
