pub mod stream;
#[cfg(not(feature = "mock"))]
pub mod websocket;

use toaster_lib_rs::server as server_lib;
