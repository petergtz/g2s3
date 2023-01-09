# G2S3

<img src="logo.png" height="300" align="right" alt="logo">

G2S3 is a solution to regularly back up your Google data by copying it over to S3.

**This is work in progress.**

What's working:
- It can copy a single Google Drive folder with all files in it (non-recursively) to S3
- CDK deploys AWS Batch jobs, necessary compute environment, queues, SNS topics, subscriptions, etc.

What's not working yet:
- Recursive copy of a Google Drive folder, or entire Google Drive into S3.


## Components

G2S3 consists of three main components:
- a **binary** built with Rust to copy files from Google Drive to S3, called `back-up-drive-folder`
- a **Docker image** built from a Dockerfile to package `back-up-drive-folder`, available as GitHub
[Package g2s3/g2s3](https://github.com/petergtz/g2s3/pkgs/container/g2s3%2Fg2s3).
- a **CDK stack** to deploy everything as AWS Batch job to regularly invoke `back-up-drive-folder`

### CLI `back-up-drive-folder`

This CLI can be invoked locally for testing, or from the cloud when part of a regular backup.
For instructions, on how to use it:

```shell
$ back-up-drive-folder --help
```

### Docker Image


The [Docker image](https://github.com/petergtz/g2s3/pkgs/container/g2s3%2Fg2s3) (built via
[this GitHub Action](
https://github.com/petergtz/g2s3/blob/main/.github/workflows/build-and-package-rust-binary.yaml))
can be used directly using:

```shell
$ docker pull ghcr.io/petergtz/g2s3/g2s3:latest
```

Or it can be built using:

```shell
$ ./scripts/build-release.sh && ./scripts/build-container.sh
```

### CDK Stack

The CDK stack can be found in `./cdk/lib`.

## Getting Started

### Google Cloud setup
TODO: Describe what needs to be created in Google Cloud

### AWS Setup
TODO: Describe what needs to be created in AWS

### Configuring and invoking CDK
TODO: Describe configuration step
