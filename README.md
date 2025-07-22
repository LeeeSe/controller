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

## 架构设计

项目采用现代化的模块化架构设计，实现了清晰的职责分离和高度的可扩展性：

### 核心模块架构

#### HID底层模块 (`hid.rs`)
负责所有与硬件设备交互的底层功能：
- **设备识别**: 定义Xbox手柄的VID/PID常量
- **数据解析**: 从HID报告中解析手柄状态
- **设备管理**: 设备连接、数据读取、错误处理

**主要组件**:
- `ControllerState`: 封装手柄所有输入状态
- `HidController`: HID设备管理器，提供类型安全的接口

#### 输入处理模块 (`input_handler.rs`)
专注于输入映射和系统操作：
- **按钮映射**: 可配置的按钮功能映射
- **光标控制**: 摇杆和陀螺仪的精确控制
- **滚动处理**: 平滑滚动和导航功能
- **快捷键执行**: 支持自定义快捷键组合

#### 配置管理模块 (`config.rs`)
提供完整的配置系统：
- **类型安全配置**: 使用Rust结构体定义配置
- **文件持久化**: 支持TOML格式配置文件
- **验证机制**: 配置参数合理性检查
- **默认值管理**: 智能的默认配置生成

#### 错误处理模块 (`error.rs`)
统一的错误管理系统：
- **自定义错误类型**: 详细的错误分类
- **恢复策略**: 智能的错误恢复机制
- **用户友好消息**: 清晰的错误提示和建议

#### 主程序模块 (`main.rs`)
应用程序的协调层：
- **初始化流程**: 配置加载、设备连接
- **主控制循环**: 事件处理和错误恢复
- **线程管理**: 滚动处理的独立线程

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

### 按钮映射配置
支持灵活的按钮功能映射，包括：
- 鼠标点击 (`LeftClick`, `RightClick`)
- 系统功能 (`CloseWindow`, `MissionControl`)
- 标签页操作 (`PrevTab`, `NextTab`)
- 自定义快捷键组合
