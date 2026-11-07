/// 把手机端返回的界面 XML 压成一行一个控件的文本。
pub fn hierarchy_to_text(xml: &str) -> Result<String, String> {
    let root = parse_xml(xml)?;
    let mut lines = Vec::new();
    render_node(&root, &mut lines, 0);
    Ok(lines.join("\n"))
}

struct Node {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

/// 解析测试和手机端使用的简单 XML。
fn parse_xml(input: &str) -> Result<Node, String> {
    let mut parser = XmlParser { text: input, index: 0 };
    parser.skip_prolog();
    parser.parse_element()
}

/// 按现有压缩规则渲染一个节点。
fn render_node(node: &Node, lines: &mut Vec<String>, depth: usize) {
    if is_invisible(node) {
        return;
    }
    if node.tag != "hierarchy" && !has_content(node) && !is_interactive(node) && node.children.len() == 1 {
        render_node(&node.children[0], lines, depth);
        return;
    }
    let mut parts = Vec::new();
    let class_name = attr(node, "class").unwrap_or(node.tag.as_str());
    if !class_name.is_empty() && class_name != "hierarchy" {
        parts.push(class_name.to_string());
    }
    let text = attr(node, "text").unwrap_or("").trim();
    if !text.is_empty() {
        parts.push(format!("\"{text}\""));
    }
    let desc = attr(node, "content-desc").unwrap_or("").trim();
    if !desc.is_empty() && desc != text {
        parts.push(format!("desc=\"{desc}\""));
    }
    let resource_id = attr(node, "resource-id").unwrap_or("").trim();
    if !resource_id.is_empty() {
        parts.push(format!("#{resource_id}"));
    }
    if let Some(bounds) = attr(node, "bounds") {
        if let Some(formatted) = format_bounds(bounds) {
            parts.push(formatted);
        }
    }
    let mut flags = Vec::new();
    if attr(node, "clickable") == Some("true") { flags.push("click"); }
    if attr(node, "scrollable") == Some("true") { flags.push("scroll"); }
    if attr(node, "checked") == Some("true") { flags.push("checked"); }
    if attr(node, "focused") == Some("true") { flags.push("focused"); }
    if attr(node, "selected") == Some("true") { flags.push("selected"); }
    if attr(node, "enabled") == Some("false") { flags.push("disabled"); }
    if !flags.is_empty() {
        parts.push(flags.join(" "));
    }
    let child_depth = if parts.is_empty() {
        depth
    } else {
        lines.push(format!("{}{}", "  ".repeat(depth), parts.join(" ")));
        depth + 1
    };
    for child in &node.children {
        render_node(child, lines, child_depth);
    }
}

/// 零面积或未显示的节点不进入压缩结果。
fn is_invisible(node: &Node) -> bool {
    if attr(node, "displayed") == Some("false") {
        return true;
    }
    attr(node, "bounds").map(|bounds| bounds_area(bounds) == 0).unwrap_or(false)
}

/// 节点是否带有可见文本或资源标识。
fn has_content(node: &Node) -> bool {
    attr(node, "text").unwrap_or("").trim() != ""
        || attr(node, "content-desc").unwrap_or("").trim() != ""
        || attr(node, "resource-id").unwrap_or("").trim() != ""
}

/// 节点是否可点击或可滚动。
fn is_interactive(node: &Node) -> bool {
    attr(node, "clickable") == Some("true") || attr(node, "scrollable") == Some("true")
}

/// 读取节点属性。
fn attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    node.attrs.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
}

/// 计算 bounds 面积，无法识别时视为 0。
fn bounds_area(bounds: &str) -> i64 {
    parse_bounds(bounds).map(|(x1, y1, x2, y2)| (x2 - x1).max(0) * (y2 - y1).max(0)).unwrap_or(0)
}

/// 把 bounds 格式化成压缩文本里的坐标。
fn format_bounds(bounds: &str) -> Option<String> {
    parse_bounds(bounds).map(|(x1, y1, x2, y2)| format!("[{x1},{y1},{x2},{y2}]"))
}

/// 解析 [x1,y1][x2,y2]。
fn parse_bounds(bounds: &str) -> Option<(i64, i64, i64, i64)> {
    let trimmed = bounds.trim();
    let rest = trimmed.strip_prefix('[')?;
    let (x1, rest) = rest.split_once(',')?;
    let (y1, rest) = rest.split_once("][")?;
    let (x2, y2) = rest.strip_suffix(']')?.split_once(',')?;
    Some((x1.parse().ok()?, y1.parse().ok()?, x2.parse().ok()?, y2.parse().ok()?))
}

struct XmlParser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> XmlParser<'a> {
    /// 跳过 XML 声明。
    fn skip_prolog(&mut self) {
        self.skip_ws();
        if self.text[self.index..].starts_with("<?") {
            if let Some(end) = self.text[self.index..].find("?>") {
                self.index += end + 2;
            }
        }
    }

    /// 解析一个元素及其子节点。
    fn parse_element(&mut self) -> Result<Node, String> {
        self.skip_ws();
        if !self.starts_with("<") {
            return Err("expected element".to_string());
        }
        self.index += 1;
        let tag = self.read_name()?;
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            if self.starts_with("/>") {
                self.index += 2;
                return Ok(Node { tag, attrs, children: Vec::new() });
            }
            if self.starts_with(">") {
                self.index += 1;
                break;
            }
            let name = self.read_name()?;
            self.skip_ws();
            if !self.starts_with("=") {
                return Err("expected attribute".to_string());
            }
            self.index += 1;
            self.skip_ws();
            attrs.push((name, self.read_attr()?));
        }
        let mut children = Vec::new();
        loop {
            self.skip_ws();
            if self.starts_with("</") {
                self.index += 2;
                let end = self.read_name()?;
                if end != tag {
                    return Err("mismatched tag".to_string());
                }
                self.skip_ws();
                if !self.starts_with(">") {
                    return Err("expected tag end".to_string());
                }
                self.index += 1;
                return Ok(Node { tag, attrs, children });
            }
            if self.starts_with("<") {
                children.push(self.parse_element()?);
            } else if self.index >= self.text.len() {
                return Err("unterminated element".to_string());
            } else if self.next_char().is_none() {
                return Err("unterminated element".to_string());
            }
        }
    }

    /// 读取标签或属性名。
    fn read_name(&mut self) -> Result<String, String> {
        let start = self.index;
        while let Some(ch) = self.next_char() {
            if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ':') {
                self.index -= ch.len_utf8();
                break;
            }
        }
        if start == self.index {
            return Err("expected name".to_string());
        }
        Ok(self.text[start..self.index].to_string())
    }

    /// 读取引号包裹的属性值。
    fn read_attr(&mut self) -> Result<String, String> {
        let quote = self.next_char().ok_or_else(|| "expected quote".to_string())?;
        if quote != '"' && quote != '\'' {
            return Err("expected quote".to_string());
        }
        let start = self.index;
        loop {
            let Some(ch) = self.next_char() else {
                return Err("unterminated attribute".to_string());
            };
            if ch == quote {
                let end = self.index - quote.len_utf8();
                return Ok(self.text[start..end].to_string());
            }
        }
    }

    /// 按字符前进，避免切到 UTF-8 中间字节。
    fn next_char(&mut self) -> Option<char> {
        let ch = self.text[self.index..].chars().next()?;
        self.index += ch.len_utf8();
        Some(ch)
    }

    /// 跳过空白。
    fn skip_ws(&mut self) {
        while let Some(ch) = self.text[self.index..].chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            self.index += ch.len_utf8();
        }
    }

    /// 判断剩余文本是否以指定前缀开始。
    fn starts_with(&self, prefix: &str) -> bool {
        self.text[self.index..].starts_with(prefix)
    }
}
