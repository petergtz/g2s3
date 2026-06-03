use error_chain::error_chain;

error_chain! {

    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        GoogleAPI(google_drive3::Error);
        S3Error(aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>);
        VarError(std::env::VarError);
    }
}
