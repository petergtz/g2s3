#!/bin/bash -ex

cd $(dirname $0)/../cdk

npm test
cdk synth
