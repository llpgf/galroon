use galroon_mcp::{client, config, protocol, tools::Tools};

const USAGE: &str = "galroon-mcp — Model Context Protocol server for Galroon

Usage:
  galroon-mcp [serve] [--url http://127.0.0.1:<port>]   Run the MCP server on stdio
  galroon-mcp pair <CODE> [--url ...]                  Redeem a pairing code from Galroon (Settings > Access)
  galroon-mcp check [--url ...]                        Verify the saved credential against the running Core

Without --url (or GALROON_URL) the running local Core is found automatically on Windows.
Environment: GALROON_URL, GALROON_TOKEN, GALROON_MCP_TOKEN_FILE, GALROON_DATA, GALROON_CORE_EXE";

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut url = None;
    if let Some(i) = args.iter().position(|a| a == "--url") {
        if i + 1 >= args.len() {
            fail(USAGE);
        }
        url = Some(args.remove(i + 1));
        args.remove(i);
    }
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["serve"] => {
            let input = tokio::io::BufReader::new(tokio::io::stdin());
            if let Err(e) = protocol::serve(Tools::new(url), input, tokio::io::stdout()).await {
                fail(e);
            }
        }
        ["pair", code] => {
            let endpoint = config::endpoint(url.as_deref()).await.unwrap_or_else(|e| fail(e));
            let session = client::redeem(&endpoint.url, code).await.unwrap_or_else(|e| fail(e));
            let token = session["token"].as_str().unwrap_or_else(|| fail("Core did not return a credential")).to_owned();
            let core = client::Core::new(endpoint.url.clone(), token.clone(), None).unwrap_or_else(|e| fail(e));
            let library_id = core.library().await.unwrap_or_else(|e| fail(e));
            let credential = config::Credential { token, library_id, device_name: session["name"].as_str().unwrap_or("MCP").to_owned(), expires: session["expires"].as_i64().unwrap_or(0) };
            let path = config::save_credential(&credential).unwrap_or_else(|e| fail(e));
            eprintln!("Paired with collection {}. Credential saved to {}", credential.library_id, path.display());
            eprintln!("It can be revoked any time in Galroon > Settings > Access.");
        }
        ["check"] => {
            let tools = Tools::new(url);
            match tools.call("galroon_overview", &serde_json::json!({})).await {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
                Err(e) => fail(e),
            }
        }
        ["-h" | "--help" | "help"] => println!("{USAGE}"),
        _ => fail(USAGE),
    }
}
