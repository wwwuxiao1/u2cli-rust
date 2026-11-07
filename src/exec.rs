use crate::cli::{Command, Invocation, Selector, SelectorValue};
use crate::device::Phone;
use crate::hierarchy;
use crate::json::{self, Value};
use crate::output::{self, py_bool, py_float, py_str};
use crate::server;

/// 执行已经解析好的命令，返回标准输出文本。
pub fn dispatch(invocation: &Invocation) -> Result<String, String> {
    let global = &invocation.global;
    match &invocation.command {
        Command::None => Ok(String::new()),
        Command::Server { .. } => {
            server::serve(&global.server_host, global.server_port, global.serial.clone())?;
            Ok(String::new())
        }
        Command::StartServer => {
            let started = server::start(&global.server_host, global.server_port, global.serial.clone())?;
            let message = if started { "u2cli server started" } else { "u2cli server is already running" };
            Ok(format!("{message}\nhost: {}\nport: {}\n", global.server_host, global.server_port))
        }
        Command::KillServer => {
            if server::stop(&global.server_host, global.server_port)? {
                Ok("u2cli server stopped\n".to_string())
            } else {
                Ok("u2cli server is not running\n".to_string())
            }
        }
        Command::ServerStatus => {
            let status = server::status(&global.server_host, global.server_port);
            let mut fields = vec![
                ("host", status.host),
                ("port", status.port.to_string()),
            ];
            if let Some(pid) = status.pid {
                fields.insert(0, ("pid", pid.to_string()));
            }
            fields.push(("running", py_bool(status.running).to_string()));
            let pairs: Vec<(&str, String)> = fields.iter().map(|(key, value)| (*key, value.clone())).collect();
            Ok(output::success(status.device_serial.as_deref(), None, &pairs, None))
        }
        Command::InstallService => {
            let phone = connect(invocation)?;
            Ok(output::success(Some(&phone.serial), Some("d.install_service"), &[("running", "True".to_string())], None))
        }
        Command::DeviceInfo => {
            let phone = connect(invocation)?;
            let info = [
                ("serial", phone.getprop("ro.serialno")?),
                ("sdk", phone.getprop("ro.build.version.sdk")?),
                ("brand", phone.getprop("ro.product.brand")?),
                ("model", phone.getprop("ro.product.model")?),
                ("arch", phone.getprop("ro.product.cpu.abi")?),
                ("version", phone.getprop("ro.build.version.release")?),
            ];
            let pairs: Vec<(&str, String)> = info.iter().map(|(key, value)| (*key, value.clone())).collect();
            Ok(output::success(Some(&phone.serial), Some("d.device_info"), &pairs, None))
        }
        Command::WindowSize => {
            let phone = connect(invocation)?;
            let (width, height) = window_size(&phone)?;
            Ok(output::success(Some(&phone.serial), Some("d.window_size()"), &[("width", width.to_string()), ("height", height.to_string())], None))
        }
        Command::AppCurrent => {
            let phone = connect(invocation)?;
            let (package, activity) = current_app(&phone)?;
            Ok(output::success(Some(&phone.serial), Some("d.app_current()"), &[("package", package), ("activity", activity)], None))
        }
        Command::Shell { timeout, command } => {
            let phone = connect(invocation)?;
            let (text, code) = phone.shell_within(command, *timeout)?;
            let body = if text.trim().is_empty() { None } else { Some(text.trim_end()) };
            Ok(output::success(Some(&phone.serial), Some(&format!("d.shell({}, timeout={timeout})", py_str(command))), &[("exit_code", code.to_string())], body))
        }
        Command::Screenshot { filename } => screenshot(invocation, filename),
        Command::DumpHierarchy { compressed, max_depth, output, raw } => dump_hierarchy(invocation, *compressed, *max_depth, output.clone(), *raw),
        Command::OpenNotification => simple_rpc(invocation, "openNotification", &Value::Array(vec![]), "d.open_notification()"),
        Command::OpenQuickSettings => simple_rpc(invocation, "openQuickSettings", &Value::Array(vec![]), "d.open_quick_settings()"),
        Command::OpenUrl { url } => {
            let phone = connect(invocation)?;
            phone.shell(&format!("am start -a android.intent.action.VIEW -d {}", shell_quote(url)))?;
            Ok(output::success(Some(&phone.serial), Some(&format!("d.open_url({})", py_str(url))), &[], None))
        }
        Command::AppStart { package, activity, wait, stop } => app_start(invocation, package, activity.as_deref(), *wait, *stop),
        Command::AppList { filter } => app_list(invocation, filter),
        Command::AppStop { package, all } => app_stop(invocation, package.as_deref(), *all),
        Command::AppInstall { apk } => {
            let phone = connect(invocation)?;
            adb_ok(&["-s", &phone.serial, "install", apk])?;
            Ok(output::success(Some(&phone.serial), Some(&format!("d.app_install({})", py_str(apk))), &[("result", py_str(apk).trim_matches('\'').to_string())], None))
        }
        Command::AppUninstall { package } => {
            let phone = connect(invocation)?;
            adb_ok(&["-s", &phone.serial, "uninstall", package])?;
            Ok(output::success(Some(&phone.serial), Some(&format!("d.app_uninstall({})", py_str(package))), &[("result", py_bool(true).to_string())], None))
        }
        Command::AppClear { package } => {
            let phone = connect(invocation)?;
            phone.shell(&format!("pm clear {package}"))?;
            Ok(output::success(Some(&phone.serial), Some(&format!("d.app_clear({})", py_str(package))), &[], None))
        }
        Command::Press { key } => press(invocation, key),
        Command::SendKeys { text, clear } => send_keys(invocation, text, *clear),
        Command::ClearText => simple_rpc(invocation, "clearInputText", &Value::Array(vec![]), "d.clear_text()"),
        Command::Click { x, y, timeout, selector, children } => pointer(invocation, true, *x, *y, 0.0, *timeout, selector, children),
        Command::LongClick { x, y, duration, timeout, selector, children } => pointer(invocation, false, *x, *y, *duration, *timeout, selector, children),
        Command::DoubleClick { x, y, duration } => {
            let phone = connect(invocation)?;
            phone.call("click", &Value::Array(vec![Value::Number(*x), Value::Number(*y)]))?;
            phone.call("click", &Value::Array(vec![Value::Number(*x), Value::Number(*y)]))?;
            Ok(output::success(Some(&phone.serial), Some(&format!("d.double_click({}, {}, duration={})", py_float(*x), py_float(*y), py_float(*duration))), &[], None))
        }
        Command::Swipe { direction, fx, fy, tx, ty, duration, steps, scale } => swipe(invocation, direction.as_deref(), *fx, *fy, *tx, *ty, *duration, *steps, *scale),
        Command::Drag { sx, sy, ex, ey, duration } => {
            let phone = connect(invocation)?;
            phone.call("drag", &Value::Array(vec![Value::Number(*sx), Value::Number(*sy), Value::Number(*ex), Value::Number(*ey), Value::Number(*duration)]))?;
            Ok(output::success(Some(&phone.serial), Some(&format!("d.drag({}, {}, {}, {}, duration={})", py_float(*sx), py_float(*sy), py_float(*ex), py_float(*ey), py_float(*duration))), &[], None))
        }
        Command::Exists { timeout, selector, children } => selector_bool(invocation, "exists", *timeout, false, selector, children),
        Command::Wait { timeout, gone, selector, children } => selector_bool(invocation, "wait", *timeout, *gone, selector, children),
        Command::Scroll { direction, action, max_swipes, to_text, selector, children } => scroll(invocation, direction, action, *max_swipes, to_text.as_deref(), selector, children),
    }
}

/// 连接命令指定的设备。
fn connect(invocation: &Invocation) -> Result<Phone, String> {
    Phone::connect(invocation.global.serial.as_deref(), invocation.global.device_port)
}

/// 调用无参数 JSON-RPC，并输出对应的 u2_code。
fn simple_rpc(invocation: &Invocation, method: &str, params: &Value, code: &str) -> Result<String, String> {
    let phone = connect(invocation)?;
    phone.call(method, params)?;
    Ok(output::success(Some(&phone.serial), Some(code), &[], None))
}

/// 保存截图并输出路径和分辨率。
fn screenshot(invocation: &Invocation, filename: &str) -> Result<String, String> {
    let phone = connect(invocation)?;
    let saved = absolute_path(filename);
    let result = phone.call("takeScreenshot", &Value::Array(vec![Value::Number(1.0), Value::Number(80.0)]))?;
    let Value::Text(encoded) = result else { return Err("screenshot payload is empty".to_string()); };
    let bytes = decode_base64(&encoded)?;
    std::fs::write(&saved, &bytes).map_err(|error| format!("cannot write screenshot: {error}"))?;
    let resolution = jpeg_size(&bytes).ok_or_else(|| "cannot read screenshot resolution".to_string())?;
    Ok(output::success(Some(&phone.serial), Some(&format!("d.screenshot({})", py_str(filename))), &[("saved_to", saved), ("resolution", resolution)], None))
}

/// 读取并输出界面层级。
fn dump_hierarchy(invocation: &Invocation, compressed: bool, max_depth: Option<i64>, output_path: Option<String>, raw: bool) -> Result<String, String> {
    let phone = connect(invocation)?;
    let mut params = vec![Value::Bool(compressed)];
    if let Some(depth) = max_depth {
        params.push(Value::Number(depth as f64));
    }
    let result = phone.call("dumpWindowHierarchy", &Value::Array(params))?;
    let Value::Text(xml) = result else { return Err("hierarchy payload is empty".to_string()); };
    let content = if raw { xml } else { hierarchy::hierarchy_to_text(&xml)? };
    let mut parts = Vec::new();
    if compressed { parts.push("compressed=True".to_string()); }
    if let Some(depth) = max_depth { parts.push(format!("max_depth={depth}")); }
    let code = format!("d.dump_hierarchy({})", parts.join(", "));
    if let Some(path) = output_path {
        let saved = absolute_path(&path);
        std::fs::write(&saved, &content).map_err(|error| error.to_string())?;
        return Ok(output::success(Some(&phone.serial), Some(&code), &[("saved_to", saved)], None));
    }
    Ok(output::success(Some(&phone.serial), Some(&code), &[], Some(&content)))
}

/// 启动应用。
fn app_start(invocation: &Invocation, package: &str, activity: Option<&str>, wait: bool, stop: bool) -> Result<String, String> {
    let phone = connect(invocation)?;
    if stop {
        phone.shell(&format!("am force-stop {package}"))?;
    }
    let activity = match activity {
        Some(value) => value.to_string(),
        None => resolve_activity(&phone, package)?,
    };
    if activity.is_empty() {
        phone.shell(&format!("monkey -p {package} -c android.intent.category.LAUNCHER 1"))?;
    } else {
        phone.shell(&format!("am start -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n {package}/{activity}"))?;
    }
    if wait {
        let _ = phone.shell(&format!("pidof {package}"));
    }
    let mut parts = vec![py_str(package)];
    if let Some(value) = activity_option(invocation) {
        parts.push(format!("activity={}", py_str(&value)));
    }
    if wait { parts.push("wait=True".to_string()); }
    if stop { parts.push("stop=True".to_string()); }
    Ok(output::success(Some(&phone.serial), Some(&format!("d.app_start({})", parts.join(", "))), &[], None))
}

/// 只有用户显式传入 activity 时才写进 u2_code。
fn activity_option(invocation: &Invocation) -> Option<String> {
    match &invocation.command {
        Command::AppStart { activity, .. } => activity.clone(),
        _ => None,
    }
}

/// 列出已安装包名。
fn app_list(invocation: &Invocation, filter: &str) -> Result<String, String> {
    let phone = connect(invocation)?;
    let command = if filter.is_empty() { "pm list packages".to_string() } else { format!("pm list packages {filter}") };
    let (output, _) = phone.shell(&command)?;
    let packages = output.lines().filter_map(|line| line.trim().strip_prefix("package:")).collect::<Vec<_>>().join("\n");
    let code = if filter.is_empty() { "d.app_list()".to_string() } else { format!("d.app_list({})", py_str(filter)) };
    Ok(output::success(Some(&phone.serial), Some(&code), &[], Some(&packages)))
}

/// 停止一个或全部第三方应用。
fn app_stop(invocation: &Invocation, package: Option<&str>, all: bool) -> Result<String, String> {
    let phone = connect(invocation)?;
    if all {
        let (output, _) = phone.shell("pm list packages -3")?;
        for line in output.lines() {
            if let Some(name) = line.trim().strip_prefix("package:") {
                if name != "com.github.uiautomator" && name != "com.github.uiautomator.test" {
                    let _ = phone.shell(&format!("am force-stop {name}"));
                }
            }
        }
        return Ok(output::success(Some(&phone.serial), Some("d.app_stop_all()"), &[], None));
    }
    let package = package.ok_or_else(|| "package is required unless --all is used".to_string())?;
    phone.shell(&format!("am force-stop {package}"))?;
    Ok(output::success(Some(&phone.serial), Some(&format!("d.app_stop({})", py_str(package))), &[], None))
}

/// 发送按键。数字按键走 keycode，其余走按键名。
fn press(invocation: &Invocation, key: &str) -> Result<String, String> {
    let phone = connect(invocation)?;
    if let Ok(code) = key.parse::<i64>() {
        phone.call("pressKeyCode", &Value::Array(vec![Value::Number(code as f64)]))?;
    } else {
        phone.call("pressKey", &Value::Array(vec![Value::Text(key.to_string())]))?;
    }
    Ok(output::success(Some(&phone.serial), Some(&format!("d.press({})", py_str(key))), &[], None))
}

/// 向焦点输入框写入文字。
fn send_keys(invocation: &Invocation, text: &str, clear: bool) -> Result<String, String> {
    let phone = connect(invocation)?;
    if clear {
        phone.call("clearInputText", &Value::Array(vec![]))?;
    }
    phone.call("setClipboard", &Value::Array(vec![Value::Text(text.to_string())]))?;
    phone.call("pasteClipboard", &Value::Array(vec![]))?;
    let code = if clear {
        format!("d.send_keys({})", py_str(text))
    } else {
        format!("d.send_keys({}, clear=False)", py_str(text))
    };
    Ok(output::success(Some(&phone.serial), Some(&code), &[], None))
}

/// 执行点击或长按。
fn pointer(invocation: &Invocation, is_click: bool, x: Option<f64>, y: Option<f64>, duration: f64, timeout: f64, selector: &Selector, children: &[Selector]) -> Result<String, String> {
    let phone = connect(invocation)?;
    if !selector.fields.is_empty() {
        let encoded = encode_selector(selector, children);
        let found = phone.call("waitForExists", &Value::Array(vec![encoded.clone(), Value::Number((timeout * 1000.0) as f64)]))?;
        if found != Value::Bool(true) {
            return Err("element not found".to_string());
        }
        let (cx, cy) = selector_center(&phone, &encoded)?;
        if is_click {
            phone.call("click", &Value::Array(vec![Value::Number(cx), Value::Number(cy)]))?;
            let code = format!("{}.click(timeout={})", selector_code(selector, children), py_float(timeout));
            return Ok(output::success(Some(&phone.serial), Some(&code), &[], None));
        }
        phone.call("click", &Value::Array(vec![Value::Number(cx), Value::Number(cy), Value::Number((duration * 1000.0) as f64)]))?;
        let code = format!("{}.long_click(duration={}, timeout={})", selector_code(selector, children), py_float(duration), py_float(timeout));
        return Ok(output::success(Some(&phone.serial), Some(&code), &[], None));
    }
    let (x, y) = match (x, y) {
        (Some(x), Some(y)) => (x, y),
        _ => return Err("x and y are required when no selector option is used".to_string()),
    };
    if is_click {
        phone.call("click", &Value::Array(vec![Value::Number(x), Value::Number(y)]))?;
        Ok(output::success(Some(&phone.serial), Some(&format!("d.click({}, {})", py_float(x), py_float(y))), &[], None))
    } else {
        phone.call("click", &Value::Array(vec![Value::Number(x), Value::Number(y), Value::Number((duration * 1000.0) as f64)]))?;
        Ok(output::success(Some(&phone.serial), Some(&format!("d.long_click({}, {}, duration={})", py_float(x), py_float(y), py_float(duration))), &[], None))
    }
}

/// 执行坐标或方向滑动。
fn swipe(invocation: &Invocation, direction: Option<&str>, fx: Option<f64>, fy: Option<f64>, tx: Option<f64>, ty: Option<f64>, duration: f64, steps: Option<i64>, scale: f64) -> Result<String, String> {
    let phone = connect(invocation)?;
    if let Some(direction) = direction {
        let (width, height) = window_size(&phone)?;
        let (x1, y1, x2, y2) = direction_points(direction, width, height, scale);
        phone.call("swipe", &Value::Array(vec![Value::Number(x1), Value::Number(y1), Value::Number(x2), Value::Number(y2), Value::Number(duration)]))?;
        let code = if let Some(steps) = steps {
            format!("d.swipe_ext({}, scale={}, steps={steps})", py_str(direction), py_float(scale))
        } else {
            format!("d.swipe_ext({}, scale={}, duration={})", py_str(direction), py_float(scale), py_float(duration))
        };
        return Ok(output::success(Some(&phone.serial), Some(&code), &[], None));
    }
    let (fx, fy, tx, ty) = match (fx, fy, tx, ty) {
        (Some(fx), Some(fy), Some(tx), Some(ty)) => (fx, fy, tx, ty),
        _ => return Err("fx, fy, tx and ty are required when swipe direction is not used".to_string()),
    };
    phone.call("swipe", &Value::Array(vec![Value::Number(fx), Value::Number(fy), Value::Number(tx), Value::Number(ty), Value::Number(duration)]))?;
    let code = if let Some(steps) = steps {
        format!("d.swipe({}, {}, {}, {}, steps={steps})", py_float(fx), py_float(fy), py_float(tx), py_float(ty))
    } else {
        format!("d.swipe({}, {}, {}, {}, duration={})", py_float(fx), py_float(fy), py_float(tx), py_float(ty), py_float(duration))
    };
    Ok(output::success(Some(&phone.serial), Some(&code), &[], None))
}

/// 检查或等待选择器，并把布尔结果打出来。
fn selector_bool(invocation: &Invocation, kind: &str, timeout: f64, gone: bool, selector: &Selector, children: &[Selector]) -> Result<String, String> {
    if selector.fields.is_empty() {
        return Err("at least one selector option is required".to_string());
    }
    let phone = connect(invocation)?;
    let encoded = encode_selector(selector, children);
    let result = if kind == "exists" && timeout <= 0.0 {
        phone.call("exist", &Value::Array(vec![encoded]))?
    } else if gone {
        phone.call("waitUntilGone", &Value::Array(vec![encoded, Value::Number((timeout * 1000.0) as f64)]))?
    } else {
        phone.call("waitForExists", &Value::Array(vec![encoded, Value::Number((timeout * 1000.0) as f64)]))?
    };
    let flag = match result {
        Value::Bool(value) => value,
        _ => false,
    };
    let code = match (kind, gone) {
        ("exists", _) => format!("{}.exists(timeout={})", selector_code(selector, children), py_float(timeout)),
        (_, true) => format!("{}.wait_gone(timeout={})", selector_code(selector, children), py_float(timeout)),
        _ => format!("{}.wait(timeout={})", selector_code(selector, children), py_float(timeout)),
    };
    Ok(output::success(Some(&phone.serial), Some(&code), &[("result", py_bool(flag).to_string())], None))
}

/// 滚动选择器。
fn scroll(invocation: &Invocation, direction: &str, action: &str, max_swipes: Option<i64>, to_text: Option<&str>, selector: &Selector, children: &[Selector]) -> Result<String, String> {
    if selector.fields.is_empty() {
        return Err("at least one selector option is required".to_string());
    }
    let phone = connect(invocation)?;
    let encoded = encode_selector(selector, children);
    let vertical = direction != "horiz";
    let max_swipes = max_swipes.unwrap_or(500) as f64;
    let steps = 55.0;
    if let Some(text) = to_text {
        let target = encode_selector(&Selector { fields: vec![("text".to_string(), SelectorValue::Text(text.to_string()))] }, &[]);
        phone.call("scrollTo", &Value::Array(vec![encoded, target, Value::Bool(vertical)]))?;
    } else {
        match action {
            "forward" => { phone.call("scrollForward", &Value::Array(vec![encoded, Value::Bool(vertical), Value::Number(steps)]))?; }
            "backward" => { phone.call("scrollBackward", &Value::Array(vec![encoded, Value::Bool(vertical), Value::Number(steps)]))?; }
            "toBeginning" => { phone.call("scrollToBeginning", &Value::Array(vec![encoded, Value::Bool(vertical), Value::Number(max_swipes), Value::Number(steps)]))?; }
            "toEnd" => { phone.call("scrollToEnd", &Value::Array(vec![encoded, Value::Bool(vertical), Value::Number(max_swipes), Value::Number(steps)]))?; }
            other => return Err(format!("unsupported scroll action: {other}")),
        }
    }
    let code = if let Some(text) = to_text {
        format!("{}.scroll.{direction}.to(text={})", selector_code(selector, children), py_str(text))
    } else if let Some(swipes) = max_swipes_opt(invocation) {
        format!("{}.scroll.{direction}.{action}(max_swipes={swipes})", selector_code(selector, children))
    } else {
        format!("{}.scroll.{direction}.{action}()", selector_code(selector, children))
    };
    Ok(output::success(Some(&phone.serial), Some(&code), &[], None))
}

/// 只有用户显式传入 max-swipes 时才写进 u2_code。
fn max_swipes_opt(invocation: &Invocation) -> Option<i64> {
    match &invocation.command {
        Command::Scroll { max_swipes, .. } => *max_swipes,
        _ => None,
    }
}

/// 编码成手机端 UiSelector 需要的对象，包含 mask 和子选择器。
fn encode_selector(selector: &Selector, children: &[Selector]) -> Value {
    let mut fields = layer_fields(selector);
    let mut kinds = Vec::new();
    let mut nested = Vec::new();
    for child in children {
        kinds.push(Value::Text("child".to_string()));
        nested.push(Value::Object(layer_fields(child)));
    }
    fields.push(("childOrSibling".to_string(), Value::Array(kinds)));
    fields.push(("childOrSiblingSelector".to_string(), Value::Array(nested)));
    Value::Object(fields)
}

/// 编码一层选择器字段，并写入对应的 mask。
fn layer_fields(selector: &Selector) -> Vec<(String, Value)> {
    let mut mask = 0i64;
    let mut fields = Vec::new();
    for (key, value) in &selector.fields {
        mask |= selector_mask(key);
        let encoded = match value {
            SelectorValue::Text(text) => Value::Text(text.clone()),
            SelectorValue::Bool(flag) => Value::Bool(*flag),
            SelectorValue::Int(number) => Value::Number(*number as f64),
        };
        fields.push((key.clone(), encoded));
    }
    fields.insert(0, ("mask".to_string(), Value::Number(mask as f64)));
    fields
}

/// 选择器字段对应的 mask 位。
fn selector_mask(key: &str) -> i64 {
    match key {
        "text" => 0x01,
        "textContains" => 0x02,
        "textMatches" => 0x04,
        "textStartsWith" => 0x08,
        "className" => 0x10,
        "description" => 0x40,
        "descriptionContains" => 0x80,
        "checkable" => 0x0400,
        "checked" => 0x0800,
        "clickable" => 0x1000,
        "scrollable" => 0x4000,
        "enabled" => 0x8000,
        "focused" => 0x020000,
        "selected" => 0x040000,
        "packageName" => 0x080000,
        "resourceId" => 0x200000,
        "index" => 0x800000,
        "instance" => 0x01000000,
        _ => 0,
    }
}

/// 读取选择器中心点。
fn selector_center(phone: &Phone, selector: &Value) -> Result<(f64, f64), String> {
    let info = phone.call("objInfo", &Value::Array(vec![selector.clone()]))?;
    let bounds = json::object_get(&info, "visibleBounds").or_else(|| json::object_get(&info, "bounds")).ok_or_else(|| "selector has no bounds".to_string())?;
    let left = number_field(bounds, "left")?;
    let top = number_field(bounds, "top")?;
    let right = number_field(bounds, "right")?;
    let bottom = number_field(bounds, "bottom")?;
    Ok(((left + right) / 2.0, (top + bottom) / 2.0))
}

/// 从对象中读取数字字段。
fn number_field(value: &Value, key: &str) -> Result<f64, String> {
    match json::object_get(value, key) {
        Some(Value::Number(number)) => Ok(*number),
        _ => Err(format!("missing number field {key}")),
    }
}

/// 生成选择器的 u2_code 前缀。
fn selector_code(selector: &Selector, children: &[Selector]) -> String {
    let mut code = format!("d({})", selector_repr(selector));
    for child in children {
        code.push_str(&format!(".child({})", selector_repr(child)));
    }
    code
}

/// 生成一层选择器的参数列表。
fn selector_repr(selector: &Selector) -> String {
    selector.fields.iter().map(|(key, value)| match value {
        SelectorValue::Text(text) => format!("{key}={}", py_str(text)),
        SelectorValue::Bool(flag) => format!("{key}={}", py_bool(*flag)),
        SelectorValue::Int(number) => format!("{key}={number}"),
    }).collect::<Vec<_>>().join(", ")
}

/// 从 wm size 读取窗口宽高。
fn window_size(phone: &Phone) -> Result<(i64, i64), String> {
    let (output, _) = phone.shell("wm size")?;
    for line in output.lines() {
        if let Some(size) = line.split(':').nth(1) {
            if let Some((width, height)) = size.trim().split_once('x') {
                if let (Ok(width), Ok(height)) = (width.parse::<i64>(), height.parse::<i64>()) {
                    return Ok((width, height));
                }
            }
        }
    }
    Err("cannot read window size".to_string())
}

/// 从当前焦点窗口读取包名和 activity。
fn current_app(phone: &Phone) -> Result<(String, String), String> {
    let (output, _) = phone.shell("dumpsys window")?;
    for line in output.lines() {
        if let Some(component) = line.split_whitespace().find(|part| part.contains('/')) {
            let component = component.trim_matches(|ch| ch == '{' || ch == '}');
            if let Some((package, activity)) = component.split_once('/') {
                return Ok((package.to_string(), activity.to_string()));
            }
        }
    }
    Err("Couldn't get focused app".to_string())
}

/// 解析应用的启动 activity。
fn resolve_activity(phone: &Phone, package: &str) -> Result<String, String> {
    let (output, _) = phone.shell(&format!("cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER {package}"))?;
    if let Some(line) = output.lines().last() {
        if let Some((found, activity)) = line.trim().split_once('/') {
            if found == package {
                return Ok(activity.to_string());
            }
        }
    }
    Ok(String::new())
}

/// 根据方向和比例计算滑动起止点。
fn direction_points(direction: &str, width: i64, height: i64, scale: f64) -> (f64, f64, f64, f64) {
    let width = width as f64;
    let height = height as f64;
    let dx = width * scale / 2.0;
    let dy = height * scale / 2.0;
    match direction {
        "left" => (width / 2.0 + dx, height / 2.0, width / 2.0 - dx, height / 2.0),
        "right" => (width / 2.0 - dx, height / 2.0, width / 2.0 + dx, height / 2.0),
        "down" => (width / 2.0, height / 2.0 - dy, width / 2.0, height / 2.0 + dy),
        _ => (width / 2.0, height / 2.0 + dy, width / 2.0, height / 2.0 - dy),
    }
}

/// 把相对路径变成绝对路径。
fn absolute_path(path: &str) -> String {
    let path = std::path::Path::new(path);
    if path.is_absolute() {
        path.display().to_string()
    } else {
        std::env::current_dir().map(|dir| dir.join(path).display().to_string()).unwrap_or_else(|_| path.display().to_string())
    }
}

/// 运行 adb 并在失败时返回错误。
fn adb_ok(args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("adb").args(args).output().map_err(|error| format!("adb is not available: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// 给 shell 参数加单引号。
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// 解码截图使用的 base64。
fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut values = Vec::new();
    for byte in input.bytes() {
        if byte == b'=' || byte.is_ascii_whitespace() { continue; }
        let index = alphabet.iter().position(|item| *item == byte).ok_or_else(|| "invalid screenshot encoding".to_string())?;
        values.push(index as u8);
    }
    let mut bytes = Vec::new();
    for chunk in values.chunks(4) {
        let a = chunk.first().copied().unwrap_or(0);
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        let d = chunk.get(3).copied().unwrap_or(0);
        bytes.push((a << 2) | (b >> 4));
        if chunk.len() > 2 { bytes.push((b << 4) | (c >> 2)); }
        if chunk.len() > 3 { bytes.push((c << 6) | d); }
    }
    Ok(bytes)
}

/// 从 JPEG 的 SOF 段读取宽高。
fn jpeg_size(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut index = 2;
    while index + 8 < bytes.len() {
        if bytes[index] != 0xff { return None; }
        let marker = bytes[index + 1];
        let size = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
        if marker == 0xc0 || marker == 0xc2 {
            let height = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]);
            let width = u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]);
            return Some(format!("{width}x{height}"));
        }
        index += 2 + size;
    }
    None
}
