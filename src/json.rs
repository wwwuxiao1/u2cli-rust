/// 手机端 JSON-RPC 用到的 JSON 值。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// 解析一段 JSON 文本。
pub fn parse(input: &str) -> Result<Value, String> {
    let mut parser = Parser { bytes: input.as_bytes(), index: 0 };
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.index != parser.bytes.len() {
        return Err("trailing json".to_string());
    }
    Ok(value)
}

/// 把 JSON 值编码成文本。
pub fn stringify(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => {
            if number.fract() == 0.0 && number.is_finite() {
                format!("{}", *number as i64)
            } else {
                format!("{number}")
            }
        }
        Value::Text(text) => format!("\"{}\"", escape(text)),
        Value::Array(items) => {
            let body = items.iter().map(stringify).collect::<Vec<_>>().join(",");
            format!("[{body}]")
        }
        Value::Object(fields) => {
            let body = fields.iter().map(|(key, value)| format!("\"{}\":{}", escape(key), stringify(value))).collect::<Vec<_>>().join(",");
            format!("{{{body}}}")
        }
    }
}

/// 转义 JSON 字符串。
fn escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> Parser<'a> {
    /// 解析任意 JSON 值。
    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        let byte = self.peek().ok_or_else(|| "unexpected end of json".to_string())?;
        match byte {
            b'n' => self.consume_literal("null").map(|_| Value::Null),
            b't' => self.consume_literal("true").map(|_| Value::Bool(true)),
            b'f' => self.consume_literal("false").map(|_| Value::Bool(false)),
            b'"' => self.parse_string().map(Value::Text),
            b'[' => self.parse_array(),
            b'{' => self.parse_object(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err("invalid json value".to_string()),
        }
    }

    /// 解析 JSON 对象。
    fn parse_object(&mut self) -> Result<Value, String> {
        self.bump();
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Value::Object(fields));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.bump() != Some(b':') {
                return Err("expected colon".to_string());
            }
            fields.push((key, self.parse_value()?));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(Value::Object(fields)),
                _ => return Err("expected object end".to_string()),
            }
        }
    }

    /// 解析 JSON 数组。
    fn parse_array(&mut self) -> Result<Value, String> {
        self.bump();
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(Value::Array(items)),
                _ => return Err("expected array end".to_string()),
            }
        }
    }

    /// 解析 JSON 字符串。
    fn parse_string(&mut self) -> Result<String, String> {
        if self.bump() != Some(b'"') {
            return Err("expected string".to_string());
        }
        let mut text = String::new();
        while self.index < self.bytes.len() {
            let byte = self.bytes[self.index];
            if byte == b'"' {
                self.index += 1;
                return Ok(text);
            }
            if byte == b'\\' {
                self.index += 1;
                let escaped = self.bump().ok_or_else(|| "bad escape".to_string())?;
                text.push(match escaped {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'u' => self.parse_unicode_escape()?,
                    other => other as char,
                });
                continue;
            }
            let rest = &self.bytes[self.index..];
            let ch = std::str::from_utf8(rest).ok().and_then(|s| s.chars().next()).ok_or_else(|| "invalid utf-8 in json string".to_string())?;
            text.push(ch);
            self.index += ch.len_utf8();
        }
        Err("unterminated string".to_string())
    }

    /// 解析 JSON 的 \\uXXXX 转义。
    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self.bump().ok_or_else(|| "bad unicode escape".to_string())?;
            let digit = (byte as char).to_digit(16).ok_or_else(|| "bad unicode escape".to_string())?;
            value = (value << 4) | digit;
        }
        char::from_u32(value).ok_or_else(|| "invalid unicode escape".to_string())
    }

    /// 解析 JSON 数字。
    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.index;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        while matches!(self.peek(), Some(b'0'..=b'9' | b'.')) {
            self.bump();
        }
        let text = std::str::from_utf8(&self.bytes[start..self.index]).map_err(|_| "bad number".to_string())?;
        text.parse::<f64>().map(Value::Number).map_err(|_| "bad number".to_string())
    }

    /// 消费一个固定单词。
    fn consume_literal(&mut self, literal: &str) -> Result<(), String> {
        for expected in literal.bytes() {
            if self.bump() != Some(expected) {
                return Err("invalid literal".to_string());
            }
        }
        Ok(())
    }

    /// 跳过空白。
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.bump();
        }
    }

    /// 看下一个字节。
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    /// 读取并前进一个字节。
    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }
}

/// 从对象里取出字符串字段。
pub fn object_text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    let Value::Object(fields) = value else { return None };
    fields.iter().find(|(name, _)| name == key).and_then(|(_, value)| match value {
        Value::Text(text) => Some(text.as_str()),
        _ => None,
    })
}

/// 从对象里取出任意字段。
pub fn object_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Object(fields) = value else { return None };
    fields.iter().find(|(name, _)| name == key).map(|(_, value)| value)
}
