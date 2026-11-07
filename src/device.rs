use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::json::{self, Value};

/// 内置的手机端 UiAutomator HTTP 服务。
const U2_JAR: &[u8] = include_bytes!("../assets/u2.jar");

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
        let phone = Phone { serial, local_port };
        phone.ensure_service(device_port)?;
        Ok(phone)
    }

    /// 确认手机端服务可访问。文件一致且已在响应时不推送、不重启。
    fn ensure_service(&self, device_port: u16) -> Result<(), String> {
        let running = ping(self.local_port);
        let pushed = self.install_jar()?;
        if running && !pushed {
            return Ok(());
        }
        if running {
            self.stop_service();
        }
        self.start_service(device_port)?;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if ping(self.local_port) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(200));
        }
        Err("phone service did not become ready".to_string())
    }

    /// 把内置 u2.jar 推到设备。返回是否发生了推送。
    fn install_jar(&self) -> Result<bool, String> {
        let digest = md5_hex(U2_JAR);
        let remote = adb_stdout(&["-s", &self.serial, "shell", "toybox", "md5sum", "/data/local/tmp/u2.jar"]);
        if remote.contains(&digest) {
            return Ok(false);
        }
        let path = std::env::temp_dir().join("u2cli-u2.jar");
        std::fs::write(&path, U2_JAR).map_err(|error| format!("cannot write bundled jar: {error}"))?;
        let local = path.display().to_string();
        adb(&["-s", &self.serial, "push", &local, "/data/local/tmp/u2.jar"])?;
        Ok(true)
    }

    /// 停掉设备上已有的手机端服务，便于换上新的 jar。
    fn stop_service(&self) {
        let _ = self.shell_within("pkill -f com.wetest.uia2.Main", 8);
    }

    /// 在设备上后台启动手机端 HTTP 服务。
    fn start_service(&self, device_port: u16) -> Result<(), String> {
        let command = format!("nohup sh -c 'CLASSPATH=/data/local/tmp/u2.jar app_process / com.wetest.uia2.Main -p {device_port}' >/data/local/tmp/u2-server.log 2>&1 &");
        let _ = self.shell_within(&command, 8);
        Ok(())
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
    /// 读取和等待同时进行，输出超过管道缓冲时进程仍能退出。
    pub fn shell_within(&self, command: &str, timeout_secs: u64) -> Result<(String, i32), String> {
        let mut child = Command::new("adb")
            .args(["-s", &self.serial, "shell", command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("adb shell failed: {error}"))?;
        let stdout_pipe = child.stdout.take().ok_or_else(|| "adb shell failed: missing stdout".to_string())?;
        let stderr_pipe = child.stderr.take().ok_or_else(|| "adb shell failed: missing stderr".to_string())?;
        let stdout_task = thread::spawn(move || read_pipe(stdout_pipe));
        let stderr_task = thread::spawn(move || read_pipe(stderr_pipe));
        let deadline = Instant::now() + Duration::from_secs(timeout_secs.max(1));
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_task.join();
                    let _ = stderr_task.join();
                    return Err(format!("shell timed out after {timeout_secs}s"));
                }
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(error) => return Err(format!("adb shell failed: {error}")),
            }
        };
        let stdout = stdout_task.join().unwrap_or_default();
        let stderr = stderr_task.join().unwrap_or_default();
        if !status.success() && stdout.trim().is_empty() {
            return Err(format!("adb shell failed: {stderr}"));
        }
        Ok((stdout, status.code().unwrap_or(0)))
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

/// 读完一条管道。调用方在进程运行时开始读，避免管道被写满。
fn read_pipe(mut pipe: impl Read) -> String {
    let mut text = String::new();
    let _ = pipe.read_to_string(&mut text);
    text
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

/// 检查手机端服务是否已经响应 ping。
fn ping(port: u16) -> bool {
    http_get(port, "/ping").map(|body| body.trim() == "pong").unwrap_or(false)
}

/// 向本机转发端口发送 HTTP GET。
fn http_get(port: u16, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).map_err(|error| error.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| error.to_string())?;
    response.split("\r\n\r\n").nth(1).map(|body| body.trim().to_string()).ok_or_else(|| "empty response".to_string())
}

/// 运行 adb，失败时仍返回标准输出，供检查远端文件摘要。
fn adb_stdout(args: &[&str]) -> String {
    Command::new("adb").args(args).output().map(|output| String::from_utf8_lossy(&output.stdout).to_string()).unwrap_or_default()
}

/// 计算内置 jar 的 MD5，用来判断设备上的文件是否需要更新。
fn md5_hex(data: &[u8]) -> String {
    let digest = md5(data);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 计算 MD5。
fn md5(data: &[u8]) -> [u8; 16] {
    let mut message = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_le_bytes());
    let mut state = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    for chunk in message.chunks(64) {
        let mut words = [0u32; 16];
        for (index, word) in words.iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_le_bytes([chunk[offset], chunk[offset + 1], chunk[offset + 2], chunk[offset + 3]]);
        }
        let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(MD5_K[i]).wrapping_add(words[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(MD5_S[i]));
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (index, value) in state.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    out
}

const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

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

#[cfg(test)]
mod tests {
    /// 用已知向量确认内置 jar 的摘要算法。
    #[test]
    fn md5_matches_known_vectors() {
        assert_eq!(super::md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(super::md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(super::md5_hex(super::U2_JAR), "d4f2cb5299dfd894a40fdde067b56bc9");
    }
}
