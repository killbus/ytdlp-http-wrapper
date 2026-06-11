use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct RunRequest {
    pub args: Vec<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Serialize)]
pub struct RunResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: &'static str,
}
