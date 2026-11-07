use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::json::{self, Value};

/// 本机服务的可见状态。
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub pid: Option<u32>,
    pub device_serial: Option<String>,
}

/// 在前台监听，直到收到停止请求。
pub fn serve(host: &str, port: u16, device_serial: Option<String>) -> Result<(), String> {
    let listener = TcpListener::bind(format!("{host}:{port}")).map_err(|error| format!("cannot listen on {host}:{port}: {error}"))?;
    write_state(&Status { running: true, host: host.to_string(), port, pid: Some(std::process::id()), device_serial: device_serial.clone() })?;
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        let stop = handle_client(&mut stream, host, port, device_serial.as_deref());
        if stop {
            let _ = fs::remove_file(state_path(port));
            break;
        }
    }
    Ok(())
}

/// 启动本机服务。已经在监听时不重复启动。
pub fn start(host: &str, port: u16, device_serial: Option<String>) -> Result<bool, String> {
    if probe(host, port).is_some() {
        return Ok(false);
    }
    let exe = std::env::current_exe().map_err(|error| format!("cannot locate u2cli: {error}"))?;
    let mut command = Command::new(exe);
    command.args(["--server-host", host, "--server-port", &port.to_string()]);
    if let Some(serial) = &device_serial {
        command.args(["-s", serial]);
    }
    command.arg("server").arg("--foreground");
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map_err(|error| format!("cannot start server: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if probe(host, port).is_some() {
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("server did not start".to_string())
}

/// 查询本机服务状态。未运行时返回 running 为假。
pub fn status(host: &str, port: u16) -> Status {
    if let Some(value) = probe(host, port) {
        return Status {
            running: true,
            host: json::object_text(&value, "host").unwrap_or(host).to_string(),
            port: json::object_get(&value, "port").and_then(number_u16).unwrap_or(port),
            pid: json::object_get(&value, "pid").and_then(number_u32),
            device_serial: json::object_text(&value, "device_serial").map(str::to_string),
        };
    }
    Status { running: false, host: host.to_string(), port, pid: None, device_serial: None }
}

/// 停止本机服务。返回是否真的停掉了一个进程。
pub fn stop(host: &str, port: u16) -> Result<bool, String> {
    if probe(host, port).is_none() {
        return Ok(false);
    }
    let _ = http(host, port, "POST", "/stop", "");
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if probe(host, port).is_none() {
            let _ = fs::remove_file(state_path(port));
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("server did not stop".to_string())
}

/// 处理一个 HTTP 连接，返回是否应该停止服务。
fn handle_client(stream: &mut TcpStream, host: &str, port: u16, device_serial: Option<&str>) -> bool {
    let mut buffer = [0u8; 4096];
    let size = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..size]);
    let line = request.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    if method == "GET" && path == "/health" {
        let mut fields = vec![
            ("host".to_string(), Value::Text(host.to_string())),
            ("port".to_string(), Value::Number(port as f64)),
            ("pid".to_string(), Value::Number(std::process::id() as f64)),
        ];
        if let Some(serial) = device_serial {
            fields.push(("device_serial".to_string(), Value::Text(serial.to_string())));
        }
        let _ = write_http(stream, &json::stringify(&Value::Object(fields)));
        return false;
    }
    if method == "POST" && path == "/stop" {
        let _ = write_http(stream, "{\"ok\":true}");
        return true;
    }
    let _ = write_http(stream, "{\"ok\":false}");
    false
}

/// 请求健康检查。连不上时返回空。
fn probe(host: &str, port: u16) -> Option<Value> {
    let body = http(host, port, "GET", "/health", "").ok()?;
    json::parse(&body).ok()
}

/// 发送一个短 HTTP 请求并返回正文。
fn http(host: &str, port: u16, method: &str, path: &str, body: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(format!("{host}:{port}")).map_err(|error| error.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| error.to_string())?;
    response.split("\r\n\r\n").nth(1).map(|text| text.trim().to_string()).ok_or_else(|| "empty response".to_string())
}

/// 写回一段 JSON 响应。
fn write_http(stream: &mut TcpStream, body: &str) -> Result<(), String> {
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    stream.write_all(response.as_bytes()).map_err(|error| error.to_string())
}

/// 把状态写到临时文件，便于排查本机服务。
fn write_state(status: &Status) -> Result<(), String> {
    let path = state_path(status.port);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let serial = status.device_serial.clone().unwrap_or_default();
    let text = format!("{}\n{}\n{}\n{serial}\n", status.pid.unwrap_or(0), status.host, status.port);
    fs::write(path, text).map_err(|error| error.to_string())
}

/// 本机服务状态文件路径。
fn state_path(port: u16) -> PathBuf {
    std::env::temp_dir().join(format!("u2cli-{port}.state"))
}

/// 从 JSON 数字读取端口。
fn number_u16(value: &Value) -> Option<u16> {
    match value {
        Value::Number(number) => Some(*number as u16),
        _ => None,
    }
}

/// 从 JSON 数字读取进程号。
fn number_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(number) => Some(*number as u32),
        _ => None,
    }
}
