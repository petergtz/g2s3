#!/bin/bash -ex

cd $(dirname $0)/..

docker build -t pego/google-backup-to-s3 .


