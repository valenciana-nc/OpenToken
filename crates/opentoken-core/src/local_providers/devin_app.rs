//! First-party desktop deployment context. A custom tenant must never be forwarded to public cloud.
use super::*;
#[derive(Default, Deserialize)]
struct Context {
    #[serde(rename = "apiServerUrl")]
    stored: Option<String>,
    #[serde(rename = "windsurf.apiServerUrl")]
    windsurf: Option<String>,
    #[serde(rename = "devin.apiServerUrl")]
    devin: Option<String>,
    #[serde(rename = "codeium.apiServerUrl")]
    codeium: Option<String>,
}
impl Drop for Context {
    fn drop(&mut self) {
        for s in [
            &mut self.stored,
            &mut self.windsurf,
            &mut self.devin,
            &mut self.codeium,
        ]
        .into_iter()
        .flatten()
        {
            zeroize::Zeroize::zeroize(s)
        }
    }
}
pub(super) fn strip_jsonc(input: &[u8]) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let mut out = Zeroizing::new(Vec::with_capacity(input.len()));
    let (mut i, mut quoted, mut escaped) = (0, false, false);
    while i < input.len() {
        let b = input[i];
        if quoted {
            out.push(b);
            if escaped {
                escaped = false
            } else if b == b'\\' {
                escaped = true
            } else if b == b'"' {
                quoted = false
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            quoted = true;
            out.push(b);
            i += 1;
            continue;
        }
        if b == b'/' && input.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < input.len() && input[i] != b'\n' {
                i += 1
            }
            out.push(b' ');
            continue;
        }
        if b == b'/' && input.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < input.len() && !(input[i] == b'*' && input[i + 1] == b'/') {
                i += 1
            }
            if i + 1 >= input.len() {
                return Err(invalid());
            }
            i += 2;
            out.push(b' ');
            continue;
        }
        out.push(b);
        i += 1;
    }
    let mut clean = Zeroizing::new(Vec::with_capacity(out.len()));
    quoted = false;
    escaped = false;
    for (i, &b) in out.iter().enumerate() {
        if quoted {
            clean.push(b);
            if escaped {
                escaped = false
            } else if b == b'\\' {
                escaped = true
            } else if b == b'"' {
                quoted = false
            }
            continue;
        }
        if b == b'"' {
            quoted = true
        }
        if b == b','
            && out[i + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| *b == b'}' || *b == b']')
        {
            continue;
        }
        clean.push(b)
    }
    Ok(clean)
}
fn check(bytes: &[u8]) -> Result<(), Fault> {
    let clean = strip_jsonc(bytes)?;
    let context:Context=serde_json::from_slice(&clean).map_err(|_|Fault::new(FaultKind::Unreadable,"Devin's deployment configuration could not be verified safely. No credential was sent."))?;
    for url in [
        &context.stored,
        &context.windsurf,
        &context.devin,
        &context.codeium,
    ]
    .into_iter()
    .flatten()
    {
        if !url.is_empty() && url.trim_end_matches('/') != "https://server.codeium.com" {
            return Err(denied("Devin desktop is configured for a different or unsupported API deployment. Its credential was not forwarded to the public cloud or an arbitrary host."));
        }
    }
    Ok(())
}
pub(super) fn verify(path: &Path) -> Result<(), Fault> {
    let user = path.parent().and_then(Path::parent).ok_or_else(invalid)?;
    let settings = user.join("settings.json");
    match std::fs::symlink_metadata(&settings) {
        Ok(_) => check(&raw_file(&settings)?)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(invalid()),
    }
    // First-party codeium.windsurf global state retains the login-selected server.
    match crate::source_db::read(path, "codeium.windsurf", 256 * 1024) {
        Ok(s) => check(s.as_bytes())?,
        Err(f) if f.kind == FaultKind::NotSignedIn => {}
        Err(f) => return Err(f),
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jsonc_preserves_https_strings_and_rejects_custom_tenants() {
        assert!(check(b"{ // public default\n\"devin.apiServerUrl\":\"https://server.codeium.com\",/* comment */\"unused\":[1,2,],}").is_ok());
        assert!(check(br#"{"apiServerUrl":"https://tenant.invalid"}"#).is_err());
        assert!(
            check(br#"{"windsurf.apiServerUrl":"https://server.codeium.com.evil.invalid"}"#)
                .is_err()
        );
        assert!(check(b"{/* incomplete").is_err());
        assert!(check(br#"{"apiServerUrl":17}"#).is_err());
    }
}
