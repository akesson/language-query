use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: Method,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Method {
    Docs {
        file: PathBuf,
        line: u32,
        symbol: String,
    },
    Impl {
        file: PathBuf,
        line: u32,
        symbol: String,
    },
    Refs {
        file: PathBuf,
        line: u32,
        symbol: String,
    },
    Resolve {
        file: PathBuf,
        symbol: String,
    },
    Status,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseResult {
    Success { result: serde_json::Value },
    Error { error: String },
}