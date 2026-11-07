# u2cli

在电脑上操作已连接 Android 设备的命令行。运行时不依赖 Python。手机端服务由这个程序自己安装和拉起。访问设备使用系统里的 `adb`，不会把 adb 打进文件。

## 前置条件

- 当前 macOS
- 已安装 Rust 工具链，并且 `cargo` 在 `PATH` 中
- 已安装 `adb`，设备处于 `device` 状态
- 多台设备同时连接时，用 `-s` 指定序列号

## 打包

在仓库根目录执行：

```sh
scripts/package.sh
```

成功后得到单一可执行文件 `dist/u2cli`。脚本只写这个路径，不写入系统路径。

```sh
dist/u2cli --help
```

下面的例子把命令写成 `u2cli`。使用打包产物时，换成 `dist/u2cli`。

## 全局选项

- `-d`：调试
- `-s SERIAL`：指定设备
- `--server-host HOST`：本机服务地址，默认 `127.0.0.1`
- `--server-port PORT`：本机服务端口，默认 `17913`
- `--device-port PORT`：手机端服务端口，默认 `9008`
- `-h`、`--help`：帮助

帮助和错误信息是英文。成功时退出码为 0。用法错误退出码为 2。执行失败退出码为 1，标准错误以 `error:` 开头。

## 安装手机端服务

```sh
u2cli install-service
u2cli -s SERIAL install-service
```

`install-service` 只安装并拉起手机端 HTTP 服务。服务已在响应且设备上的文件与内置文件一致时，命令直接成功，不重新推送、不重启。其他设备命令发现服务没有响应时，也会自动安装。这条命令不安装输入法。

## 真机测试

未设置环境变量时，真机用例直接通过，不访问手机。要对一台设备验证基本命令：

```sh
U2CLI_DEVICE_SERIAL=SERIAL cargo test --test device
```

覆盖 `install-service`、`device-info`、`window-size`、`screenshot`、`dump-hierarchy`、`app-current`、`press`、坐标 `click` 和坐标 `swipe`。

## 命令

### 本机服务

- `server`：在前台运行本机服务。`u2cli server`
- `start-server`：启动本机服务。已在运行时会说明。`u2cli start-server`
- `server-status`：查看本机服务是否在运行。`u2cli server-status`
- `kill-server`：停止本机服务。`u2cli kill-server`

### 设备

- `install-service`：安装并拉起手机端服务。`u2cli install-service`
- `device-info`：查看序列号、系统和机型。`u2cli device-info`
- `window-size`：查看屏幕宽高。`u2cli window-size`
- `screenshot`：保存当前画面。`u2cli screenshot /tmp/screen.jpg`
- `dump-hierarchy`：输出压缩后的界面层级。完整层级用 `--raw`。`u2cli dump-hierarchy`
- `app-current`：查看前台应用。`u2cli app-current`
- `shell`：在设备上执行命令，可用 `--timeout`。`u2cli shell --timeout 10 pm list packages`

### 应用

- `app-start`：按包名启动，可选 `--activity`、`--wait`、`--stop`。`u2cli app-start com.android.settings --activity .Settings --wait`
- `app-list`：列出已安装包，可用 `--filter`。`u2cli app-list --filter -3`
- `app-stop`：停止应用。`--all` 停止第三方应用。`u2cli app-stop com.example.app`
- `app-install`：安装 APK。`u2cli app-install ./app-debug.apk`
- `app-uninstall`：卸载应用。`u2cli app-uninstall com.example.app`
- `app-clear`：清空应用数据。`u2cli app-clear com.example.app`

### 输入和手势

- `press`：发送按键。`u2cli press home`
- `send-keys`：向焦点输入框打字，默认先清空。`--no-clear` 保留原文。设备上需要已有可用输入法。`u2cli send-keys "hello"`
- `clear-text`：清空焦点输入框。`u2cli clear-text`
- `click`：按坐标、比例或选择器点击。`u2cli click 500 1200`
- `double-click`：按坐标双击。`u2cli double-click 500 1200`
- `long-click`：长按坐标或选择器。`u2cli long-click 500 1200`
- `swipe`：按坐标或方向滑动。`u2cli swipe 500 1800 500 400 --duration 0.3`
- `drag`：按坐标拖动。`u2cli drag 300 1200 800 1200`

### 选择器

`click`、`long-click`、`exists`、`wait`、`scroll` 可以使用 `--text`、`--text-contains`、`--text-matches`、`--text-starts-with`、`--description`、`--description-contains`、`--resource-id`、`--class-name`、`--package`，以及 `--selected`、`--focused`、`--enabled`、`--scrollable`、`--clickable`、`--checked`、`--checkable`、`--instance`、`--index`。子控件用 `--child KEY=VALUE` 或 `--child-text`。

- `exists`：判断控件是否存在。`u2cli exists --text Gmail --timeout 3`
- `wait`：等待控件出现。`--gone` 表示等待消失。`u2cli wait --text Loading --gone --timeout 10`
- `scroll`：滚动。可指定方向、动作、最大次数，或滚到某段文字。`u2cli scroll --scrollable --to-text Settings`

### 系统界面

- `open-notification`：打开通知栏。`u2cli open-notification`
- `open-quick-settings`：打开快捷设置。`u2cli open-quick-settings`
- `open-url`：在设备上打开链接。`u2cli open-url https://example.com`
