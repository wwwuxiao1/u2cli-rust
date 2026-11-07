/// 命令帮助里的分组。只列出已经注册的子命令。
pub struct Group {
    pub title: &'static str,
    pub commands: &'static [&'static str],
}

/// 全部命令分组，顺序与现有帮助一致。
pub fn groups() -> &'static [Group] {
    &[
        Group { title: "server commands:", commands: &["server", "start-server", "kill-server", "server-status"] },
        Group { title: "device commands:", commands: &["install-service", "device-info", "window-size", "screenshot", "dump-hierarchy", "app-current", "shell"] },
        Group { title: "app commands:", commands: &["app-start", "app-list", "app-stop", "app-install", "app-uninstall", "app-clear"] },
        Group { title: "input commands:", commands: &["press", "send-keys", "clear-text", "click", "double-click", "long-click", "swipe", "drag"] },
        Group { title: "selector commands:", commands: &["exists", "wait", "scroll"] },
        Group { title: "system ui commands:", commands: &["open-notification", "open-quick-settings", "open-url"] },
    ]
}

/// 生成根命令或子命令的帮助。命令名和选项名保持英文，说明使用中文。
pub fn render(invocation: &crate::cli::Invocation) -> String {
    if let Some(name) = &invocation.help_topic {
        return subcommand_help(name);
    }
    let mut text = String::from("Usage: u2cli [OPTIONS] COMMAND [ARGS]...\n\nOptions:\n");
    text.push_str(&option("-d, --debug", "打开调试输出。", "u2cli -d device-info"));
    text.push_str(&option("-s, --serial SERIAL", "指定要操作的设备序列号。", "u2cli -s SERIAL device-info"));
    text.push_str(&option("--server-host HOST", "本机服务地址，默认 127.0.0.1。", "u2cli --server-host 127.0.0.1 server-status"));
    text.push_str(&option("--server-port PORT", "本机服务端口，默认 17913。", "u2cli --server-port 17913 server-status"));
    text.push_str(&option("--device-port PORT", "手机端服务端口，默认 9008。", "u2cli --device-port 9008 install-service"));
    text.push_str(&option("-h, --help", "显示这份帮助。", "u2cli --help"));
    for group in groups() {
        text.push('\n');
        text.push_str(group.title);
        text.push('\n');
        for name in group.commands {
            text.push_str("  ");
            text.push_str(name);
            text.push('\n');
            text.push_str("      ");
            text.push_str(summary(name));
            text.push('\n');
        }
    }
    text
}

/// 生成单个子命令的帮助。
fn subcommand_help(name: &str) -> String {
    let mut text = format!("Usage: u2cli {name} [OPTIONS] [ARGS]...\n\n{}\n\n", summary(name));
    text.push_str(&detail(name));
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// 格式化一个选项：英文名字、一句中文、一个例子。
fn option(names: &str, sentence: &str, example: &str) -> String {
    format!("  {names}\n      {sentence}\n      例子: {example}\n")
}

/// 没有额外参数的命令说明。
fn no_args(example: &str) -> String {
    format!("不需要额外参数。\n例子: {example}\n")
}

/// 选择器参数。例子里的命令名换成当前子命令。
fn selector_options(command: &str) -> String {
    [
        option("--text TEXT", "按完整文本定位控件。", &format!("u2cli {command} --text Gmail")),
        option("--text-contains TEXT", "按文本片段定位控件。", &format!("u2cli {command} --text-contains 设置")),
        option("--text-matches REGEX", "按文本正则定位控件。", &format!("u2cli {command} --text-matches ^Gmail")),
        option("--text-starts-with TEXT", "按文本开头定位控件。", &format!("u2cli {command} --text-starts-with G")),
        option("--description TEXT", "按完整描述定位控件。", &format!("u2cli {command} --description 返回")),
        option("--description-contains TEXT", "按描述片段定位控件。", &format!("u2cli {command} --description-contains 返回")),
        option("--resource-id ID", "按资源 id 定位控件。", &format!("u2cli {command} --resource-id com.example:id/title")),
        option("--class-name CLASS", "按控件类名定位。", &format!("u2cli {command} --class-name android.widget.TextView")),
        option("--package PACKAGE", "按应用包名定位控件。", &format!("u2cli {command} --package com.example")),
        option("--selected", "只匹配已选中的控件。", &format!("u2cli {command} --selected")),
        option("--focused", "只匹配有焦点的控件。", &format!("u2cli {command} --focused")),
        option("--enabled", "只匹配可用的控件。", &format!("u2cli {command} --enabled")),
        option("--scrollable", "只匹配可滚动的控件。", &format!("u2cli {command} --scrollable")),
        option("--clickable", "只匹配可点击的控件。", &format!("u2cli {command} --clickable")),
        option("--checked", "只匹配已勾选的控件。", &format!("u2cli {command} --checked")),
        option("--checkable", "只匹配可勾选的控件。", &format!("u2cli {command} --checkable")),
        option("--instance N", "在匹配结果里取第几个，从 0 开始。", &format!("u2cli {command} --text OK --instance 0")),
        option("--index N", "按控件在父节点中的序号定位。", &format!("u2cli {command} --index 0")),
        option("--child KEY=VALUE", "在父控件下再加一层子条件。", &format!("u2cli {command} --resource-id com.example:id/list --child text=OK")),
        option("--child-text TEXT", "子控件的完整文本。", &format!("u2cli {command} --resource-id com.example:id/list --child-text OK")),
        option("--timeout SECONDS", "等待控件出现的秒数。", &format!("u2cli {command} --text Gmail --timeout 3")),
    ].concat()
}

/// 根帮助里每条命令的一句话。
fn summary(name: &str) -> &'static str {
    match name {
        "server" => "在当前终端前台运行本机服务。",
        "start-server" => "在后台启动本机服务。已经在运行时会说明。",
        "kill-server" => "停止本机服务。",
        "server-status" => "查看本机服务是否在运行。",
        "install-service" => "安装并拉起手机端服务。",
        "device-info" => "查看序列号、系统和机型。",
        "window-size" => "查看屏幕宽高。",
        "screenshot" => "把当前画面保存到文件。",
        "dump-hierarchy" => "输出当前界面层级。",
        "app-current" => "查看前台应用的包名和 Activity。",
        "shell" => "在设备上执行一条命令。",
        "app-start" => "按包名启动应用。",
        "app-list" => "列出已安装的包。",
        "app-stop" => "停止一个应用，或停止第三方应用。",
        "app-install" => "把 APK 安装到设备。",
        "app-uninstall" => "按包名卸载应用。",
        "app-clear" => "清空应用数据。",
        "press" => "发送一个按键。",
        "send-keys" => "向焦点输入框打字。",
        "clear-text" => "清空焦点输入框。",
        "click" => "按坐标或选择器点击。",
        "double-click" => "按坐标双击。",
        "long-click" => "按坐标或选择器长按。",
        "swipe" => "按坐标或方向滑动。",
        "drag" => "按坐标拖动。",
        "exists" => "判断控件是否存在。",
        "wait" => "等待控件出现或消失。",
        "scroll" => "滚动控件，或滚到指定文字。",
        "open-notification" => "打开通知栏。",
        "open-quick-settings" => "打开快捷设置。",
        "open-url" => "在设备上打开一个链接。",
        _ => "未知命令。",
    }
}

/// 子命令帮助里的参数说明。
fn detail(name: &str) -> String {
    match name {
        "server" => option("--foreground", "留在前台运行。start-server 会自己带上它。", "u2cli server --foreground"),
        "screenshot" => option("FILENAME", "保存当前画面的文件路径。省略时写到 screenshot.jpg。", "u2cli screenshot /tmp/screen.jpg"),
        "dump-hierarchy" => {
            option("--raw", "输出未压缩层级。", "u2cli dump-hierarchy --raw")
                + &option("--compressed", "输出压缩后的层级。这是默认行为。", "u2cli dump-hierarchy --compressed")
                + &option("--max-depth N", "限制层级深度。", "u2cli dump-hierarchy --max-depth 5")
                + &option("-o, --output FILE", "把层级写到文件。", "u2cli dump-hierarchy -o /tmp/hierarchy.txt")
        }
        "shell" => {
            option("COMMAND", "要在设备上执行的命令。", "u2cli shell echo hello")
                + &option("--timeout SECONDS", "最长等待秒数。", "u2cli shell --timeout 10 echo hello")
        }
        "app-start" => {
            option("PACKAGE", "要启动的包名。", "u2cli app-start com.android.settings")
                + &option("--activity ACTIVITY", "指定要打开的 Activity。", "u2cli app-start com.android.settings --activity .Settings")
                + &option("--wait", "等到应用进入前台。", "u2cli app-start com.android.settings --wait")
                + &option("--stop", "启动前先停止该应用。", "u2cli app-start com.android.settings --stop")
        }
        "app-list" => option("--filter TEXT", "按包名片段过滤。", "u2cli app-list --filter -3"),
        "app-stop" => {
            option("PACKAGE", "要停止的包名。", "u2cli app-stop com.example.app")
                + &option("--all", "停止第三方应用。", "u2cli app-stop --all")
        }
        "app-install" => option("APK", "要安装的 APK 路径。", "u2cli app-install ./app-debug.apk"),
        "app-uninstall" => option("PACKAGE", "要卸载的包名。", "u2cli app-uninstall com.example.app"),
        "app-clear" => option("PACKAGE", "要清空数据的包名。", "u2cli app-clear com.example.app"),
        "press" => option("KEY", "按键名，例如 home、back、enter。", "u2cli press home"),
        "send-keys" => {
            option("TEXT", "要输入的文字。默认先清空输入框。", "u2cli send-keys hello")
                + &option("--no-clear", "输入前保留输入框里的原文。", "u2cli send-keys --no-clear hello")
        }
        "click" => option("X Y", "点击位置。可以用像素，也可以用 0 到 1 的比例。", "u2cli click 500 1200") + &selector_options("click"),
        "double-click" => {
            option("X Y", "双击位置。可以用像素或比例。", "u2cli double-click 500 1200")
                + &option("--duration SECONDS", "两次点击之间的间隔。", "u2cli double-click 500 1200 --duration 0.1")
        }
        "long-click" => {
            option("X Y", "长按位置。可以用像素或比例。", "u2cli long-click 500 1200")
                + &option("--duration SECONDS", "按住的秒数。", "u2cli long-click 500 1200 --duration 1")
                + &selector_options("long-click")
        }
        "swipe" => {
            option("FX FY TX TY", "起点和终点。也可以只写 up、down、left、right。", "u2cli swipe 500 1800 500 400 --duration 0.3")
                + &option("--duration SECONDS", "滑动耗时，单位是秒。", "u2cli swipe up --duration 0.3")
                + &option("--steps N", "滑动分成多少步。", "u2cli swipe 500 1800 500 400 --steps 20")
                + &option("--scale N", "方向滑动时使用的距离比例。", "u2cli swipe up --scale 0.5")
        }
        "drag" => {
            option("SX SY EX EY", "拖动的起点和终点。", "u2cli drag 300 1200 800 1200")
                + &option("--duration SECONDS", "拖动耗时，单位是秒。", "u2cli drag 300 1200 800 1200 --duration 0.5")
        }
        "exists" => selector_options("exists"),
        "wait" => option("--gone", "改为等待控件消失。", "u2cli wait --text Loading --gone --timeout 10") + &selector_options("wait"),
        "scroll" => {
            option("--direction vert|horiz", "滚动方向，默认垂直。", "u2cli scroll --scrollable --direction vert")
                + &option("--action forward|backward|toEnd|toBeginning", "滚动动作。", "u2cli scroll --scrollable --action forward")
                + &option("--max-swipes N", "最多滚动次数。", "u2cli scroll --scrollable --max-swipes 5")
                + &option("--to-text TEXT", "滚到包含这段文字的位置。", "u2cli scroll --scrollable --to-text Settings")
                + &selector_options("scroll")
        }
        "open-url" => option("URL", "要打开的链接。", "u2cli open-url https://example.com"),
        "start-server" => no_args("u2cli start-server"),
        "kill-server" => no_args("u2cli kill-server"),
        "server-status" => no_args("u2cli server-status"),
        "install-service" => no_args("u2cli install-service"),
        "device-info" => no_args("u2cli device-info"),
        "window-size" => no_args("u2cli window-size"),
        "app-current" => no_args("u2cli app-current"),
        "clear-text" => no_args("u2cli clear-text"),
        "open-notification" => no_args("u2cli open-notification"),
        "open-quick-settings" => no_args("u2cli open-quick-settings"),
        _ => "不需要额外参数。\n".to_string(),
    }
}
