use std::process::Command;

/// 打包脚本生成单一 u2cli，并且不安装到系统路径。
#[test]
fn package_script_writes_one_binary() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = root.join("scripts/package.sh");
    let output = Command::new(&script).current_dir(&root).output().expect("run package script");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("dist/u2cli"), "{text}");
    let binary = root.join("dist/u2cli");
    assert!(binary.is_file());
    let help = Command::new(&binary).arg("--help").output().expect("run packaged u2cli");
    assert!(help.status.success());
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("install-service"), "{help_text}");
    let script_text = std::fs::read_to_string(&script).unwrap_or_default();
    assert!(!script_text.contains("/usr/local/bin"));
}
