# G2S3

<img src="logo.png" height="300" align="right" alt="logo">

G2S3 is a solution to regularly back up your Google data by copying it over to S3.

**This is work in progress.**

What's working:
- It can copy a single Google Drive folder with all files in it (non-recursively) to S3
- CDK deploys AWS Batch jobs, necessary compute environment, queues, SNS topics, subscriptions, etc.

What's not working yet:
- No GitHub actions to automatically build and push container image
- CDK does not create security groups, subnets, job roles, or execution roles yet. These need to be created manually and referred to.

So while this can be used, it requires some manual effort.


## Components

G2S3 consists of three main components:
- a binary built with Rust to copy files from Google Drive to S3, called `back-up-drive-folder`
- a Docker container built from a Dockerfile to package `back-up-drive-folder` into an image
(currently available as
[pego/google-backup-to-s3](https://hub.docker.com/repository/docker/pego/google-backup-to-s3) on
Docker Hub)
- a CDK stack to deploy everything as AWS Batch job to regularly invoke `back-up-drive-folder`

### CLI `back-up-drive-folder`

This CLI can be invoked locally for testing, or from the cloud when part of a regular backup.
For instructions, on how to use it:

```shell
$ back-up-drive-folder --help
```

### Docker container

The Docker container can be used directly from [pego/google-backup-to-s3](https://hub.docker.com/repository/docker/pego/google-backup-to-s3). Or it can be built, using:

```shell
$ ./scripts/build-release.sh && ./scripts/build-container.sh
```

In the future, this container will be built as part of a GitHub action and also hosted in GitHub's
registry.

### CDK Stack

The CDK stack can be found in `./cdk/lib`.

## Getting Started

### Google Cloud setup
TODO: Describe what needs to be created in Google Cloud

### AWS Setup
TODO: Describe what needs to be created in AWS

### Configuring and invoking CDK
TODO: Describe configuration step
