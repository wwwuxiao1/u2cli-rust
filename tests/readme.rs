/// README 用中文说明打包、安装和全部命令，命令名保持英文。
#[test]
fn readme_documents_commands_and_packaging() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join("README.md")).expect("README");
    for needle in ["dist/u2cli", "scripts/package.sh", "install-service", "U2CLI_DEVICE_SERIAL", "-s", "--server-port", "--device-port", "打包"] {
        assert!(text.contains(needle), "missing {needle}");
    }
    for name in [
        "server", "start-server", "kill-server", "server-status",
        "install-service", "device-info", "window-size", "screenshot", "dump-hierarchy", "app-current", "shell",
        "app-start", "app-list", "app-stop", "app-install", "app-uninstall", "app-clear",
        "press", "send-keys", "clear-text", "click", "double-click", "long-click", "swipe", "drag",
        "exists", "wait", "scroll",
        "open-notification", "open-quick-settings", "open-url",
    ] {
        assert!(text.contains(&format!("`{name}`")) || text.contains(&format!("u2cli {name}")), "missing command {name}");
    }
}
