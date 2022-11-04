#!/bin/bash -ex

cd $(dirname $0)/../cdk

cdk deploy --hotswap
