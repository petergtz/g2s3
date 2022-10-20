#!/bin/bash -ex

cd $(dirname $0)/..

docker run \
    --mount type=bind,source="$(pwd)"/private,target=/private \
    --mount type=bind,source="$HOME"/.aws,target=/root/.aws  \
    -it pego/google-backup-to-s3 bash


