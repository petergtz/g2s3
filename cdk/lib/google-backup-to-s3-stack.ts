import {aws_events as events, aws_sns as sns, Stack} from 'aws-cdk-lib';
import {Construct} from 'constructs';
import {Schedule} from "aws-cdk-lib/aws-events";
import * as targets from "aws-cdk-lib/aws-events-targets";
import * as batch from 'aws-cdk-lib/aws-batch';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as s3 from "aws-cdk-lib/aws-s3";
import {CronOptions} from "aws-cdk-lib/aws-events/lib/schedule";
import {SubscriptionProtocol} from "aws-cdk-lib/aws-sns";
import {CfnJobDefinition} from "aws-cdk-lib/aws-batch/lib/batch.generated";
import {URL} from "url";
import assert from "assert";
import * as iam from "aws-cdk-lib/aws-iam";
import * as ec2 from "aws-cdk-lib/aws-ec2";
import SecretProperty = CfnJobDefinition.SecretProperty;

interface Config {
    backup_definitions: BackupDefinition[];
    email?: string,
    image?: string,
}

interface BackupDefinition {
    google_drive_folder: string;
    google_secrets: SecretProperty[],
    s3_url: string,
    should_create_bucket: boolean,
    storage_class?: string,
    schedule?: CronOptions;
}

export function createGoogleBackupToS3ResourcesIn(scope: Construct, config: Config) {
    const snsTopic = new sns.Topic(scope, "GoogleBackupToS3", {
        topicName: "GoogleBackupToS3",
    })
    if (!config.image) {
        config.image = "ghcr.io/petergtz/g2s3/g2s3:latest"
    }
    if (config.email) {
        new sns.Subscription(scope, "GoogleBackupToS3-SnsSubscription", {
            protocol: SubscriptionProtocol.EMAIL,
            endpoint: config.email,
            topic: snsTopic,
        })
    }
    const lambdaToForwardBatchCompletion = new lambda.Function(scope, 'lambda-forward-batch-completion-to-sns',
        {
            functionName: "ForwardBatchCompletionStatusToSnS",
            runtime: lambda.Runtime.PYTHON_3_9,
            handler: 'index.handler',
            code: lambda.Code.fromInline(`
import boto3, json
def handler(event, context): boto3.client("sns").publish(
    TargetArn="${snsTopic.topicArn}", 
    Message=json.dumps(event, sort_keys=True, indent=4), 
    Subject=f"AWS Batch job {event['detail']['jobName']} {event['detail']['status']}")`),
        });

    snsTopic.grantPublish(lambdaToForwardBatchCompletion);

    const jobQueue = createDefaultJobQueue(scope);
    let batchJobRole = new iam.Role(scope, "google-takeout-backup-job-role", {
        assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com'),
    })

    let batchJobExecutionRole = new iam.Role(scope, "google-takeout-backup-execution-job-role", {
        assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com'),
        managedPolicies: [
            iam.ManagedPolicy.fromAwsManagedPolicyName("SecretsManagerReadWrite"),
            iam.ManagedPolicy.fromAwsManagedPolicyName("service-role/AWSBatchServiceRole"),
        ],
    });

    for (let [bucketName, shouldCreate] of new Map(config.backup_definitions.map(
        backupDef => [bucketNameFrom(backupDef.s3_url), backupDef.should_create_bucket]))) {
        (shouldCreate ? new s3.Bucket(scope, `backup-bucket-${bucketName}`, {bucketName: bucketName})
            : s3.Bucket.fromBucketName(scope, `backup-bucket-${bucketName}`, bucketName)).grantPut(batchJobRole)
    }

    for (let backupDef of config.backup_definitions) {
        let command = ["/back-up-drive-folder", backupDef.google_drive_folder, backupDef.s3_url];
        if (backupDef.storage_class) {
            command.push("--s3-storage-class", backupDef.storage_class)
        }
        const jobDefinition = new batch.CfnJobDefinition(
            scope,
            `google-${backupDef.google_drive_folder}-backup-to-s3-job-def`,
            {
                type: "container",
                jobDefinitionName: `google-${backupDef.google_drive_folder}-backup-to-s3`,
                containerProperties: {
                    command: command,
                    image: config.image,
                    jobRoleArn: batchJobRole.roleArn,
                    executionRoleArn: batchJobExecutionRole.roleArn,
                    networkConfiguration: {assignPublicIp: "ENABLED",},
                    resourceRequirements: [
                        {type: "VCPU", value: "1"},
                        {type: "MEMORY", value: "2048"}
                    ],
                    secrets: backupDef.google_secrets,
                },
                platformCapabilities: ["FARGATE"],
            });

        new events.Rule(scope, jobDefinition.jobDefinitionName + "-completed", {
            eventPattern: {
                source: ["aws.batch"],
                detailType: ["Batch Job State Change"],
                detail: {
                    "status": ["FAILED", "SUCCEEDED"],
                    "jobDefinition": [jobDefinition.ref],
                }
            },
            enabled: true,
            ruleName: jobDefinition.jobDefinitionName + "-completed",
            targets: [new targets.LambdaFunction(lambdaToForwardBatchCompletion)],
        });
        if (backupDef.schedule) {
            new events.Rule(scope, `run-google-${backupDef.google_drive_folder}-backup-to-s3-event-rule`, {
                enabled: true,
                ruleName: `run-google-${backupDef.google_drive_folder}-backup-to-s3`,
                schedule: Schedule.cron(backupDef.schedule),
                targets: [new targets.BatchJob(jobQueue.attrJobQueueArn, jobQueue, jobDefinition.ref, jobDefinition,
                    {jobName: `google-${backupDef.google_drive_folder}-backup-to-s3`}
                )],
            });
        }
    }
}

export function bucketNameFrom(s3Url: string) {
    let u = new URL(s3Url);
    assert(u.protocol == "s3:");
    return u.hostname;
}

function createDefaultJobQueue(scope: Construct) {
    let vpc = new ec2.Vpc(scope, "vpc", {
        cidr: "10.0.0.0/16",
        natGateways: 0,
        subnetConfiguration: [
            {
                name: 'public',
                cidrMask: 24,
                subnetType: ec2.SubnetType.PUBLIC
            }
        ],
        vpcName: "google-backup-vpc",
    })

    return new batch.CfnJobQueue(scope, "default-job-queue", {
        jobQueueName: "default-job-queue",
        computeEnvironmentOrder: [{
            computeEnvironment: new batch.CfnComputeEnvironment(scope, "default-compute-env", {
                computeEnvironmentName: "default-compute-environment",
                type: "MANAGED",
                computeResources: {
                    type: "FARGATE",
                    maxvCpus: 256,
                    subnets: vpc.publicSubnets.map(s => s.subnetId),
                    securityGroupIds: [vpc.vpcDefaultSecurityGroup],
                },
                serviceRole: `arn:aws:iam::${Stack.of(scope).account}:role/aws-service-role/batch.amazonaws.com/AWSServiceRoleForBatch`,
            }).attrComputeEnvironmentArn,
            order: 0,
        }],
        priority: 0
    });
}
