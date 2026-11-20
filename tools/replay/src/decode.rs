//! Decodes recorded event JSON (the `stellar events --output json` shape
//! documented in `docs/EVENTS.md`) into [`Event`]s.

use crate::state::Event;

/// Decodes one JSON object per line (or a JSON array of objects).
pub fn decode_events(input: &str) -> Result<Vec<Event>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        let values = parse_array(trimmed)?;
        values.iter().map(decode_event).collect()
    } else {
        trimmed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| decode_event(&parse_object(line.trim())?))
            .collect()
    }
}

fn decode_event(obj: &[(String, Value)]) -> Result<Event, String> {
    let ledger = get(obj, "ledger")
        .and_then(Value::as_u32)
        .ok_or_else(|| "event missing numeric `ledger`".to_string())?;
    let topic = match get(obj, "prefix_topics") {
        Some(Value::Array(items)) => items
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| "empty `prefix_topics`".to_string())?
            .to_string(),
        _ => get(obj, "topic")
            .and_then(Value::as_str)
            .ok_or_else(|| "event missing `prefix_topics`".to_string())?
            .to_string(),
    };
    let params = match get(obj, "params") {
        Some(Value::Object(fields)) => fields,
        _ => return Err("event missing `params` object".to_string()),
    };
    let data = params
        .iter()
        .map(|(_, value)| value.as_field())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Event { ledger, topic, data })
}

fn get<'a>(obj: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    obj.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Num(String),
    Bool(bool),
    Null,
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    fn as_u32(&self) -> Option<u32> {
        match self {
            Value::Num(n) => n.parse().ok(),
            _ => None,
        }
    }

    /// Renders a data field the way the contract stores it.
    fn as_field(&self) -> Result<String, String> {
        match self {
            Value::Str(s) => Ok(s.clone()),
            Value::Num(n) => Ok(n.clone()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Err("null event data field".to_string()),
            Value::Array(_) | Value::Object(_) => {
                Err("nested event data field is not supported".to_string())
            }
        }
    }
}

fn parse_array(input: &str) -> Result<Vec<Value>, String> {
    match parse_value(input)? {
        Value::Array(items) => Ok(items),
        _ => Err("expected a JSON array".to_string()),
    }
}

fn parse_object(input: &str) -> Result<Vec<(String, Value)>, String> {
    match parse_value(input)? {
        Value::Object(fields) => Ok(fields),
        _ => Err("expected a JSON object".to_string()),
    }
}

fn parse_value(input: &str) -> Result<Value, String> {
    let mut parser = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(format!("trailing input at byte {}", parser.pos));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.pos) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", byte as char, self.pos))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::Str),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("unexpected input at byte {}", self.pos)),
        }
    }

    fn literal(&mut self, text: &str, value: Value) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(value)
        } else {
            Err(format!("invalid literal at byte {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        while let Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "invalid utf-8 in number".to_string())?;
        Ok(Value::Num(raw.to_string()))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".to_string()),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self.peek().ok_or("unterminated escape")?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.pos..self.pos + 4)
                                .ok_or("truncated \\u escape")?;
                            let hex = std::str::from_utf8(hex)
                                .map_err(|_| "invalid \\u escape".to_string())?;
                            let code = u32::from_str_radix(hex, 16)
                                .map_err(|_| "invalid \\u escape".to_string())?;
                            self.pos += 4;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        other => return Err(format!("invalid escape \\{}", other as char)),
                    }
                }
                Some(_) => {
                    let rest = std::str::from_utf8(&self.bytes[self.pos..])
                        .map_err(|_| "invalid utf-8 in string".to_string())?;
                    let ch = rest.chars().next().ok_or("unterminated string")?;
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(fields));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.expect(b':')?;
            let value = self.value()?;
            fields.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(fields));
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
    }
}
