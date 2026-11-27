use galroon_mcp::{client, config, protocol, tools::Tools};
use tokio::io::AsyncBufReadExt;

const USAGE: &str = "galroon-mcp — Model Context Protocol server for Galroon

Usage:
  galroon-mcp [serve] [OPTIONS]                           Run the MCP server on stdio
  galroon-mcp pair <CODE|PAIRING_STRING> [OPTIONS]        Redeem a pairing code or LAN pairing string
  galroon-mcp check [OPTIONS]                             Verify the credential against the running Core

Options:
  --url <URL>                Core origin: http://127.0.0.1:<port> or LAN https://<host>:<port>
  --fingerprint <SPKI>       52-character base32 SPKI SHA-256 pin (required for LAN https:// URLs)
  --token-env <VAR>          Read bearer token from environment variable VAR (keeps secrets out of argv)
  --token-stdin              Read bearer token from the first line of stdin (keeps secrets out of argv)
  --library <ID>             Pin requests to a collection ID (x-galroon-library)
  --device <ID>              Pin requests to a Core device ID (x-galroon-device)

Without --url (or GALROON_URL) the saved LAN endpoint or running local Windows Core is used.
Environment: GALROON_URL, GALROON_FINGERPRINT, GALROON_TOKEN, GALROON_LIBRARY, GALROON_DEVICE,
             GALROON_MCP_TOKEN_FILE, GALROON_DATA, GALROON_CORE_EXE";

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}

fn parse_args(raw: Vec<String>) -> Result<(Vec<String>, config::Options, bool), String> {
    let mut positional = Vec::new();
    let mut options = config::Options::default();
    let mut token_stdin = false;
    let mut iter = raw.into_iter();
    while let Some(arg) = iter.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_owned(), Some(v.to_owned())),
            _ => (arg, None),
        };
        let mut take_val = |name: &str| -> Result<String, String> {
            inline.clone().or_else(|| iter.next()).filter(|s| !s.is_empty()).ok_or_else(|| format!("Missing value for {name}\n\n{USAGE}"))
        };
        match flag.as_str() {
            "--url" => options.url = Some(take_val("--url")?),
            "--fingerprint" => options.fingerprint = Some(take_val("--fingerprint")?),
            "--token-env" => options.token_env = Some(take_val("--token-env")?),
            "--library" => options.library_id = Some(take_val("--library")?),
            "--device" => options.device_id = Some(take_val("--device")?),
            "--token-stdin" => {
                if inline.is_some() {
                    return Err(format!("--token-stdin does not take a value\n\n{USAGE}"));
                }
                token_stdin = true;
            }
            "--token" => {
                return Err("Do not pass secrets via --token on the command line; use --token-env <VAR>, --token-stdin, GALROON_TOKEN, or galroon-mcp pair".into());
            }
            "-h" | "--help" => positional.push(flag),
            other if other.starts_with("--") => return Err(format!("Unknown option: {other}\n\n{USAGE}")),
            other => positional.push(other.to_owned()),
        }
    }
    if token_stdin && options.token_env.is_some() {
        return Err("Specify only one of --token-stdin or --token-env".into());
    }
    Ok((positional, options, token_stdin))
}

async fn read_token_line<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R) -> Result<String, String> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await.map_err(|e| format!("Failed to read token from stdin: {e}"))?;
    if n == 0 {
        return Err("Expected a bearer token on stdin (--token-stdin), but stdin was empty".into());
    }
    let token = line.trim().to_owned();
    if token.is_empty() {
        return Err("Bearer token read from stdin was empty".into());
    }
    Ok(token)
}

#[tokio::main]
async fn main() {
    let (positional, mut options, token_stdin) = parse_args(std::env::args().skip(1).collect()).unwrap_or_else(|e| fail(e));
    match positional.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["serve"] => {
            let mut input = tokio::io::BufReader::new(tokio::io::stdin());
            if token_stdin {
                options.token = Some(read_token_line(&mut input).await.unwrap_or_else(|e| fail(e)));
            }
            if let Err(e) = protocol::serve(Tools::with_options(options), input, tokio::io::stdout()).await {
                fail(e);
            }
        }
        ["pair", code] => {
            if token_stdin || options.token_env.is_some() {
                fail("pair redeems a pairing code and does not accept --token-env or --token-stdin");
            }
            let code = if code.starts_with("galroon-pair:") {
                if options.url.is_some() || options.fingerprint.is_some() {
                    fail("Pass either a galroon-pair: string or --url/--fingerprint with a code, not both");
                }
                let parsed = galroon_core::pairing::parse_pairing_string(code).unwrap_or_else(|e| fail(e));
                options.url = Some(format!("https://{}:{}", parsed.host, parsed.port));
                options.fingerprint = Some(parsed.fingerprint);
                parsed.code
            } else {
                (*code).to_owned()
            };
            let endpoint = config::resolve_endpoint(&options, None).await.unwrap_or_else(|e| fail(e));
            let (session, core, saved_url, saved_fp) = match &endpoint.fingerprint {
                Some(fp) => {
                    let session = client::redeem_lan(&endpoint.url, fp, &code).await.unwrap_or_else(|e| fail(e));
                    let token = session["token"].as_str().unwrap_or_else(|| fail("Core did not return a credential")).to_owned();
                    let core = client::Core::new_lan(endpoint.url.clone(), fp, token, None, None).unwrap_or_else(|e| fail(e));
                    (session, core, Some(endpoint.url.clone()), Some(fp.clone()))
                }
                None => {
                    let session = client::redeem(&endpoint.url, &code).await.unwrap_or_else(|e| fail(e));
                    let token = session["token"].as_str().unwrap_or_else(|| fail("Core did not return a credential")).to_owned();
                    let core = client::Core::new(endpoint.url.clone(), token, None).unwrap_or_else(|e| fail(e));
                    (session, core, None, None)
                }
            };
            let (library_id, device_id) = core.identity().await.unwrap_or_else(|e| fail(e));
            let token = session["token"].as_str().unwrap_or_else(|| fail("Core did not return a credential")).to_owned();
            let credential = config::Credential {
                token,
                library_id,
                device_name: session["name"].as_str().unwrap_or("MCP").to_owned(),
                expires: session["expires"].as_i64().unwrap_or(0),
                url: saved_url.clone(),
                fingerprint: saved_fp,
                device_id: if saved_url.is_some() { device_id } else { None },
            };
            let path = config::save_credential(&credential).unwrap_or_else(|e| fail(e));
            eprintln!("Paired with collection {}. Credential saved to {}", credential.library_id, path.display());
            eprintln!("It can be revoked any time in Galroon > Settings > Access.");
        }
        ["check"] => {
            if token_stdin {
                let mut input = tokio::io::BufReader::new(tokio::io::stdin());
                options.token = Some(read_token_line(&mut input).await.unwrap_or_else(|e| fail(e)));
            }
            let tools = Tools::with_options(options);
            match tools.call("galroon_overview", &serde_json::json!({})).await {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
                Err(e) => fail(e),
            }
        }
        ["-h" | "--help" | "help"] => println!("{USAGE}"),
        _ => fail(USAGE),
    }
}
