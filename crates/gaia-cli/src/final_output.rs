pub fn print_final_summary(backend_name: &str, model_id: &str, api_base_url: &str, api_key: &str) {
    let chat_endpoint = format!("{api_base_url}/chat/completions");
    let masked_api_key = mask_secret(api_key);

    println!();
    println!("+-------------------------------------------------------------+");
    println!("| Your LLM server is running                                  |");
    println!("+-------------------------------------------------------------+");
    println!("| Backend:        {:<43}|", backend_name);
    println!("| Model:          {:<43}|", truncate(model_id, 43));
    println!("| API base URL:   {:<43}|", truncate(api_base_url, 43));
    println!("| Chat endpoint:  {:<43}|", truncate(&chat_endpoint, 43));
    println!("| API key:        {:<43}|", truncate(&masked_api_key, 43));
    println!("+-------------------------------------------------------------+");
}

pub fn print_connection_examples(api_base_url: &str, api_key: &str, model_id: &str) {
    let _ = api_key;
    println!();
    println!("Use this to connect your agent or app:");
    println!("(set GAIA_API_KEY in your environment first)");
    println!();
    println!("Python:");
    println!();
    println!("import os");
    println!("from openai import OpenAI");
    println!();
    println!("client = OpenAI(");
    println!("    base_url=\"{api_base_url}\",");
    println!("    api_key=os.environ[\"GAIA_API_KEY\"],");
    println!(")");
    println!();
    println!("response = client.chat.completions.create(");
    println!("    model=\"{model_id}\",");
    println!("    messages=[");
    println!("        {{\"role\": \"user\", \"content\": \"Hello!\"}}");
    println!("    ],");
    println!(")");
    println!();
    println!("print(response.choices[0].message.content)");
    println!();
    println!("JavaScript:");
    println!();
    println!("import OpenAI from \"openai\";");
    println!();
    println!("const client = new OpenAI({{");
    println!("  baseURL: \"{api_base_url}\",");
    println!("  apiKey: process.env.GAIA_API_KEY,");
    println!("}});");
    println!();
    println!("const response = await client.chat.completions.create({{");
    println!("  model: \"{model_id}\",");
    println!("  messages: [{{ role: \"user\", content: \"Hello!\" }}],");
    println!("}});");
    println!();
    println!("console.log(response.choices[0].message.content);");
    println!();
    println!("curl:");
    println!();
    println!(
        "curl -sS -X POST \"{api_base_url}/chat/completions\" \\\n  -H \"Authorization: Bearer $GAIA_API_KEY\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"model\":\"{model_id}\",\"messages\":[{{\"role\":\"user\",\"content\":\"Hello!\"}}],\"temperature\":0.7,\"stream\":false}}'"
    );
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return format!("{value:<width$}");
    }

    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index + 3 >= width {
            output.push_str("...");
            break;
        }
        output.push(ch);
    }
    format!("{output:<width$}")
}

fn mask_secret(secret: &str) -> String {
    let len = secret.chars().count();
    if len <= 6 {
        return "***".to_owned();
    }

    let prefix: String = secret.chars().take(3).collect();
    let suffix: String = secret
        .chars()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}***{suffix}")
}
