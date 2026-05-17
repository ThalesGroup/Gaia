use tiny_http::Request;

pub(crate) fn is_authorized(request: &Request, api_key: &str) -> bool {
    if api_key.trim().is_empty() {
        return true;
    }

    request.headers().iter().any(|header| {
        header.field.equiv("Authorization") && header.value.as_str() == format!("Bearer {api_key}")
    })
}
