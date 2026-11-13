//! A proxy the user sets themselves (Settings → Services), for places where the services
//! cannot be reached directly. It is kept in a small file next to the app's data and read
//! once at start: every connection the app makes (the services, the songs, downloads) and the
//! page's own requests go through it. Only a plain HTTP proxy address is taken (what Clash,
//! v2rayN, Hiddify and most others offer next to SOCKS as a "mixed" or "HTTP" port).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ADDRESS: OnceLock<Option<String>> = OnceLock::new();

fn file(dir: &Path) -> PathBuf {
    dir.join("proxy.txt")
}

/// "host:port" or "http://host:port" as the address to use, or None when it is not one
pub fn parse(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    let with_scheme = if text.contains("://") { text.to_string() } else { format!("http://{text}") };
    let url = url::Url::parse(&with_scheme).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() || url.path() != "/" || url.query().is_some() {
        return None;
    }
    // the port is required: a proxy has no default one worth guessing (the parsed address
    // forgets a default port, so the text is looked at)
    let (_, port) = with_scheme.trim_end_matches('/').rsplit_once(':')?;
    if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(url.as_str().trim_end_matches('/').to_string())
}

/// reads the saved address; called once at start, before any connection is made
pub fn load(dir: &Path) {
    let saved = std::fs::read_to_string(file(dir)).ok().and_then(|t| parse(&t));
    let _ = ADDRESS.set(saved);
}

pub fn current() -> Option<&'static str> {
    ADDRESS.get().and_then(|a| a.as_deref())
}

/// what is saved on disk now (it is used after the next start)
pub fn saved(dir: &Path) -> String {
    std::fs::read_to_string(file(dir)).ok().and_then(|t| parse(&t)).unwrap_or_default()
}

/// saves the address (empty: no proxy); a wrong address is refused
pub fn save(dir: &Path, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return match std::fs::remove_file(file(dir)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    }
    let address = parse(text).ok_or_else(|| "нужен адрес вида 127.0.0.1:7890".to_string())?;
    std::fs::write(file(dir), address).map_err(|e| e.to_string())
}

pub fn apply(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    match current().and_then(|a| reqwest::Proxy::all(a).ok()) {
        Some(proxy) => builder.proxy(proxy),
        None => builder,
    }
}

pub fn apply_blocking(builder: reqwest::blocking::ClientBuilder) -> reqwest::blocking::ClientBuilder {
    match current().and_then(|a| reqwest::Proxy::all(a).ok()) {
        Some(proxy) => builder.proxy(proxy),
        None => builder,
    }
}

/// the browser engine's own requests (pictures and the like) follow the same proxy
pub fn browser_arg() -> Option<String> {
    current().map(|a| format!(" --proxy-server={a}"))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn takes_plain_http_proxy_addresses() {
        assert_eq!(parse("127.0.0.1:7890").as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(parse("  http://localhost:8080/ ").as_deref(), Some("http://localhost:8080"));
        assert_eq!(parse("https://proxy.example.com:8443").as_deref(), Some("https://proxy.example.com:8443"));
    }

    #[test]
    fn refuses_what_is_not_an_address() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("127.0.0.1"), None);
        assert_eq!(parse("socks5://127.0.0.1:1080"), None);
        assert_eq!(parse("http://127.0.0.1:80/path"), None);
        assert_eq!(parse("not an address"), None);
    }
}
