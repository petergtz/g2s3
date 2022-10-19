use error_chain::error_chain;


error_chain! {

    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        GoogleAPI(google_drive::Error);
        S3Error(aws_smithy_http::result::SdkError<aws_sdk_s3::error::PutObjectError>);
        VarError(std::env::VarError);
    }
}
