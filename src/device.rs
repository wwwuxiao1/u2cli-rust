use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::json::{self, Value};

/// 通过系统 adb 访问到的一台设备。
pub struct Phone {
    pub serial: String,
    local_port: u16,
}

impl Phone {
    /// 选择设备并建立到手机端服务的端口转发。
    pub fn connect(serial: Option<&str>, device_port: u16) -> Result<Self, String> {
        let serial = match serial {
            Some(value) => value.to_string(),
            None => default_serial()?,
        };
        let device_port_text = format!("tcp:{device_port}");
        let output = adb(&["-s", &serial, "forward", "tcp:0", &device_port_text])?;
        let local_port = output.trim().parse::<u16>().map_err(|_| format!("adb forward returned '{output}'"))?;
        Ok(Phone { serial, local_port })
    }

    /// 调用手机端 JSON-RPC 方法。
    pub fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"{method}\",\"params\":{}}}", json::stringify(params));
        let response = http_post(self.local_port, "/jsonrpc/0", &body)?;
        let value = json::parse(&response)?;
        if let Some(error) = json::object_get(&value, "error") {
            return Err(error_message(error));
        }
        json::object_get(&value, "result").cloned().ok_or_else(|| "phone response has no result".to_string())
    }

    /// 在设备上执行 shell，默认最多等待 60 秒。
    pub fn shell(&self, command: &str) -> Result<(String, i32), String> {
        self.shell_within(command, 60)
    }

    /// 在设备上执行 shell，并在超时后结束命令。
    pub fn shell_within(&self, command: &str, timeout_secs: u64) -> Result<(String, i32), String> {
        let mut child = Command::new("adb")
            .args(["-s", &self.serial, "shell", command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("adb shell failed: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs.max(1));
        loop {
            if let Some(status) = child.try_wait().map_err(|error| format!("adb shell failed: {error}"))? {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_string(&mut stdout).ok();
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_string(&mut stderr).ok();
                }
                if !status.success() && stdout.trim().is_empty() {
                    return Err(format!("adb shell failed: {stderr}"));
                }
                return Ok((stdout, status.code().unwrap_or(0)));
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return Err(format!("shell timed out after {timeout_secs}s"));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// 读取设备属性。
    pub fn getprop(&self, name: &str) -> Result<String, String> {
        let (output, code) = self.shell(&format!("getprop {name}"))?;
        if code != 0 {
            return Err(format!("getprop {name} failed"));
        }
        Ok(output.trim().to_string())
    }
}

/// 从 adb devices 中取出唯一在线设备。
fn default_serial() -> Result<String, String> {
    let output = adb(&["devices"])?;
    let serials: Vec<&str> = output.lines().filter_map(|line| {
        let mut parts = line.split_whitespace();
        let serial = parts.next()?;
        let state = parts.next()?;
        if state == "device" && serial != "List" {
            Some(serial)
        } else {
            None
        }
    }).collect();
    match serials.as_slice() {
        [serial] => Ok((*serial).to_string()),
        [] => Err("no device found".to_string()),
        _ => Err("more than one device, pass -s SERIAL".to_string()),
    }
}

/// 运行 adb 并返回标准输出。
fn adb(args: &[&str]) -> Result<String, String> {
    let output = Command::new("adb").args(args).output().map_err(|error| format!("adb is not available: {error}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("adb failed: {err}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 向本机转发端口发送 HTTP POST。
fn http_post(port: u16, path: &str, body: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).map_err(|error| format!("phone service is not reachable: {error}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let request = format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).map_err(|error| format!("phone request failed: {error}"))?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| format!("phone response failed: {error}"))?;
    response.split("\r\n\r\n").nth(1).map(|body| body.trim().to_string()).ok_or_else(|| "invalid phone response".to_string())
}

/// 从 JSON-RPC 错误对象取出说明。
fn error_message(error: &Value) -> String {
    json::object_text(error, "message").unwrap_or("phone request failed").to_string()
}
