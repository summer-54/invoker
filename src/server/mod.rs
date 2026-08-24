pub mod stream;
use std::str::FromStr;

use crate::prelude::*;

use toaster_lib_rs::server as server_lib;

use tonic::{
    metadata::{Ascii, MetadataValue},
    service::Interceptor,
};

pub struct TokenInterceptor {
    token: MetadataValue<Ascii>,
}

impl TokenInterceptor {
    pub fn new(token: &str) -> Result<Self> {
        Ok(Self {
            token: MetadataValue::from_str(token)
                .context(format!("converting token: '{}' into MetadataValue", token))?,
        })
    }
}

impl Interceptor for TokenInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request.metadata_mut().append("token", self.token.clone());
        Ok(request)
    }
}
