// import * as cdk from 'aws-cdk-lib';
// import { Template } from 'aws-cdk-lib/assertions';
// import * as Cdk from '../lib/cdk-stack';
import {bucketNameFrom} from '../lib/google-backup-to-s3-stack'

// example test. To run these tests, uncomment this file along with the
// example resource in lib/cdk-stack.ts
test('SQS Queue Created', () => {
//   const app = new cdk.App();
//     // WHEN
//   const stack = new Cdk.CdkStack(app, 'MyTestStack');
//     // THEN
//   const template = Template.fromStack(stack);

//   template.hasResourceProperties('AWS::SQS::Queue', {
//     VisibilityTimeout: 300
//   });
});

test('can extract bucket name from s3 URL', ()=> {
    expect(bucketNameFrom("s3://my-bucket/some/path")).toEqual("my-bucket");
    expect(bucketNameFrom("s3://my-bucket")).toEqual("my-bucket");
    expect(bucketNameFrom("s3://my-bucket/")).toEqual("my-bucket");
});
