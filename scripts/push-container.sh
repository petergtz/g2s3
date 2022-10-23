#!/bin/bash -ex

cd $(dirname $0)/..

docker push pego/google-backup-to-s3:latest


