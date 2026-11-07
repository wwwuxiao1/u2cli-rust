use std::process::{Command, Output};

/// 未设置序列号时不访问设备。设置 U2CLI_DEVICE_SERIAL 后按顺序验证基本命令。
#[test]
fn real_device_basic_commands() {
    let Ok(serial) = std::env::var("U2CLI_DEVICE_SERIAL") else {
        return;
    };
    if serial.is_empty() {
        return;
    }
    assert_output(&serial, &["install-service"], &["u2_code: d.install_service", "running: True"]);
    assert_output(&serial, &["device-info"], &[&format!("serial: {serial}"), "brand:"]);
    let size = assert_output(&serial, &["window-size"], &["width:", "height:"]);
    let (width, height) = window_size(&stdout(&size));
    let shot = std::env::temp_dir().join(format!("u2cli-device-{serial}.jpg"));
    let _ = std::fs::remove_file(&shot);
    assert_output(&serial, &["screenshot", shot.to_str().unwrap()], &["resolution:"]);
    assert!(std::fs::metadata(&shot).map(|meta| meta.len() > 0).unwrap_or(false));
    assert_output(&serial, &["dump-hierarchy"], &["["]);
    assert_output(&serial, &["app-current"], &["package:"]);
    assert_output(&serial, &["press", "home"], &["d.press('home')"]);
    let x = (width / 2).to_string();
    let y = (height / 2).to_string();
    let from_y = (height * 3 / 4).to_string();
    let to_y = (height / 4).to_string();
    assert_output(&serial, &["click", &x, &y], &["d.click("]);
    assert_output(&serial, &["swipe", &x, &from_y, &x, &to_y, "--duration", "0.2"], &["d.swipe("]);
}

/// 运行一条设备命令并检查可见输出。
fn assert_output(serial: &str, args: &[&str], needles: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_u2cli"));
    command.arg("-s").arg(serial).args(args);
    let output = command.output().expect("run u2cli");
    assert_eq!(output.status.code(), Some(0), "{args:?} {}", stderr(&output));
    let text = stdout(&output);
    for needle in needles {
        assert!(text.contains(needle), "{args:?} missing {needle} in {text}");
    }
    output
}

/// 从 window-size 的文本里取出宽和高。
fn window_size(text: &str) -> (i64, i64) {
    let mut width = None;
    let mut height = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("width: ") {
            width = value.trim().parse().ok();
        }
        if let Some(value) = line.strip_prefix("height: ") {
            height = value.trim().parse().ok();
        }
    }
    (width.expect("width"), height.expect("height"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
