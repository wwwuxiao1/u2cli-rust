/// 把布尔值打印成与现有命令一致的 True 或 False。
pub fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// 按 Python 的默认浮点格式打印数字。
pub fn py_float(value: f64) -> String {
    if value.fract() == 0.0 && value.is_finite() {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

/// 用单引号包住字符串，供 u2_code 使用。
pub fn py_str(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// 组装成功时的标准输出。device_serial 和 u2_code 在前，其余字段随后。
pub fn success(device_serial: Option<&str>, u2_code: Option<&str>, fields: &[(&str, String)], body: Option<&str>) -> String {
    let mut lines = Vec::new();
    if let Some(serial) = device_serial {
        if !serial.is_empty() {
            lines.push(format!("device_serial: {serial}"));
        }
    }
    if let Some(code) = u2_code {
        lines.push(format!("u2_code: {code}"));
    }
    for (key, value) in fields {
        if *key == "device_serial" {
            continue;
        }
        lines.push(format!("{key}: {value}"));
    }
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    if let Some(body) = body {
        text.push_str(body);
        if !body.ends_with('\n') {
            text.push('\n');
        }
    }
    text
}
