//! Minimal JSON: a value tree that keeps key order and the int / float
//! distinction, a parser, and output formatted like Python's
//! `json.dump(value, indent=n)` (the format the N64KR Python tools wrote, so
//! files stay comparable): floats as Python's repr, ASCII-only strings.

use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Sets `key` (replacing its value in place, or appending it).
    pub fn set(&mut self, key: &str, value: Value) {
        if let Value::Obj(items) = self {
            match items.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => items.push((key.to_string(), value)),
            }
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The text Python's `json.dumps(value, indent=indent)` gives.
    pub fn to_string_indent(&self, indent: usize) -> String {
        let mut out = String::new();
        write_value(&mut out, self, indent, 0);
        out
    }
}

/// An object from (key, value) pairs.
pub fn obj(items: Vec<(&str, Value)>) -> Value {
    Value::Obj(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

pub fn floats(v: &[f64]) -> Value {
    Value::Arr(v.iter().map(|&f| Value::Float(f)).collect())
}

pub fn ints(v: &[i64]) -> Value {
    Value::Arr(v.iter().map(|&i| Value::Int(i)).collect())
}

/// Python's repr() of a float: the shortest digits that read back exactly,
/// fixed notation for 1e-4 <= |x| < 1e16 (with ".0" when integral), else
/// d.ddde+XX.
pub fn py_float(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let sci = format!("{:e}", x.abs()); // shortest round-trip digits, e.g. 3.4028234663852886e38
    let (mant, exp) = sci.split_once('e').expect("{:e} has an exponent");
    let exp: i32 = exp.parse().expect("exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    if (-4..16).contains(&exp) {
        if exp >= 0 {
            let int_len = exp as usize + 1;
            let (int_part, frac) = if digits.len() > int_len {
                (digits[..int_len].to_string(), digits[int_len..].to_string())
            } else {
                (format!("{digits}{}", "0".repeat(int_len - digits.len())), "0".to_string())
            };
            format!("{sign}{int_part}.{frac}")
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
        }
    } else {
        let m = if digits.len() > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits };
        format!("{sign}{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}

/// Python's round(x, ndigits): the double nearest the correctly rounded decimal.
pub fn py_round(x: f64, ndigits: usize) -> f64 {
    format!("{:.*}", ndigits, x).parse().expect("formatted float parses")
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{:04x}", unit);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(out: &mut String, v: &Value, indent: usize, depth: usize) {
    let pad = |out: &mut String, d: usize| {
        out.push('\n');
        out.push_str(&" ".repeat(indent * d));
    };
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::Float(f) => out.push_str(&py_float(*f)),
        Value::Str(s) => write_str(out, s),
        Value::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                pad(out, depth + 1);
                write_value(out, item, indent, depth + 1);
            }
            pad(out, depth);
            out.push(']');
        }
        Value::Obj(items) => {
            if items.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, item)) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                pad(out, depth + 1);
                write_str(out, k);
                out.push_str(": ");
                write_value(out, item, indent, depth + 1);
            }
            pad(out, depth);
            out.push('}');
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser { s: text.as_bytes(), pos: 0 };
    let v = p.value()?;
    p.ws();
    if p.pos != p.s.len() {
        return Err(format!("JSON: trailing data at byte {}", p.pos));
    }
    Ok(v)
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.pos < self.s.len() && matches!(self.s[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
            self.pos += 1;
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("JSON: {what} at byte {}", self.pos))
    }

    fn lit(&mut self, word: &str, v: Value) -> Result<Value, String> {
        if self.s[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(v)
        } else {
            self.err("bad literal")
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.ws();
        match self.s.get(self.pos) {
            None => self.err("unexpected end"),
            Some(b'{') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.ws();
                if self.s.get(self.pos) == Some(&b'}') {
                    self.pos += 1;
                    return Ok(Value::Obj(items));
                }
                loop {
                    self.ws();
                    let key = match self.value()? {
                        Value::Str(k) => k,
                        _ => return self.err("object key is not a string"),
                    };
                    self.ws();
                    if self.s.get(self.pos) != Some(&b':') {
                        return self.err("expected ':'");
                    }
                    self.pos += 1;
                    let v = self.value()?;
                    items.push((key, v));
                    self.ws();
                    match self.s.get(self.pos) {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Value::Obj(items));
                        }
                        _ => return self.err("expected ',' or '}'"),
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.ws();
                if self.s.get(self.pos) == Some(&b']') {
                    self.pos += 1;
                    return Ok(Value::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    self.ws();
                    match self.s.get(self.pos) {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Value::Arr(items));
                        }
                        _ => return self.err("expected ',' or ']'"),
                    }
                }
            }
            Some(b'"') => {
                self.pos += 1;
                let mut out = String::new();
                loop {
                    match self.s.get(self.pos) {
                        None => return self.err("unterminated string"),
                        Some(b'"') => {
                            self.pos += 1;
                            return Ok(Value::Str(out));
                        }
                        Some(b'\\') => {
                            let c = *self.s.get(self.pos + 1).ok_or("JSON: bad escape")?;
                            self.pos += 2;
                            match c {
                                b'"' => out.push('"'),
                                b'\\' => out.push('\\'),
                                b'/' => out.push('/'),
                                b'b' => out.push('\u{8}'),
                                b'f' => out.push('\u{c}'),
                                b'n' => out.push('\n'),
                                b'r' => out.push('\r'),
                                b't' => out.push('\t'),
                                b'u' => {
                                    let hex = |p: &Parser, at: usize| -> Result<u32, String> {
                                        std::str::from_utf8(p.s.get(at..at + 4).ok_or("JSON: bad \\u")?)
                                            .ok()
                                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                                            .ok_or_else(|| "JSON: bad \\u".to_string())
                                    };
                                    let mut code = hex(self, self.pos)?;
                                    self.pos += 4;
                                    if (0xD800..0xDC00).contains(&code) && self.s[self.pos..].starts_with(b"\\u") {
                                        let low = hex(self, self.pos + 2)?;
                                        self.pos += 6;
                                        code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                    }
                                    out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                                }
                                _ => return self.err("bad escape"),
                            }
                        }
                        Some(_) => {
                            let start = self.pos;
                            while self.pos < self.s.len() && self.s[self.pos] != b'"' && self.s[self.pos] != b'\\' {
                                self.pos += 1;
                            }
                            out.push_str(std::str::from_utf8(&self.s[start..self.pos]).map_err(|e| e.to_string())?);
                        }
                    }
                }
            }
            Some(b't') => self.lit("true", Value::Bool(true)),
            Some(b'f') => self.lit("false", Value::Bool(false)),
            Some(b'n') => self.lit("null", Value::Null),
            Some(b'N') => self.lit("NaN", Value::Float(f64::NAN)),
            Some(b'I') => self.lit("Infinity", Value::Float(f64::INFINITY)),
            Some(_) => {
                let start = self.pos;
                if self.s[self.pos..].starts_with(b"-Infinity") {
                    self.pos += 9;
                    return Ok(Value::Float(f64::NEG_INFINITY));
                }
                while self.pos < self.s.len() && matches!(self.s[self.pos], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.pos += 1;
                }
                let text = std::str::from_utf8(&self.s[start..self.pos]).map_err(|e| e.to_string())?;
                if text.contains(['.', 'e', 'E']) {
                    text.parse().map(Value::Float).or_else(|_| self.err("bad number"))
                } else {
                    text.parse().map(Value::Int).or_else(|_| self.err("bad number"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_float_repr() {
        for (x, want) in [
            (100.0, "100.0"),
            (0.5, "0.5"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (1e16, "1e+16"),
            (3.4028234663852886e38, "3.4028234663852886e+38"),
            (-2.25, "-2.25"),
            (0.2199999988079071, "0.2199999988079071"),
            (123456789012345.6, "123456789012345.6"),
        ] {
            assert_eq!(py_float(x), want);
        }
    }

    #[test]
    fn round_trip() {
        let text = "{\n \"a\": [\n  1,\n  2.5\n ],\n \"b\": {},\n \"c\": null\n}";
        assert_eq!(parse(text).unwrap().to_string_indent(1), text);
    }
}
