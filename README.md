# 手柄控制器项目

这是一个使用黑武士4pro手柄控制macOS系统光标和执行系统操作的Rust项目。

## 项目结构

```
controller/
├── src/
│   ├── main.rs          # 主程序入口和控制循环
│   ├── hid.rs           # HID底层设备通信
│   ├── input_handler.rs # 输入处理和映射逻辑
│   ├── config.rs        # 配置管理系统
│   └── error.rs         # 自定义错误类型和处理
├── config.example.toml  # 配置文件示例
├── Cargo.toml
├── Cargo.lock
└── README.md
```

## 功能特性

### 输入控制
- **左摇杆**: 控制鼠标光标移动
- **右摇杆上下**: 页面滚动（平滑滚动）
- **右摇杆左右**: 浏览器前进/后退导航
- **LT + 陀螺仪**: 精确光标控制

### 按钮功能
- **A/B键**: 鼠标左右键
- **LB/RB键**: 切换标签页 (Cmd+Shift+[/])
- **X键**: 关闭当前窗口 (Cmd+W)
- **Y键**: 打开调度中心 (Mission Control)

### 自动重连功能
- **断线重连**: 手柄休眠或断开后自动等待重新连接
- **设备扫描**: 重连时自动扫描所有支持的设备ID
- **状态提示**: 实时显示连接状态和重连进度
- **智能恢复**: 重连成功后立即恢复所有功能

## 依赖项

### 核心依赖
- `hidapi`: HID设备通信
- `enigo`: 跨平台输入模拟
- `serde`: 序列化/反序列化支持
- `toml`: 配置文件格式支持
- `dirs`: 系统目录路径获取

### 开发依赖
- `tempfile`: 测试用临时文件

### 自定义补丁
- `enigo`: 使用自定义分支以支持特定功能

## 使用方法

### 基本使用
```bash
# 编译项目
cargo build --release

# 运行程序
cargo run

# 运行测试
cargo test
```

### 配置管理
```bash
# 复制示例配置文件
cp config.example.toml ~/.config/controller/config.toml

# 编辑配置文件
vim ~/.config/controller/config.toml
```

程序首次运行时会自动创建默认配置文件，无需手动配置。

## 配置系统

### 配置文件位置
- **默认路径**: `~/.config/controller/config.toml`
- **示例文件**: `config.example.toml`

### 主要配置参数

#### 基本阈值设置
```toml
analog_trigger_threshold = 20    # 模拟扳机阈值 (0-255)
joystick_deadzone = 1000        # 左摇杆死区 (0-32767)
right_joystick_deadzone = 5000  # 右摇杆死区 (0-32767)
gyro_deadzone = 10              # 陀螺仪死区 (0-32767)
nav_trigger_threshold = 32001   # 导航触发阈值 (0-32767)
```

#### 灵敏度设置
```toml
joystick_sensitivity = 15.0        # 摇杆灵敏度 (5.0-30.0)
gyro_sensitivity = 0.08            # 陀螺仪灵敏度 (0.01-0.2)
direct_scroll_sensitivity = 20.0   # 滚动灵敏度 (5.0-50.0)
```

#### 高级设置
```toml
dominant_axis_factor = 1.5  # 主导轴系数 (>1.0)
pacer_loop_hz = 75         # 步调器频率 (30-120 Hz)
```

#### 重连配置
```toml
[reconnection]
enable_auto_reconnect = true        # 启用自动重连
reconnect_interval_ms = 2000        # 重连间隔 (毫秒)
max_reconnect_attempts = 0          # 最大重连次数 (0=无限)
show_reconnect_messages = true      # 显示重连消息
max_silent_failures = 5            # 静默失败次数阈值
```

### 按钮映射配置
支持灵活的按钮功能映射，包括：
- 鼠标点击 (`LeftClick`, `RightClick`)
- 系统功能 (`CloseWindow`, `MissionControl`)
- 标签页操作 (`PrevTab`, `NextTab`)
- 自定义快捷键组合

## 自动重连系统

### 功能概述
程序支持智能的自动重连功能，解决手柄休眠导致程序退出的问题。当手柄断开连接时，程序会：

1. **检测断开**: 自动检测手柄连接状态
2. **等待重连**: 定期扫描并尝试重新连接
3. **恢复功能**: 重连成功后立即恢复所有控制功能
4. **状态提示**: 显示清晰的连接状态信息

### 使用场景
- **观看视频**: 手柄休眠后无需重启程序
- **长时间使用**: 电池耗尽后更换电池可直接继续使用
- **多设备环境**: 支持切换不同的Xbox兼容手柄
- **不稳定连接**: 自动处理蓝牙连接不稳定的情况

### 配置选项详解

#### 基本设置
- `enable_auto_reconnect`: 控制是否启用自动重连功能
  - `true`: 启用，手柄断开后等待重连
  - `false`: 禁用，手柄断开后程序退出

#### 重连策略
- `reconnect_interval_ms`: 重连尝试的时间间隔
  - 推荐值: 1000-5000毫秒
  - 较短间隔: 快速恢复，但可能增加系统负担
  - 较长间隔: 节省资源，但恢复较慢

- `max_reconnect_attempts`: 最大重连尝试次数
  - `0`: 永不停止重连（推荐）
  - `>0`: 达到次数后停止重连并退出程序

#### 用户体验
- `show_reconnect_messages`: 控制重连消息显示
  - `true`: 显示详细的重连状态信息
  - `false`: 静默重连，减少控制台输出

- `max_silent_failures`: 静默失败次数阈值
  - 连续失败超过此次数后开始显示消息
  - 避免重连消息过于频繁

### 支持的设备变化
自动重连系统能够处理以下设备变化情况：
- 手柄设备ID变化（休眠重启后）
- 不同型号的Xbox兼容手柄切换
- USB/蓝牙连接方式变化
- 多个手柄设备的自动选择

### 故障排除

#### 重连不工作
```bash
# 检查配置文件
cat ~/.config/controller/config.toml | grep -A 10 "\[reconnection\]"

# 确认自动重连已启用
enable_auto_reconnect = true
```

#### 重连消息过多
```toml
# 增加静默失败次数
max_silent_failures = 10

# 或者禁用消息显示
show_reconnect_messages = false
```

#### 重连间隔太频繁
```toml
# 增加重连间隔
reconnect_interval_ms = 5000  # 5秒间隔
```

### 技术实现
- **状态管理**: 使用状态机模式管理连接状态
- **设备扫描**: 每次重连时重新扫描所有支持的设备
- **错误恢复**: 智能的错误处理和恢复策略
- **线程安全**: 多线程环境下的安全重连处理
