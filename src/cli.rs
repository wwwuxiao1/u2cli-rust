/// 一次解析后的调用。show_help 为真时只打印帮助。
#[derive(Debug, Clone, PartialEq)]
pub struct Invocation {
    pub global: GlobalOptions,
    pub command: Command,
    pub show_help: bool,
    pub help_topic: Option<String>,
}

/// 对所有子命令生效的全局选项。
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalOptions {
    pub debug: bool,
    pub serial: Option<String>,
    pub server_host: String,
    pub server_port: u16,
    pub device_port: u16,
}

/// 选择器里的一个条件值。
#[derive(Debug, Clone, PartialEq)]
pub enum SelectorValue {
    Text(String),
    Bool(bool),
    Int(i64),
}

/// 一层选择器条件，字段按命令行出现顺序保留。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selector {
    pub fields: Vec<(String, SelectorValue)>,
}

/// 已识别的子命令及其参数。
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    None,
    Server { foreground: bool },
    StartServer,
    KillServer,
    ServerStatus,
    Screenshot { filename: String },
    DumpHierarchy { compressed: bool, max_depth: Option<i64>, output: Option<String>, raw: bool },
    AppCurrent,
    DeviceInfo,
    WindowSize,
    Shell { timeout: u64, command: String },
    AppStart { package: String, activity: Option<String>, wait: bool, stop: bool },
    AppList { filter: String },
    AppStop { package: Option<String>, all: bool },
    AppInstall { apk: String },
    AppUninstall { package: String },
    AppClear { package: String },
    Press { key: String },
    SendKeys { text: String, clear: bool },
    ClearText,
    Click { x: Option<f64>, y: Option<f64>, timeout: f64, selector: Selector, children: Vec<Selector> },
    DoubleClick { x: f64, y: f64, duration: f64 },
    LongClick { x: Option<f64>, y: Option<f64>, duration: f64, timeout: f64, selector: Selector, children: Vec<Selector> },
    Swipe { direction: Option<String>, fx: Option<f64>, fy: Option<f64>, tx: Option<f64>, ty: Option<f64>, duration: f64, steps: Option<i64>, scale: f64 },
    Drag { sx: f64, sy: f64, ex: f64, ey: f64, duration: f64 },
    Exists { timeout: f64, selector: Selector, children: Vec<Selector> },
    Wait { timeout: f64, gone: bool, selector: Selector, children: Vec<Selector> },
    Scroll { direction: String, action: String, max_swipes: Option<i64>, to_text: Option<String>, selector: Selector, children: Vec<Selector> },
    OpenNotification,
    OpenQuickSettings,
    OpenUrl { url: String },
}

/// 解析 argv。用法错误返回说明文字，不访问设备。
pub fn parse(args: &[String]) -> Result<Invocation, String> {
    if args.is_empty() {
        return Ok(help_invocation(None));
    }
    let mut index = 0;
    let mut global = GlobalOptions {
        debug: false,
        serial: None,
        server_host: "127.0.0.1".to_string(),
        server_port: 17913,
        device_port: 9008,
    };
    while index < args.len() {
        let token = &args[index];
        if token == "-h" || token == "--help" {
            return Ok(help_invocation(None));
        }
        if !token.starts_with('-') {
            break;
        }
        index = take_global(args, index, &mut global)?;
    }
    if index >= args.len() {
        return Err("Missing command".to_string());
    }
    let name = args[index].clone();
    index += 1;
    if index < args.len() && (args[index] == "-h" || args[index] == "--help") {
        let mut invocation = help_invocation(Some(name));
        invocation.global = global;
        return Ok(invocation);
    }
    let command = parse_command(&name, args, &mut index, &global)?;
    if index != args.len() {
        return Err(format!("No such option '{}'", args[index]));
    }
    Ok(Invocation { global, command, show_help: false, help_topic: None })
}

/// 构造帮助调用。
fn help_invocation(topic: Option<String>) -> Invocation {
    Invocation {
        global: GlobalOptions {
            debug: false,
            serial: None,
            server_host: "127.0.0.1".to_string(),
            server_port: 17913,
            device_port: 9008,
        },
        command: Command::None,
        show_help: true,
        help_topic: topic,
    }
}

/// 读取一个全局选项并前进下标。
fn take_global(args: &[String], index: usize, global: &mut GlobalOptions) -> Result<usize, String> {
    let token = &args[index];
    match token.as_str() {
        "-d" | "--debug" => {
            global.debug = true;
            Ok(index + 1)
        }
        "-s" | "--serial" => {
            global.serial = Some(require_value(args, index, token)?);
            Ok(index + 2)
        }
        "--server-host" => {
            global.server_host = require_value(args, index, token)?;
            Ok(index + 2)
        }
        "--server-port" => {
            global.server_port = require_port(args, index, token)?;
            Ok(index + 2)
        }
        "--device-port" => {
            global.device_port = require_port(args, index, token)?;
            Ok(index + 2)
        }
        _ => Err(format!("No such option '{token}'")),
    }
}

/// 按子命令名称解析剩余参数。
fn parse_command(name: &str, args: &[String], index: &mut usize, _global: &GlobalOptions) -> Result<Command, String> {
    match name {
        "server" => Ok(Command::Server { foreground: take_flag(args, index, "--foreground") }),
        "start-server" => Ok(Command::StartServer),
        "kill-server" => Ok(Command::KillServer),
        "server-status" => Ok(Command::ServerStatus),
        "screenshot" => Ok(Command::Screenshot { filename: optional_arg(args, index).unwrap_or_else(|| "screenshot.jpg".to_string()) }),
        "dump-hierarchy" => parse_dump(args, index),
        "app-current" => Ok(Command::AppCurrent),
        "device-info" => Ok(Command::DeviceInfo),
        "window-size" => Ok(Command::WindowSize),
        "shell" => parse_shell(args, index),
        "app-start" => parse_app_start(args, index),
        "app-list" => parse_app_list(args, index),
        "app-stop" => parse_app_stop(args, index),
        "app-install" => Ok(Command::AppInstall { apk: require_arg(args, index, "APK")? }),
        "app-uninstall" => Ok(Command::AppUninstall { package: require_arg(args, index, "PACKAGE")? }),
        "app-clear" => Ok(Command::AppClear { package: require_arg(args, index, "PACKAGE")? }),
        "press" => Ok(Command::Press { key: require_arg(args, index, "KEY")? }),
        "send-keys" => parse_send_keys(args, index),
        "clear-text" => Ok(Command::ClearText),
        "click" => parse_pointer(args, index, true),
        "double-click" => parse_double_click(args, index),
        "long-click" => parse_pointer(args, index, false),
        "swipe" => parse_swipe(args, index),
        "drag" => parse_drag(args, index),
        "exists" => parse_exists(args, index),
        "wait" => parse_wait(args, index),
        "scroll" => parse_scroll(args, index),
        "open-notification" => Ok(Command::OpenNotification),
        "open-quick-settings" => Ok(Command::OpenQuickSettings),
        "open-url" => Ok(Command::OpenUrl { url: require_arg(args, index, "URL")? }),
        _ => Err(format!("No such command '{name}'")),
    }
}

/// 解析界面层级命令的选项。
fn parse_dump(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut compressed = false;
    let mut max_depth = None;
    let mut output = None;
    let mut raw = false;
    while *index < args.len() && args[*index].starts_with('-') {
        match args[*index].as_str() {
            "--compressed" => { compressed = true; *index += 1; }
            "--raw" => { raw = true; *index += 1; }
            "--max-depth" => { max_depth = Some(require_i64(args, *index, "--max-depth")?); *index += 2; }
            "-o" | "--output" => { output = Some(require_value(args, *index, &args[*index])?); *index += 2; }
            other => return Err(format!("No such option '{other}'")),
        }
    }
    Ok(Command::DumpHierarchy { compressed, max_depth, output, raw })
}

/// 解析 shell 命令和超时。
fn parse_shell(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut timeout = 60u64;
    let mut parts = Vec::new();
    while *index < args.len() {
        if args[*index] == "--timeout" {
            timeout = require_u64(args, *index, "--timeout")?;
            *index += 2;
        } else if args[*index].starts_with('-') {
            return Err(format!("No such option '{}'", args[*index]));
        } else {
            parts.push(args[*index].clone());
            *index += 1;
        }
    }
    if parts.is_empty() {
        return Err("Missing argument 'CMD'".to_string());
    }
    Ok(Command::Shell { timeout, command: parts.join(" ") })
}

/// 解析启动应用的包名和选项。
fn parse_app_start(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut activity = None;
    let mut wait = false;
    let mut stop = false;
    let mut package = None;
    while *index < args.len() {
        match args[*index].as_str() {
            "--activity" => { activity = Some(require_value(args, *index, "--activity")?); *index += 2; }
            "--wait" => { wait = true; *index += 1; }
            "--stop" => { stop = true; *index += 1; }
            other if other.starts_with('-') => return Err(format!("No such option '{other}'")),
            other => { package = Some(other.to_string()); *index += 1; }
        }
    }
    Ok(Command::AppStart { package: package.ok_or_else(|| "Missing argument 'PACKAGE'".to_string())?, activity, wait, stop })
}

/// 解析应用列表过滤条件。
fn parse_app_list(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut filter = String::new();
    while *index < args.len() {
        if args[*index] == "--filter" {
            filter = require_value(args, *index, "--filter")?;
            *index += 2;
        } else {
            return Err(format!("No such option '{}'", args[*index]));
        }
    }
    Ok(Command::AppList { filter })
}

/// 解析停止应用的包名或全部停止。
fn parse_app_stop(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut all = false;
    let mut package = None;
    while *index < args.len() {
        if args[*index] == "--all" {
            all = true;
            *index += 1;
        } else if args[*index].starts_with('-') {
            return Err(format!("No such option '{}'", args[*index]));
        } else {
            package = Some(args[*index].clone());
            *index += 1;
        }
    }
    if !all && package.is_none() {
        return Err("Missing argument 'PACKAGE'".to_string());
    }
    Ok(Command::AppStop { package, all })
}

/// 解析输入文字，默认先清空。
fn parse_send_keys(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut clear = true;
    let mut text = None;
    while *index < args.len() {
        if args[*index] == "--no-clear" {
            clear = false;
            *index += 1;
        } else if args[*index].starts_with('-') {
            return Err(format!("No such option '{}'", args[*index]));
        } else {
            text = Some(args[*index].clone());
            *index += 1;
        }
    }
    Ok(Command::SendKeys { text: text.ok_or_else(|| "Missing argument 'TEXT'".to_string())?, clear })
}

/// 解析点击或长按，坐标和选择器二选一。
fn parse_pointer(args: &[String], index: &mut usize, is_click: bool) -> Result<Command, String> {
    let mut timeout = 3.0;
    let mut duration = 0.5;
    let mut numbers = Vec::new();
    let mut selector = Selector::default();
    let mut child_fields = Selector::default();
    let mut children = Vec::new();
    while *index < args.len() {
        let token = args[*index].clone();
        if token == "--timeout" {
            timeout = require_f64(args, *index, "--timeout")?;
            *index += 2;
        } else if token == "--duration" && !is_click {
            duration = require_f64(args, *index, "--duration")?;
            *index += 2;
        } else if let Some(parsed) = take_selector_token(args, index, &mut selector, &mut child_fields, &mut children)? {
            if !parsed {
                if token.starts_with('-') {
                    return Err(format!("No such option '{token}'"));
                }
                numbers.push(parse_f64(&token)?);
                *index += 1;
            }
        }
    }
    push_child(&mut children, child_fields, &selector)?;
    let x = numbers.first().copied();
    let y = numbers.get(1).copied();
    if is_click {
        Ok(Command::Click { x, y, timeout, selector, children })
    } else {
        Ok(Command::LongClick { x, y, duration, timeout, selector, children })
    }
}

/// 解析双击坐标。
fn parse_double_click(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut duration = 0.1;
    let mut numbers = Vec::new();
    while *index < args.len() {
        if args[*index] == "--duration" {
            duration = require_f64(args, *index, "--duration")?;
            *index += 2;
        } else if args[*index].starts_with('-') {
            return Err(format!("No such option '{}'", args[*index]));
        } else {
            numbers.push(parse_f64(&args[*index])?);
            *index += 1;
        }
    }
    if numbers.len() != 2 {
        return Err("Missing argument 'X'".to_string());
    }
    Ok(Command::DoubleClick { x: numbers[0], y: numbers[1], duration })
}

/// 解析滑动的方向或四个坐标。
fn parse_swipe(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut duration = 0.5;
    let mut steps = None;
    let mut scale = 0.9;
    let mut positionals = Vec::new();
    while *index < args.len() {
        match args[*index].as_str() {
            "--duration" => { duration = require_f64(args, *index, "--duration")?; *index += 2; }
            "--steps" => { steps = Some(require_i64(args, *index, "--steps")?); *index += 2; }
            "--scale" => { scale = require_f64(args, *index, "--scale")?; *index += 2; }
            other if other.starts_with('-') => return Err(format!("No such option '{other}'")),
            other => { positionals.push(other.to_string()); *index += 1; }
        }
    }
    if positionals.is_empty() {
        return Err("Missing argument 'FX'".to_string());
    }
    if let Some(direction) = normalize_direction(&positionals[0]) {
        if positionals.len() != 1 {
            return Err("coordinate arguments are not allowed when swipe direction is used".to_string());
        }
        return Ok(Command::Swipe { direction: Some(direction), fx: None, fy: None, tx: None, ty: None, duration, steps, scale });
    }
    if positionals.len() != 4 {
        return Err("Missing argument 'FY'".to_string());
    }
    if (scale - 0.9).abs() > f64::EPSILON {
        return Err("--scale is only supported when swipe direction is used".to_string());
    }
    Ok(Command::Swipe {
        direction: None,
        fx: Some(parse_f64(&positionals[0])?),
        fy: Some(parse_f64(&positionals[1])?),
        tx: Some(parse_f64(&positionals[2])?),
        ty: Some(parse_f64(&positionals[3])?),
        duration,
        steps,
        scale,
    })
}

/// 解析拖拽的四个坐标。
fn parse_drag(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut duration = 0.5;
    let mut numbers = Vec::new();
    while *index < args.len() {
        if args[*index] == "--duration" {
            duration = require_f64(args, *index, "--duration")?;
            *index += 2;
        } else if args[*index].starts_with('-') {
            return Err(format!("No such option '{}'", args[*index]));
        } else {
            numbers.push(parse_f64(&args[*index])?);
            *index += 1;
        }
    }
    if numbers.len() != 4 {
        return Err("Missing argument 'SX'".to_string());
    }
    Ok(Command::Drag { sx: numbers[0], sy: numbers[1], ex: numbers[2], ey: numbers[3], duration })
}

/// 解析控件是否存在。
fn parse_exists(args: &[String], index: &mut usize) -> Result<Command, String> {
    let (timeout, selector, children) = parse_selector_command(args, index, 0.0)?;
    Ok(Command::Exists { timeout, selector, children })
}

/// 解析等待控件出现或消失。
fn parse_wait(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut gone = false;
    let mut timeout = 3.0;
    let mut selector = Selector::default();
    let mut child_fields = Selector::default();
    let mut children = Vec::new();
    while *index < args.len() {
        if args[*index] == "--gone" {
            gone = true;
            *index += 1;
        } else if args[*index] == "--timeout" {
            timeout = require_f64(args, *index, "--timeout")?;
            *index += 2;
        } else if !take_selector_token(args, index, &mut selector, &mut child_fields, &mut children)?.unwrap_or(false) {
            return Err(format!("No such option '{}'", args[*index]));
        }
    }
    push_child(&mut children, child_fields, &selector)?;
    Ok(Command::Wait { timeout, gone, selector, children })
}

/// 解析滚动选择器。
fn parse_scroll(args: &[String], index: &mut usize) -> Result<Command, String> {
    let mut direction = "vert".to_string();
    let mut action = "forward".to_string();
    let mut max_swipes = None;
    let mut to_text = None;
    let mut selector = Selector::default();
    let mut child_fields = Selector::default();
    let mut children = Vec::new();
    while *index < args.len() {
        match args[*index].as_str() {
            "--direction" => { direction = require_value(args, *index, "--direction")?; *index += 2; }
            "--action" => { action = require_value(args, *index, "--action")?; *index += 2; }
            "--max-swipes" => { max_swipes = Some(require_i64(args, *index, "--max-swipes")?); *index += 2; }
            "--to-text" => { to_text = Some(require_value(args, *index, "--to-text")?); *index += 2; }
            _ => {
                if !take_selector_token(args, index, &mut selector, &mut child_fields, &mut children)?.unwrap_or(false) {
                    return Err(format!("No such option '{}'", args[*index]));
                }
            }
        }
    }
    if !matches!(direction.as_str(), "vert" | "horiz") {
        return Err("invalid choice for --direction".to_string());
    }
    if !matches!(action.as_str(), "forward" | "backward" | "toEnd" | "toBeginning") {
        return Err("invalid choice for --action".to_string());
    }
    push_child(&mut children, child_fields, &selector)?;
    Ok(Command::Scroll { direction, action, max_swipes, to_text, selector, children })
}

/// 解析带超时的选择器命令。
fn parse_selector_command(args: &[String], index: &mut usize, default_timeout: f64) -> Result<(f64, Selector, Vec<Selector>), String> {
    let mut timeout = default_timeout;
    let mut selector = Selector::default();
    let mut child_fields = Selector::default();
    let mut children = Vec::new();
    while *index < args.len() {
        if args[*index] == "--timeout" {
            timeout = require_f64(args, *index, "--timeout")?;
            *index += 2;
        } else if !take_selector_token(args, index, &mut selector, &mut child_fields, &mut children)?.unwrap_or(false) {
            return Err(format!("No such option '{}'", args[*index]));
        }
    }
    push_child(&mut children, child_fields, &selector)?;
    Ok((timeout, selector, children))
}

/// 读取一个选择器选项。返回 Some(true) 表示已消费。
fn take_selector_token(args: &[String], index: &mut usize, selector: &mut Selector, child_fields: &mut Selector, children: &mut Vec<Selector>) -> Result<Option<bool>, String> {
    let token = &args[*index];
    if token == "--child" {
        let values = take_child_tokens(args, *index)?;
        let mut parsed = Selector::default();
        for value in &values {
            let (key, parsed_value) = parse_selector_pair(value)?;
            parsed.fields.push((key, parsed_value));
        }
        children.push(parsed);
        *index += 1 + values.len();
        return Ok(Some(true));
    }
    if let Some(key) = token.strip_prefix("--child-") {
        let value = require_value(args, *index, token)?;
        child_fields.fields.push((selector_key(key)?, parse_selector_text(key, &value)?));
        *index += 2;
        return Ok(Some(true));
    }
    if let Some(key) = token.strip_prefix("--") {
        if is_selector_flag(key) {
            selector.fields.push((selector_key(key)?, SelectorValue::Bool(true)));
            *index += 1;
            return Ok(Some(true));
        }
        if is_selector_value(key) {
            let value = require_value(args, *index, token)?;
            selector.fields.push((selector_key(key)?, parse_selector_text(key, &value)?));
            *index += 2;
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}

/// 把 --child-* 收成一层子选择器，并要求存在父选择器。
fn push_child(children: &mut Vec<Selector>, child_fields: Selector, parent: &Selector) -> Result<(), String> {
    let has_child = !child_fields.fields.is_empty() || !children.is_empty();
    if !child_fields.fields.is_empty() {
        children.insert(0, child_fields);
    }
    if has_child && parent.fields.is_empty() {
        return Err("at least one parent selector option is required when child selector is used".to_string());
    }
    Ok(())
}

/// 收集 --child 后面直到下一个选项的键值。
fn take_child_tokens(args: &[String], index: usize) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut cursor = index + 1;
    while cursor < args.len() && !args[cursor].starts_with('-') {
        values.push(args[cursor].clone());
        cursor += 1;
    }
    if values.is_empty() {
        return Err("Option '--child' requires an argument".to_string());
    }
    Ok(values)
}

/// 判断选项是不是布尔选择器。
fn is_selector_flag(key: &str) -> bool {
    matches!(key, "selected" | "focused" | "enabled" | "scrollable" | "clickable" | "checked" | "checkable")
}

/// 判断选项是不是带值的选择器。
fn is_selector_value(key: &str) -> bool {
    matches!(key, "text" | "text-contains" | "text-matches" | "text-starts-with" | "description" | "description-contains" | "resource-id" | "class-name" | "package" | "instance" | "index")
}

/// 把命令行上的选择器名字换成手机端使用的字段名。
fn selector_key(key: &str) -> Result<String, String> {
    let name = match key {
        "text" => "text",
        "text-contains" => "textContains",
        "text-matches" => "textMatches",
        "text-starts-with" => "textStartsWith",
        "description" => "description",
        "description-contains" => "descriptionContains",
        "resource-id" => "resourceId",
        "class-name" => "className",
        "package" => "packageName",
        "instance" => "instance",
        "index" => "index",
        "selected" => "selected",
        "focused" => "focused",
        "enabled" => "enabled",
        "scrollable" => "scrollable",
        "clickable" => "clickable",
        "checked" => "checked",
        "checkable" => "checkable",
        other => return Err(format!("unknown selector key: {other}")),
    };
    Ok(name.to_string())
}

/// 按字段类型解析选择器的值。
fn parse_selector_text(key: &str, value: &str) -> Result<SelectorValue, String> {
    if matches!(key, "index" | "instance") {
        let number = value.parse::<i64>().map_err(|_| format!("invalid integer selector value for {key}: {value}"))?;
        return Ok(SelectorValue::Int(number));
    }
    Ok(SelectorValue::Text(value.to_string()))
}

/// 解析 child 里的 key=value。
fn parse_selector_pair(token: &str) -> Result<(String, SelectorValue), String> {
    let (key, value) = token.split_once('=').unwrap_or((token, "true"));
    let canonical = key.replace('_', "-");
    let name = selector_key(&canonical)?;
    if matches!(name.as_str(), "index" | "instance") {
        return Ok((name, SelectorValue::Int(value.parse::<i64>().map_err(|_| format!("invalid integer selector value for {key}: {value}"))?)));
    }
    if matches!(name.as_str(), "selected" | "focused" | "enabled" | "scrollable" | "clickable" | "checked" | "checkable") {
        return Ok((name, SelectorValue::Bool(parse_bool(value)?)));
    }
    Ok((name, SelectorValue::Text(value.to_string())))
}

/// 解析选择器里的布尔文本。
fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("invalid boolean selector value: {value}")),
    }
}

/// 把滑动方向别名规范成 up/down/left/right。
fn normalize_direction(value: &str) -> Option<String> {
    let key = value.to_ascii_lowercase().replace('_', "-");
    let direction = match key.as_str() {
        "left" => "left",
        "right" => "right",
        "up" => "up",
        "down" => "down",
        "forward" => "up",
        "backward" => "down",
        "horiz-forward" => "left",
        "horiz-backward" => "right",
        _ => return None,
    };
    Some(direction.to_string())
}

/// 读取选项后面的必填文本。
fn require_value(args: &[String], index: usize, option: &str) -> Result<String, String> {
    args.get(index + 1).cloned().ok_or_else(|| format!("Option '{option}' requires an argument"))
}

/// 读取必填位置参数。
fn require_arg(args: &[String], index: &mut usize, name: &str) -> Result<String, String> {
    if *index >= args.len() || args[*index].starts_with('-') {
        return Err(format!("Missing argument '{name}'"));
    }
    let value = args[*index].clone();
    *index += 1;
    Ok(value)
}

/// 读取可选位置参数。
fn optional_arg(args: &[String], index: &mut usize) -> Option<String> {
    if *index >= args.len() || args[*index].starts_with('-') {
        return None;
    }
    let value = args[*index].clone();
    *index += 1;
    Some(value)
}

/// 若当前参数是指定开关，则消费它。
fn take_flag(args: &[String], index: &mut usize, name: &str) -> bool {
    if *index < args.len() && args[*index] == name {
        *index += 1;
        return true;
    }
    false
}

/// 读取端口号。
fn require_port(args: &[String], index: usize, option: &str) -> Result<u16, String> {
    let text = require_value(args, index, option)?;
    let port = text.parse::<u16>().map_err(|_| "port must be an integer".to_string())?;
    if port == 0 {
        return Err("port must be between 1 and 65535".to_string());
    }
    Ok(port)
}

/// 读取浮点选项。
fn require_f64(args: &[String], index: usize, option: &str) -> Result<f64, String> {
    parse_f64(&require_value(args, index, option)?)
}

/// 读取整数选项。
fn require_i64(args: &[String], index: usize, option: &str) -> Result<i64, String> {
    require_value(args, index, option)?.parse::<i64>().map_err(|_| format!("Option '{option}' requires an integer"))
}

/// 读取无符号整数选项。
fn require_u64(args: &[String], index: usize, option: &str) -> Result<u64, String> {
    require_value(args, index, option)?.parse::<u64>().map_err(|_| format!("Option '{option}' requires an integer"))
}

/// 把文本解析成浮点数。
fn parse_f64(value: &str) -> Result<f64, String> {
    value.parse::<f64>().map_err(|_| format!("invalid number: {value}"))
}
