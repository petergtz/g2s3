import * as cdk from 'aws-cdk-lib';
import {aws_events as events, Stack} from 'aws-cdk-lib';
import {Construct} from 'constructs';
import {Schedule} from "aws-cdk-lib/aws-events";
import {BatchJob} from "aws-cdk-lib/aws-events-targets";
import * as batch from 'aws-cdk-lib/aws-batch';
import {CfnComputeEnvironment} from 'aws-cdk-lib/aws-batch';
import {Bucket} from "aws-cdk-lib/aws-s3";
import {CronOptions} from "aws-cdk-lib/aws-events/lib/schedule";

interface Config {
    s3_backup_bucket: string;
    should_create_bucket: boolean
    backup_definitions: BackupDefinition[];
}

interface BackupDefinition {
    s3_folder: string;
    google_drive_folder: string;
    schedule?: CronOptions;
}

export class GoogleBackupToS3Stack extends cdk.Stack {
    constructor(scope: Construct, id: string, config: Config, props?: cdk.StackProps) {
        super(scope, id, props);

        if (config.should_create_bucket) {
            new Bucket(this, "backup-bucket", {bucketName: config.s3_backup_bucket});
        }
        const jobQueue = this.createDefaultJobQueue();
        for (let backup_def of config.backup_definitions) {
            const jobDefinition = new batch.CfnJobDefinition(
                this,
                `google-${backup_def.google_drive_folder}-backup-to-s3-job-def`,
                {
                    type: "container",
                    jobDefinitionName: `google-${backup_def.google_drive_folder}-backup-to-s3-job-definition`,
                    containerProperties: {
                        command: ["/back-up-drive-folder",
                            "--s3-folder", backup_def.s3_folder, backup_def.google_drive_folder, config.s3_backup_bucket],
                        image: "pego/google-backup-to-s3:latest",
                        jobRoleArn: "arn:aws:iam::512841817041:role/google-takeout-backup-batch-execution-role",
                        executionRoleArn: "arn:aws:iam::512841817041:role/google-takeout-backup-batch-execution-role",
                        networkConfiguration: {assignPublicIp: "ENABLED",},
                        resourceRequirements: [
                            {type: "VCPU", value: "1"},
                            {type: "MEMORY", value: "2048"}
                        ],
                    },
                    platformCapabilities: ["FARGATE"],
                });
            new events.Rule(this, "job-completed", {
                eventPattern:
            })
            if (backup_def.schedule) {
                new events.Rule(this, `run-google-${backup_def.google_drive_folder}-backup-to-s3-event-rule`, {
                    enabled: true,
                    ruleName: `run-google-${backup_def.google_drive_folder}-backup-to-s3`,
                    schedule: Schedule.cron(backup_def.schedule),
                    targets: [new BatchJob(jobQueue.attrJobQueueArn, jobQueue, jobDefinition.ref, jobDefinition,
                        {jobName: `google-${backup_def.google_drive_folder}-backup-to-s3`}
                    )],
                });
            }
        }
    }

    private createDefaultJobQueue() {
        return new batch.CfnJobQueue(this, "default-job-queue", {
            jobQueueName: "default-job-queue",
            computeEnvironmentOrder: [{
                computeEnvironment: new CfnComputeEnvironment(this, "default-compute-env", {
                    computeEnvironmentName: "default-compute-environment",
                    type: "MANAGED",
                    computeResources: {
                        type: "FARGATE",
                        maxvCpus: 256,
                        subnets: ["subnet-09651f50fbbae3a34"],
                        securityGroupIds: ["sg-0e7c9bcf25301a59b"],
                    },
                    serviceRole: `arn:aws:iam::${Stack.of(this).account}:role/aws-service-role/batch.amazonaws.com/AWSServiceRoleForBatch`,
                }).attrComputeEnvironmentArn,
                order: 0,
            }],
            priority: 0
        });
    }
}
