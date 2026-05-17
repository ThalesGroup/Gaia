use serde_json::{Value, json};
use tiny_http::StatusCode;

use super::{HttpResponse, json_response};

pub(crate) fn openai_error_response(
    status: StatusCode,
    message: &str,
    error_type: &str,
) -> HttpResponse {
    json_response(
        status,
        json!({
            "error": {
                "message": message,
                "type": error_type,
                "param": Value::Null,
                "code": Value::Null
            }
        }),
    )
}
