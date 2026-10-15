use crate::eval::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn has_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn to_json_string(&self) -> String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            JsonValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e16 {
                    format!("{:.0}", n)
                } else {
                    format!("{}", n)
                }
            }
            JsonValue::String(s) => escape_json_string(s),
            JsonValue::Array(items) => {
                let inner = items
                    .iter()
                    .map(|v| v.to_json_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{}]", inner)
            }
            JsonValue::Object(pairs) => {
                let inner = pairs
                    .iter()
                    .map(|(k, v)| format!("{}:{}", escape_json_string(k), v.to_json_string()))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{}}}", inner)
            }
        }
    }

    pub fn to_pretty_string(&self, indent: usize) -> String {
        let mut out = String::new();
        self.pretty_helper(&mut out, indent, 0);
        out
    }

    fn pretty_helper(&self, out: &mut String, indent: usize, current_level: usize) {
        let pad = " ".repeat(indent * current_level);
        let inner_pad = " ".repeat(indent * (current_level + 1));

        match self {
            JsonValue::Null => out.push_str("null"),
            JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JsonValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e16 {
                    out.push_str(&format!("{:.0}", n));
                } else {
                    out.push_str(&format!("{}", n));
                }
            }
            JsonValue::String(s) => out.push_str(&escape_json_string(s)),
            JsonValue::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (i, it) in items.iter().enumerate() {
                    out.push_str(&inner_pad);
                    it.pretty_helper(out, indent, current_level + 1);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push(']');
            }
            JsonValue::Object(pairs) => {
                if pairs.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                for (i, (k, v)) in pairs.iter().enumerate() {
                    out.push_str(&inner_pad);
                    out.push_str(&escape_json_string(k));
                    out.push_str(": ");
                    v.pretty_helper(out, indent, current_level + 1);
                    if i + 1 < pairs.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push('}');
            }
        }
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_json_string())
    }
}

pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn value_to_json(val: &Value) -> String {
    match val {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e16 {
                format!("{:.1}", f)
            } else {
                format!("{}", f)
            }
        }
        Value::String(s) => escape_json_string(s),
        Value::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
        Value::Unit => "null".to_string(),
        Value::List(items) => {
            let lock = items.lock().unwrap();
            let inner = lock.iter().map(value_to_json).collect::<Vec<_>>().join(",");
            format!("[{}]", inner)
        }
        Value::Struct { fields, .. } => {
            let inner = fields
                .iter()
                .map(|(k, v)| format!("{}:{}", escape_json_string(k), value_to_json(v)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{}}}", inner)
        }
        Value::ErrorUnion(Ok(v)) => value_to_json(v),
        Value::ErrorUnion(Err(e)) => format!("{{\"kesalahan\":{}}}", value_to_json(e)),
        Value::Task(_) => "\"<Tugas>\"".to_string(),
    }
}

// --- Recursive Descent JSON Parser ---

pub struct JsonParser<'a> {
    chars: Vec<char>,
    pos: usize,
    _marker: std::marker::PhantomData<&'a str>,
}

impl<'a> JsonParser<'a> {
    pub fn new(input: &str) -> Self {
        JsonParser {
            chars: input.chars().collect(),
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn parse(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        let val = self.parse_value()?;
        self.skip_whitespace();
        if self.pos < self.chars.len() {
            return Err(format!(
                "Karakter tak terduga setelah JSON di indeks {}: '{}'",
                self.pos, self.chars[self.pos]
            ));
        }
        Ok(val)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('t') | Some('f') => self.parse_bool(),
            Some('n') => self.parse_null(),
            Some('-') | Some('0'..='9') => self.parse_number(),
            Some(ch) => Err(format!("Token JSON tidak valid: '{}' di indeks {}", ch, self.pos)),
            None => Err("Input JSON kosong".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.advance(); // consume '{'
        self.skip_whitespace();

        let mut pairs = Vec::new();
        if let Some('}') = self.peek() {
            self.advance();
            return Ok(JsonValue::Object(pairs));
        }

        loop {
            self.skip_whitespace();
            if self.peek() != Some('"') {
                return Err(format!("Diharapkan string kunci objek di indeks {}", self.pos));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();

            if self.advance() != Some(':') {
                return Err(format!("Diharapkan ':' setelah kunci '{}' di indeks {}", key, self.pos));
            }

            self.skip_whitespace();
            let val = self.parse_value()?;
            pairs.push((key, val));

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                Some(ch) => return Err(format!("Diharapkan ',' atau '}}' di objek, menemukan '{}' di indeks {}", ch, self.pos)),
                None => return Err("Objek JSON tidak ditutup dengan '}'".to_string()),
            }
        }

        Ok(JsonValue::Object(pairs))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.advance(); // consume '['
        self.skip_whitespace();

        let mut items = Vec::new();
        if let Some(']') = self.peek() {
            self.advance();
            return Ok(JsonValue::Array(items));
        }

        loop {
            self.skip_whitespace();
            let val = self.parse_value()?;
            items.push(val);

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some(']') => {
                    self.advance();
                    break;
                }
                Some(ch) => return Err(format!("Diharapkan ',' atau ']' di array, menemukan '{}' di indeks {}", ch, self.pos)),
                None => return Err("Array JSON tidak ditutup dengan ']'".to_string()),
            }
        }

        Ok(JsonValue::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.advance(); // consume opening quote '"'
        let mut s = String::new();

        while let Some(ch) = self.advance() {
            match ch {
                '"' => return Ok(s),
                '\\' => {
                    match self.advance() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('b') => s.push('\x08'),
                        Some('f') => s.push('\x0C'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                if let Some(h) = self.advance() {
                                    hex.push(h);
                                } else {
                                    return Err("Escape unicode \\u tidak lengkap".to_string());
                                }
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| format!("Escape unicode tidak valid: \\u{}", hex))?;
                            if let Some(unicode_char) = char::from_u32(code) {
                                s.push(unicode_char);
                            } else {
                                return Err(format!("Karakter unicode tidak valid: \\u{}", hex));
                            }
                        }
                        Some(other) => return Err(format!("Karakter escape tidak valid: \\{}", other)),
                        None => return Err("String JSON terpotong pada escape".to_string()),
                    }
                }
                c => s.push(c),
            }
        }

        Err("String JSON tidak ditutup dengan tanda kutip ganda".to_string())
    }

    fn parse_bool(&mut self) -> Result<JsonValue, String> {
        let rest: String = self.chars[self.pos..].iter().collect();
        if rest.starts_with("true") {
            self.pos += 4;
            Ok(JsonValue::Bool(true))
        } else if rest.starts_with("false") {
            self.pos += 5;
            Ok(JsonValue::Bool(false))
        } else {
            Err(format!("Nilai boolean tidak valid di indeks {}", self.pos))
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, String> {
        let rest: String = self.chars[self.pos..].iter().collect();
        if rest.starts_with("null") {
            self.pos += 4;
            Ok(JsonValue::Null)
        } else {
            Err(format!("Nilai null tidak valid di indeks {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        if let Some('-') = self.peek() {
            self.advance();
        }

        while let Some('0'..='9') = self.peek() {
            self.advance();
        }

        if let Some('.') = self.peek() {
            self.advance();
            while let Some('0'..='9') = self.peek() {
                self.advance();
            }
        }

        if let Some('e') | Some('E') = self.peek() {
            self.advance();
            if let Some('+') | Some('-') = self.peek() {
                self.advance();
            }
            while let Some('0'..='9') = self.peek() {
                self.advance();
            }
        }

        let num_str: String = self.chars[start..self.pos].iter().collect();
        num_str
            .parse::<f64>()
            .map(JsonValue::Number)
            .map_err(|e| format!("Gagal mengurai angka '{}' di indeks {}: {}", num_str, start, e))
    }
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(input);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_primitives() {
        assert_eq!(parse_json("123.45").unwrap(), JsonValue::Number(123.45));
        assert_eq!(parse_json("\"halo dunia\"").unwrap(), JsonValue::String("halo dunia".to_string()));
        assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
        assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn test_parse_json_object_and_array() {
        let raw = r#"{"id": 42, "nama": "DapCode", "skor": [95.5, 100], "aktif": true}"#;
        let parsed = parse_json(raw).expect("JSON should parse");
        assert_eq!(parsed.get("id"), Some(&JsonValue::Number(42.0)));
        assert_eq!(parsed.get("nama"), Some(&JsonValue::String("DapCode".to_string())));
        assert_eq!(parsed.get("aktif"), Some(&JsonValue::Bool(true)));

        if let Some(JsonValue::Array(items)) = parsed.get("skor") {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], JsonValue::Number(95.5));
        } else {
            panic!("skor should be an array");
        }
    }
}
