//! A small JSON reader (std only), enough for the pack's `manifest.json`, its `data/*.json` and
//! the JSON chunk of a `.glb`. Objects keep their keys in file order.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

static NULL: Json = Json::Null;

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let bytes = text.trim_start_matches('\u{feff}').as_bytes();
        let mut at = 0;
        let value = value(bytes, &mut at)?;
        skip(bytes, &mut at);
        if at != bytes.len() {
            return Err(format!("JSON: unexpected text at byte {at}"));
        }
        Ok(value)
    }

    /// The member `key` of an object; `Null` when absent or not an object.
    pub fn get(&self, key: &str) -> &Json {
        match self {
            Json::Object(members) => members.iter().find(|(k, _)| k == key).map_or(&NULL, |(_, v)| v),
            _ => &NULL,
        }
    }

    /// Element `index` of an array; `Null` when absent.
    pub fn at(&self, index: usize) -> &Json {
        match self {
            Json::Array(items) => items.get(index).unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    pub fn items(&self) -> &[Json] {
        match self {
            Json::Array(items) => items,
            _ => &[],
        }
    }

    pub fn members(&self) -> &[(String, Json)] {
        match self {
            Json::Object(members) => members,
            _ => &[],
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|n| n as f32)
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_f64().filter(|n| *n >= 0.0 && n.fract() == 0.0).map(|n| n as usize)
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64().filter(|n| n.fract() == 0.0).map(|n| n as i64)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    /// An array of numbers as `f32`s (empty when it is not one).
    pub fn floats(&self) -> Vec<f32> {
        self.items().iter().filter_map(Json::as_f32).collect()
    }
}

fn skip(b: &[u8], at: &mut usize) {
    while *at < b.len() && matches!(b[*at], b' ' | b'\n' | b'\r' | b'\t') {
        *at += 1;
    }
}

fn value(b: &[u8], at: &mut usize) -> Result<Json, String> {
    skip(b, at);
    let Some(&c) = b.get(*at) else { return Err("JSON: unexpected end".to_string()) };
    match c {
        b'{' => {
            *at += 1;
            let mut members = Vec::new();
            loop {
                skip(b, at);
                if b.get(*at) == Some(&b'}') {
                    *at += 1;
                    return Ok(Json::Object(members));
                }
                let key = string(b, at)?;
                skip(b, at);
                if b.get(*at) != Some(&b':') {
                    return Err(format!("JSON: ':' expected at byte {at}"));
                }
                *at += 1;
                members.push((key, value(b, at)?));
                skip(b, at);
                match b.get(*at) {
                    Some(b',') => *at += 1,
                    Some(b'}') => {}
                    _ => return Err(format!("JSON: ',' or '}}' expected at byte {at}")),
                }
            }
        }
        b'[' => {
            *at += 1;
            let mut items = Vec::new();
            loop {
                skip(b, at);
                if b.get(*at) == Some(&b']') {
                    *at += 1;
                    return Ok(Json::Array(items));
                }
                items.push(value(b, at)?);
                skip(b, at);
                match b.get(*at) {
                    Some(b',') => *at += 1,
                    Some(b']') => {}
                    _ => return Err(format!("JSON: ',' or ']' expected at byte {at}")),
                }
            }
        }
        b'"' => Ok(Json::String(string(b, at)?)),
        b't' if b[*at..].starts_with(b"true") => {
            *at += 4;
            Ok(Json::Bool(true))
        }
        b'f' if b[*at..].starts_with(b"false") => {
            *at += 5;
            Ok(Json::Bool(false))
        }
        b'n' if b[*at..].starts_with(b"null") => {
            *at += 4;
            Ok(Json::Null)
        }
        _ => {
            let start = *at;
            while *at < b.len() && matches!(b[*at], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
                *at += 1;
            }
            std::str::from_utf8(&b[start..*at]).ok().and_then(|s| s.parse::<f64>().ok()).map(Json::Number).ok_or_else(|| format!("JSON: a value expected at byte {start}"))
        }
    }
}

fn string(b: &[u8], at: &mut usize) -> Result<String, String> {
    if b.get(*at) != Some(&b'"') {
        return Err(format!("JSON: a string expected at byte {at}"));
    }
    *at += 1;
    let mut out = Vec::new();
    loop {
        let Some(&c) = b.get(*at) else { return Err("JSON: unterminated string".to_string()) };
        *at += 1;
        match c {
            b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
            b'\\' => {
                let Some(&e) = b.get(*at) else { return Err("JSON: unterminated escape".to_string()) };
                *at += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'u' => {
                        let hex = b.get(*at..*at + 4).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u32::from_str_radix(h, 16).ok()).ok_or("JSON: bad \\u escape")?;
                        *at += 4;
                        let mut buffer = [0u8; 4];
                        out.extend_from_slice(char::from_u32(hex).unwrap_or('?').encode_utf8(&mut buffer).as_bytes());
                    }
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_values() {
        let j = Json::parse(r#"{"a": [1, -2.5e1, true, null, "x\"yA"], "b": {"c": 3}}"#).unwrap();
        assert_eq!(j.get("a").at(1).as_f32(), Some(-25.0));
        assert_eq!(j.get("a").at(4).as_str(), Some("x\"yA"));
        assert_eq!(j.get("b").get("c").as_usize(), Some(3));
        assert!(j.get("zz").is_null() && j.get("a").at(3).is_null());
        assert!(Json::parse("{").is_err() && Json::parse("[1,]x").is_err());
    }
}
