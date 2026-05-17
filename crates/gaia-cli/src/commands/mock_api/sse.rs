use serde_json::Value;

pub(crate) fn to_sse_body(events: &[Value]) -> String {
    let mut output = String::new();
    for event in events {
        output.push_str("data: ");
        output.push_str(&event.to_string());
        output.push_str("\n\n");
    }
    output.push_str("data: [DONE]\n\n");
    output
}

pub(crate) fn split_for_streaming(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let end = (index + 18).min(chars.len());
        chunks.push(chars[index..end].iter().collect::<String>());
        index = end;
    }
    chunks
}
