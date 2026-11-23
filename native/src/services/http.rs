//! Asking the music services. Only the hosts the app is allowed to reach (OPEN_HOSTS in
//! proxy.rs, the same list the Tauri build checks) are asked; anything else is refused before a
//! connection is made. One client for all of them, so connections are reused.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;

/// the browser the services see (the Tauri build's page sent the same)
pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

pub fn client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| {
        crate::userproxy::apply_blocking(reqwest::blocking::Client::builder())
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .user_agent(BROWSER_UA)
            .build()
            .expect("http client")
    })
}

fn checked(url: &str) -> Result<url::Url, String> {
    let u = url::Url::parse(url).map_err(|e| e.to_string())?;
    if !crate::proxy::is_open_host(&u) {
        return Err(format!("адрес не из списка разрешённых: {}", u.host_str().unwrap_or("")));
    }
    Ok(u)
}

pub fn get(url: &str, headers: &[(&str, &str)]) -> Result<reqwest::blocking::Response, String> {
    let mut req = client().get(checked(url)?);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    req.send().map_err(|e| e.to_string())
}

/// a picture: from any https address, as the Tauri build's page could show any
/// (its CSP has `img-src https:`); nothing but the picture is read
pub fn get_image(url: &str) -> Result<Vec<u8>, String> {
    let u = url::Url::parse(url).map_err(|e| e.to_string())?;
    if u.scheme() != "https" {
        return Err("only https pictures".into());
    }
    let res = client().get(u).send().map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("{}", res.status().as_u16()));
    }
    // a cover is small; anything huge is not one
    if res.content_length().is_some_and(|l| l > 20 * 1024 * 1024) {
        return Err("too big".into());
    }
    res.bytes().map(|b| b.to_vec()).map_err(|e| e.to_string())
}

pub fn get_text(url: &str) -> Result<String, String> {
    let res = get(url, &[])?;
    if !res.status().is_success() {
        return Err(format!("{} ответил {}", host(url), res.status().as_u16()));
    }
    res.text().map_err(|e| e.to_string())
}

pub fn get_json(url: &str, headers: &[(&str, &str)]) -> Result<Value, String> {
    let res = get(url, headers)?;
    let status = res.status().as_u16();
    if !res.status().is_success() {
        return Err(format!("{} ответил {status}", host(url)));
    }
    res.json().map_err(|e| e.to_string())
}

pub fn post_json(url: &str, headers: &[(&str, &str)], body: &Value) -> Result<Value, String> {
    let mut req = client().post(checked(url)?).header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.body(body.to_string()).send().map_err(|e| e.to_string())?;
    let status = res.status().as_u16();
    if !res.status().is_success() {
        return Err(format!("{} ответил {status}", host(url)));
    }
    res.json().map_err(|e| e.to_string())
}

fn host(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
}

// ---- reading JSON without a schema ------------------------------------------------------------

/// every value under `key`, at any depth, in document order
pub fn find_all<'a>(node: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match node {
        Value::Array(a) => {
            for v in a {
                find_all(v, key, out);
            }
        }
        Value::Object(o) => {
            if let Some(v) = o.get(key) {
                out.push(v);
            }
            for v in o.values() {
                find_all(v, key, out);
            }
        }
        _ => {}
    }
}

pub fn first<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    let mut v = Vec::new();
    find_all(node, key, &mut v);
    v.into_iter().next()
}

pub fn s(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn n(v: &Value) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite())
}
