use clap::{ArgGroup, Parser};
use helix_llm::intent_lattice::IntentFacets;
use helix_llm::providers::{LlmProvider, LlmRequest, Message, MessageRole, OpenAiProvider};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Translate plain English prompts into Quint specifications using an LLM.
#[derive(Parser)]
#[command(
    name = "quint_translator",
    about = "Translate English to Quint using an LLM",
    group(
        ArgGroup::new("input")
            .required(true)
            .args(["prompt", "prompt_file"])
    )
)]
struct Args {
    /// Prompt to translate
    #[arg(group = "input")]
    prompt: Vec<String>,

    /// Read prompt from a file
    #[arg(long, group = "input")]
    prompt_file: Option<PathBuf>,

    /// Model to use for translation
    #[arg(short, long, default_value = "gpt-4o-mini")]
    model: String,

    /// Sampling temperature
    #[arg(short, long, default_value_t = 0.2)]
    temperature: f32,

    /// Optional output file to write the generated Quint spec
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Override the LLM base URL
    #[arg(long)]
    base_url: Option<String>,
}

fn get_api_credentials() -> Result<(String, String), String> {
    let configs = [
        (
            "OPENROUTER_API_KEY",
            "OPENROUTER_BASE_URL",
            "https://openrouter.ai/api/v1",
        ),
        (
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "https://api.openai.com/v1",
        ),
        ("LLM_API_KEY", "LLM_BASE_URL", "https://api.openai.com/v1"),
    ];

    for (key_env, url_env, default_url) in configs {
        if let Ok(key) = env::var(key_env) {
            let base = env::var(url_env).unwrap_or_else(|_| default_url.to_string());
            return Ok((key, base));
        }
    }

    Err("No LLM API key set. Use OPENAI_API_KEY, OPENROUTER_API_KEY, or LLM_API_KEY.".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let prompt = if let Some(file) = args.prompt_file {
        fs::read_to_string(file)?
    } else {
        args.prompt.join(" ")
    };

    let facets = IntentFacets::parse(&prompt);
    let questions = facets.clarifying_questions();
    if !questions.is_empty() {
        eprintln!("Potential ambiguities detected:");
        for q in &questions {
            eprintln!("- {}", q);
        }
    }

    let (api_key, mut base_url) = get_api_credentials().unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });

    if let Some(url) = args.base_url {
        base_url = url;
    }

    let provider = OpenAiProvider::with_base_url(api_key, base_url);

    let mut parameters = HashMap::new();
    parameters.insert("model".to_string(), serde_json::json!(args.model));

    let request = LlmRequest {
        system_prompt: Some(
            "You are a translation agent that converts plain English descriptions into Quint specifications following the Quint design principles found at https://quint-lang.org/docs/design-principles.".to_string(),
        ),
        messages: vec![Message {
            role: MessageRole::User,
            content: prompt,
            function_call: None,
        }],
        max_tokens: Some(512),
        temperature: Some(args.temperature),
        top_p: None,
        functions: None,
        parameters,
    };

    let response = provider.complete(request).await?;

    if let Some(path) = args.output {
        fs::write(path, &response.content)?;
    } else {
        println!("{}", response.content);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn selects_openai_key() {
        let _lock = ENV_MUTEX.lock().unwrap();
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("LLM_API_KEY");
        env::set_var("OPENAI_API_KEY", "k");
        env::set_var("OPENAI_BASE_URL", "https://x");
        let creds = get_api_credentials().unwrap();
        assert_eq!(creds.0, "k");
        assert_eq!(creds.1, "https://x");
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("OPENAI_BASE_URL");
    }

    #[test]
    fn errors_when_missing_key() {
        let _lock = ENV_MUTEX.lock().unwrap();
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("LLM_API_KEY");
        assert!(get_api_credentials().is_err());
    }
}
