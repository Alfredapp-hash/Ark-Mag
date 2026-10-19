//! Ark-Mag core: magnet URI validation (step 2 of the add-download workflow).

use std::fmt;

/// Reasons a magnet URI can be rejected, so the UI can explain the problem.
#[derive(Debug, PartialEq, Eq)]
pub enum MagnetError {
    NotMagnet,
    MissingInfoHash,
    InvalidInfoHash,
}

impl fmt::Display for MagnetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotMagnet => "not a magnet link (must start with \"magnet:?\")",
            Self::MissingInfoHash => "magnet link has no info hash (xt=urn:btih:...)",
            Self::InvalidInfoHash => "magnet link has an invalid info hash",
        })
    }
}

impl std::error::Error for MagnetError {}

/// A validated magnet link.
#[derive(Debug, PartialEq, Eq)]
pub struct Magnet {
    /// Info hash, lowercase hex (40 chars).
    pub info_hash: String,
    pub display_name: Option<String>,
    pub trackers: Vec<String>,
}

fn base32_to_hex(s: &str) -> Option<String> {
    let (mut bits, mut nbits, mut out) = (0u32, 0, Vec::new());
    for c in s.to_ascii_uppercase().chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            '2'..='7' => c as u32 - '2' as u32 + 26,
            _ => return None,
        };
        bits = (bits << 5) | v;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
            bits &= (1 << nbits) - 1;
        }
    }
    Some(out.iter().map(|b| format!("{b:02x}")).collect())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse and validate a magnet URI.
pub fn parse_magnet(uri: &str) -> Result<Magnet, MagnetError> {
    let uri = uri.trim();
    let query = match uri.get(..8) {
        Some(p) if p.eq_ignore_ascii_case("magnet:?") => &uri[8..],
        _ => return Err(MagnetError::NotMagnet),
    };
    let (mut hash, mut name, mut trackers) = (None, None, Vec::new());
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = percent_decode(v);
        match k {
            "xt" => {
                if let Some(h) = v.strip_prefix("urn:btih:") {
                    let hex = match h.len() {
                        40 if h.chars().all(|c| c.is_ascii_hexdigit()) => h.to_ascii_lowercase(),
                        32 => base32_to_hex(h).ok_or(MagnetError::InvalidInfoHash)?,
                        _ => return Err(MagnetError::InvalidInfoHash),
                    };
                    hash.get_or_insert(hex);
                }
            }
            "dn" => name = Some(v),
            "tr" if !v.is_empty() => trackers.push(v),
            _ => {}
        }
    }
    Ok(Magnet {
        info_hash: hash.ok_or(MagnetError::MissingInfoHash)?,
        display_name: name,
        trackers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const H: &str = "c12fe1c06bba254a9dc9f519b335aa7c1367a88a";

    #[test]
    fn parses_hex() {
        let m = parse_magnet(&format!("magnet:?xt=urn:btih:{H}&dn=My+File%21&tr=udp%3A%2F%2Ft.example%3A1")).unwrap();
        assert_eq!(m.info_hash, H);
        assert_eq!(m.display_name.as_deref(), Some("My File!"));
        assert_eq!(m.trackers, vec!["udp://t.example:1"]);
    }

    #[test]
    fn parses_base32() {
        let m = parse_magnet("magnet:?xt=urn:btih:YEX6DQDLXISUVHOJ6UM3GNNKPQJWPKEK").unwrap();
        assert_eq!(m.info_hash, H);
    }

    #[test]
    fn rejects_invalid() {
        assert_eq!(parse_magnet("http://x"), Err(MagnetError::NotMagnet));
        assert_eq!(parse_magnet("magnet:?dn=x"), Err(MagnetError::MissingInfoHash));
        assert_eq!(parse_magnet("magnet:?xt=urn:btih:zz"), Err(MagnetError::InvalidInfoHash));
    }
}
