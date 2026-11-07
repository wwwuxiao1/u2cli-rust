use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

/// 运行 u2cli 并返回进程输出。
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_u2cli")).args(args).output().expect("run u2cli")
}

/// 在带假 adb 和假手机端服务的环境里运行 u2cli。
fn run_with_phone(args: &[&str], phone: &Phone) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_u2cli"));
    command.args(args);
    command.env("PATH", format!("{}:{}", phone.adb_dir.display(), std::env::var("PATH").unwrap_or_default()));
    command.env("U2CLI_PHONE_PORT", phone.port.to_string());
    command.output().expect("run u2cli")
}

struct Phone {
    port: u16,
    adb_dir: std::path::PathBuf,
}

impl Phone {
    /// 启动假手机端服务，并放一个会转发到它的假 adb。
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || serve(listener));
        let adb_dir = std::env::temp_dir().join(format!("u2cli-adb-{port}"));
        std::fs::create_dir_all(&adb_dir).unwrap();
        let script = adb_dir.join("adb");
        std::fs::write(&script, fake_adb_script()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions).unwrap();
        }
        Phone { port, adb_dir }
    }
}

/// 假 adb：列出设备、转发端口、回答属性和 shell。
fn fake_adb_script() -> String {
    r#"#!/bin/sh
port="$U2CLI_PHONE_PORT"
serial=serial-1
if [ "$1" = "-s" ]; then
  serial="$2"
  shift 2
fi
case "$1" in
  devices)
    printf 'List of devices attached\n%s\tdevice\n' "$serial"
    ;;
  forward)
    printf '%s\n' "$port"
    ;;
  install|uninstall)
    printf 'Success\n'
    ;;
  shell)
    shift
    text="$*"
    case "$text" in
      getprop\ ro.serialno) printf '%s\n' "$serial" ;;
      getprop\ ro.build.version.sdk) printf '34\n' ;;
      getprop\ ro.product.brand) printf 'Fake\n' ;;
      getprop\ ro.product.model) printf 'Phone\n' ;;
      getprop\ ro.product.cpu.abi) printf 'arm64-v8a\n' ;;
      getprop\ ro.build.version.release) printf '14\n' ;;
      wm\ size) printf 'Physical size: 1080x2400\n' ;;
      dumpsys\ window) printf 'mCurrentFocus=Window{abc u0 com.example/.Main}\n' ;;
      pm\ list\ packages*) printf 'package:com.example.one\npackage:com.example.two\n' ;;
      cmd\ package\ resolve-activity*) printf 'com.example/.Main\n' ;;
      *) printf 'ok\n' ;;
    esac
    ;;
  *)
    printf 'ok\n'
    ;;
esac
"#.to_string()
}

/// 假手机端服务，按 JSON-RPC 方法返回固定结果。
fn serve(listener: TcpListener) {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut buffer = [0u8; 8192];
        let size = stream.read(&mut buffer).unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..size]);
        let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
        let result = if body.contains("takeScreenshot") {
            "\"/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////2wBDAf//////////////////////////////////////////////////////////////////////////////////////wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAb/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIQAxAAAAGf/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABBQJ//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAwEBPwF//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAgEBPwF//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQAGPwJ//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPyF//9k=\"".to_string()
        } else if body.contains("dumpWindowHierarchy") {
            r#""<hierarchy><node class=\"android.widget.TextView\" text=\"设置\" resource-id=\"pkg:id/title\" bounds=\"[10,20][200,80]\" clickable=\"true\" /><node class=\"android.widget.TextView\" text=\"Hidden\" bounds=\"[0,0][0,0]\" /></hierarchy>""#.to_string()
        } else if body.contains("objInfo") {
            r#"{"bounds":{"left":10,"top":20,"right":200,"bottom":80},"visibleBounds":{"left":10,"top":20,"right":200,"bottom":80}}"#.to_string()
        } else if body.contains("waitUntilGone") {
            "false".to_string()
        } else if body.contains("waitForExists") || body.contains("\"method\":\"exist\"") {
            "true".to_string()
        } else {
            "true".to_string()
        };
        let payload = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{result}}}");
        let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len());
        let _ = stream.write_all(response.as_bytes());
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn help_without_arguments_exits_cleanly() {
    for args in [Vec::<&str>::new(), vec!["--help"], vec!["-h"]] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(0));
        let text = stdout(&output);
        assert!(text.contains("Usage: u2cli"), "{text}");
        assert!(!text.contains("--json"));
        assert!(stderr(&output).is_empty());
    }
}

#[test]
fn help_lists_command_groups() {
    let text = stdout(&run(&["--help"]));
    for title in ["server commands:", "device commands:", "app commands:", "input commands:", "selector commands:", "system ui commands:"] {
        assert!(text.contains(title), "{text}");
    }
}

#[test]
fn usage_errors_are_distinct_and_do_not_touch_devices() {
    let cases = [
        (vec!["no-such-command"], "No such command"),
        (vec!["--no-such-option"], "No such option"),
        (vec!["--serial"], "requires an argument"),
        (vec!["--serial", "serial-1"], "Missing command"),
        (vec!["app-start"], "Missing argument"),
    ];
    for (args, message) in cases {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?} {}", stderr(&output));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains(message), "{}", stderr(&output));
        assert!(!stderr(&output).contains("Traceback"));
    }
}

#[test]
fn subcommand_help_shows_options() {
    let app = stdout(&run(&["app-start", "--help"]));
    assert_eq!(run(&["app-start", "--help"]).status.code(), Some(0));
    assert!(app.contains("--activity") && app.contains("--wait") && app.contains("--stop"));
    let exists = stdout(&run(&["exists", "--help"]));
    assert!(exists.contains("--text") && exists.contains("--child-text") && exists.contains("--child"));
}

#[test]
fn server_can_start_report_and_stop() {
    let port = 28111u16;
    let _ = run(&["--server-port", &port.to_string(), "kill-server"]);
    let started = run(&["-s", "serial-1", "--server-port", &port.to_string(), "start-server"]);
    assert_eq!(started.status.code(), Some(0), "{}", stderr(&started));
    let text = stdout(&started);
    assert!(text.contains("u2cli server started"));
    assert!(text.contains("host: 127.0.0.1"));
    assert!(text.contains(&format!("port: {port}")));
    let again = run(&["--server-port", &port.to_string(), "start-server"]);
    assert!(stdout(&again).contains("already running"));
    let status = run(&["--server-port", &port.to_string(), "server-status"]);
    let status_text = stdout(&status);
    assert!(status_text.lines().next().unwrap().starts_with("device_serial: "));
    assert!(status_text.contains("host: 127.0.0.1"));
    assert!(status_text.contains(&format!("port: {port}")));
    assert!(status_text.contains("pid: "));
    assert!(status_text.contains("running: True"));
    let stopped = run(&["--server-port", &port.to_string(), "kill-server"]);
    assert!(stdout(&stopped).contains("u2cli server stopped"));
    let missing = run(&["--server-port", &port.to_string(), "server-status"]);
    assert!(stdout(&missing).contains("running: False"));
    let idle = run(&["--server-port", &port.to_string(), "kill-server"]);
    assert!(stdout(&idle).contains("not running"));
}

#[test]
fn device_commands_print_visible_results() {
    let phone = Phone::start();
    let info = run_with_phone(&["-s", "serial-1", "device-info"], &phone);
    assert_eq!(info.status.code(), Some(0), "{}", stderr(&info));
    let text = stdout(&info);
    assert!(text.starts_with("device_serial: serial-1\n"));
    assert!(text.contains("u2_code: d.device_info"));
    assert!(text.contains("brand: Fake"));
    assert!(text.contains("model: Phone"));
    let size = run_with_phone(&["window-size"], &phone);
    assert!(stdout(&size).contains("width: 1080") && stdout(&size).contains("height: 2400"));
    let current = run_with_phone(&["app-current"], &phone);
    assert!(stdout(&current).contains("package: com.example"));
    assert!(stdout(&current).contains("activity: .Main"));
    let shell = run_with_phone(&["shell", "--timeout", "45", "echo", "hello"], &phone);
    assert!(stdout(&shell).contains("timeout=45"));
    assert!(stdout(&shell).contains("ok"));
    let shot = std::env::temp_dir().join(format!("u2cli-shot-{}.jpg", phone.port));
    let screenshot = run_with_phone(&["screenshot", shot.to_str().unwrap()], &phone);
    assert_eq!(screenshot.status.code(), Some(0), "{}", stderr(&screenshot));
    assert!(stdout(&screenshot).contains("resolution:"));
    assert!(shot.exists());
    let dumped = run_with_phone(&["dump-hierarchy"], &phone);
    assert!(stdout(&dumped).contains("TextView \"设置\""));
    assert!(!stdout(&dumped).contains("Hidden"));
    let raw = run_with_phone(&["dump-hierarchy", "--raw"], &phone);
    assert!(stdout(&raw).contains("<hierarchy>"));
    for command in ["open-notification", "open-quick-settings"] {
        let output = run_with_phone(&[command], &phone);
        assert_eq!(output.status.code(), Some(0), "{command} {}", stderr(&output));
    }
    let url = run_with_phone(&["open-url", "https://example.com"], &phone);
    assert!(stdout(&url).contains("d.open_url('https://example.com')"));
}

#[test]
fn device_failure_has_no_traceback() {
    let output = Command::new(env!("CARGO_BIN_EXE_u2cli"))
        .args(["device-info"])
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let err = stderr(&output);
    assert!(err.starts_with("error: "));
    assert!(!err.contains("Traceback"));
}

#[test]
fn app_commands_cover_lifecycle() {
    let phone = Phone::start();
    let started = run_with_phone(&["app-start", "com.example", "--activity", ".Main", "--wait", "--stop"], &phone);
    assert_eq!(started.status.code(), Some(0), "{}", stderr(&started));
    assert!(stdout(&started).contains("d.app_start('com.example', activity='.Main', wait=True, stop=True)"));
    let listed = run_with_phone(&["app-list", "--filter", "-3"], &phone);
    assert!(stdout(&listed).contains("com.example.one"));
    assert!(stdout(&listed).contains("com.example.two"));
    let stopped = run_with_phone(&["app-stop", "com.example"], &phone);
    assert!(stdout(&stopped).contains("d.app_stop('com.example')"));
    let all = run_with_phone(&["app-stop", "--all"], &phone);
    assert!(stdout(&all).contains("d.app_stop_all()"));
    let apk = run_with_phone(&["app-install", "demo.apk"], &phone);
    assert_eq!(apk.status.code(), Some(0), "{}", stderr(&apk));
    assert!(stdout(&apk).contains("d.app_install('demo.apk')"));
    assert_eq!(run_with_phone(&["app-uninstall", "com.example"], &phone).status.code(), Some(0));
    assert_eq!(run_with_phone(&["app-clear", "com.example"], &phone).status.code(), Some(0));
    assert_eq!(run(&["app-install"]).status.code(), Some(2));
    assert_eq!(run(&["app-stop"]).status.code(), Some(2));
}

#[test]
fn input_and_selectors_match_visible_commands() {
    let phone = Phone::start();
    let press = run_with_phone(&["press", "home"], &phone);
    assert!(stdout(&press).contains("d.press('home')"), "{}", stderr(&press));
    let typed = run_with_phone(&["send-keys", "hello"], &phone);
    assert!(stdout(&typed).contains("d.send_keys('hello')"));
    let kept = run_with_phone(&["send-keys", "--no-clear", "hello"], &phone);
    assert!(stdout(&kept).contains("clear=False"));
    assert_eq!(run_with_phone(&["clear-text"], &phone).status.code(), Some(0));
    let click = run_with_phone(&["click", "500", "1200"], &phone);
    assert!(stdout(&click).contains("d.click(500.0, 1200.0)"));
    let selected = run_with_phone(&["click", "--text", "Gmail"], &phone);
    assert!(stdout(&selected).contains("text='Gmail'"));
    let nested = run_with_phone(&["click", "--resource-id", "com.example:id/list", "--child", "text=OK"], &phone);
    assert!(stdout(&nested).contains(".child("), "{}", stdout(&nested));
    let missing_parent = run(&["click", "--child", "text=OK"]);
    assert_eq!(missing_parent.status.code(), Some(2), "{}", stderr(&missing_parent));
    assert_eq!(run_with_phone(&["double-click", "0.5", "0.5"], &phone).status.code(), Some(0));
    assert!(stdout(&run_with_phone(&["long-click", "10", "20"], &phone)).contains("d.long_click(10.0, 20.0"));
    assert!(stdout(&run_with_phone(&["swipe", "1", "2", "3", "4", "--duration", "0.3"], &phone)).contains("d.swipe(1.0, 2.0, 3.0, 4.0"));
    assert!(stdout(&run_with_phone(&["swipe", "up", "--scale", "0.5"], &phone)).contains("d.swipe_ext('up'"));
    assert_eq!(run_with_phone(&["drag", "1", "2", "3", "4"], &phone).status.code(), Some(0));
    let exists = run_with_phone(&["exists", "--text", "Gmail", "--timeout", "3"], &phone);
    assert!(stdout(&exists).contains("result: True"), "{}", stdout(&exists));
    let gone = run_with_phone(&["wait", "--text", "Loading", "--gone", "--timeout", "10"], &phone);
    assert!(stdout(&gone).contains("wait_gone"));
    let scroll = run_with_phone(&["scroll", "--scrollable", "--to-text", "Settings"], &phone);
    assert!(stdout(&scroll).contains("scroll.vert.to(text='Settings')"), "{}", stdout(&scroll));
}
