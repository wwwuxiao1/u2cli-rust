/// 命令帮助里的分组。只列出已经注册的子命令。
pub struct Group {
    pub title: &'static str,
    pub commands: &'static [&'static str],
}

/// 全部命令分组，顺序与现有帮助一致。
pub fn groups() -> &'static [Group] {
    &[
        Group { title: "server commands:", commands: &["server", "start-server", "kill-server", "server-status"] },
        Group { title: "device commands:", commands: &["device-info", "window-size", "screenshot", "dump-hierarchy", "app-current", "shell"] },
        Group { title: "app commands:", commands: &["app-start", "app-list", "app-stop", "app-install", "app-uninstall", "app-clear"] },
        Group { title: "input commands:", commands: &["press", "send-keys", "clear-text", "click", "double-click", "long-click", "swipe", "drag"] },
        Group { title: "selector commands:", commands: &["exists", "wait", "scroll"] },
        Group { title: "system ui commands:", commands: &["open-notification", "open-quick-settings", "open-url"] },
    ]
}

/// 生成根命令或子命令的英文帮助。
pub fn render(invocation: &crate::cli::Invocation) -> String {
    if let Some(name) = &invocation.help_topic {
        return subcommand_help(name);
    }
    let mut text = String::from("Usage: u2cli [OPTIONS] COMMAND [ARGS]...\n\nOptions:\n  -d, --debug\n  -s, --serial SERIAL\n  --server-host HOST\n  --server-port PORT\n  --device-port PORT\n  -h, --help\n");
    for group in groups() {
        text.push_str("\n");
        text.push_str(group.title);
        text.push('\n');
        for name in group.commands {
            text.push_str("  ");
            text.push_str(name);
            text.push('\n');
        }
    }
    text
}

/// 生成单个子命令的帮助，供参数说明被测试识别。
fn subcommand_help(name: &str) -> String {
    let body = match name {
        "app-start" => "Options:\n  --activity ACTIVITY\n  --wait\n  --stop\n",
        "exists" | "wait" | "click" | "long-click" | "scroll" => "Options:\n  --text TEXT\n  --child-text TEXT\n  --child KEY=VALUE\n",
        "screenshot" => "Arguments:\n  FILENAME\n",
        "dump-hierarchy" => "Options:\n  --compressed\n  --max-depth N\n  -o, --output FILE\n  --raw\n",
        "shell" => "Options:\n  --timeout SECONDS\n",
        "swipe" => "Options:\n  --duration SECONDS\n  --steps N\n  --scale N\n",
        "send-keys" => "Options:\n  --no-clear\n",
        _ => "",
    };
    format!("Usage: u2cli {name} [OPTIONS] [ARGS]...\n\n{body}")
}
