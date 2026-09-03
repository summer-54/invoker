pub mod stream;
use std::str::FromStr;

use crate::prelude::*;

use toaster_lib_rs::{
    auth::{CertName, Token},
    server as server_lib,
};

use tonic::{
    metadata::{Ascii, MetadataValue},
    service::Interceptor,
};

pub struct TokenInterceptor {
    token: MetadataValue<Ascii>,
}

impl TokenInterceptor {
    pub fn new(token: &Token) -> Result<Self> {
        Ok(Self {
            token: MetadataValue::from_str(token)
                .context(format!("converting token: '{token:?}' into MetadataValue"))?,
        })
    }
}

impl Interceptor for TokenInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request.metadata_mut().append(
            server_lib::grpc::invoker_manager::metadata::TOKEN,
            self.token.clone(),
        );
        Ok(request)
    }
}

pub struct CertNameInterceptor {
    cert_name: MetadataValue<Ascii>,
}

impl CertNameInterceptor {
    pub fn new(cert_name: &CertName) -> Result<Self> {
        Ok(Self {
            cert_name: MetadataValue::from_str(cert_name).context(format!(
                "converting cert name: '{cert_name:?}' into MetadataValue"
            ))?,
        })
    }
}

impl Interceptor for CertNameInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request.metadata_mut().append(
            server_lib::grpc::invoker_manager::metadata::CERT_NAME,
            self.cert_name.clone(),
        );
        Ok(request)
    }
}
