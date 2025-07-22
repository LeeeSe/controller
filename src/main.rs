use enigo::{Axis, Enigo, Mouse, Settings};
use std::sync::{Arc, Mutex};
use std::{thread, time};

// 模块导入
mod config;
mod error;
mod hid;
mod input_handler;

use config::{ButtonMappingConfig, ControllerConfig};
use error::{ControllerError, ControllerResult, ErrorContext, RecoveryStrategy};
use hid::HidController;
use input_handler::InputHandler;

/// 滚动处理器，使用独立的 Enigo 实例
struct ScrollHandler {
    enigo: Enigo,
}

impl ScrollHandler {
    fn new() -> ControllerResult<Self> {
        let enigo = Enigo::new(&Settings::default()).map_err(|e| {
            ControllerError::InitializationFailed(format!("滚动处理器Enigo初始化失败: {}", e))
        })?;
        Ok(Self { enigo })
    }
}

/// "步调器"线程用于发送平滑滚动事件
fn run_pacer_loop(scroll_power: Arc<Mutex<f64>>, config: ControllerConfig) {
    let mut scroll_handler = match ScrollHandler::new() {
        Ok(handler) => handler,
        Err(e) => {
            eprintln!("在步调器线程中初始化滚动处理器时出错: {}", e);
            return;
        }
    };

    let loop_interval = time::Duration::from_secs_f64(1.0 / config.pacer_loop_hz as f64);

    loop {
        let power = match scroll_power.lock() {
            Ok(guard) => *guard,
            Err(_) => {
                eprintln!("无法获取滚动力度锁");
                continue;
            }
        };

        if power.abs() > 0.01 {
            let scroll_delta = power.round() as i32;
            if scroll_delta != 0 {
                // 正值向下滚动，负值向上滚动
                if let Err(e) = scroll_handler
                    .enigo
                    .scroll(-1 * scroll_delta, Axis::Vertical)
                {
                    eprintln!("滚动时出错: {}", e);
                }
            }
        }
        thread::sleep(loop_interval);
    }
}

/// 打印操作说明
fn print_instructions(button_mapping: &ButtonMappingConfig) {
    println!("设备已连接！控制器现在可以控制鼠标了。");
    println!(" - 左摇杆：移动光标");
    println!(" - 右摇杆上/下：滚动页面（平滑且松开时停止）");
    println!(" - 右摇杆左/右：导航前进/后退（在浏览器等应用中）");
    println!(" - 按住LT + 移动控制器：陀螺仪瞄准");

    println!(
        " - A 按钮：{}",
        format_button_action(&button_mapping.button_a)
    );
    println!(
        " - B 按钮：{}",
        format_button_action(&button_mapping.button_b)
    );
    println!(
        " - X 按钮：{}",
        format_button_action(&button_mapping.button_x)
    );
    println!(
        " - Y 按钮：{}",
        format_button_action(&button_mapping.button_y)
    );
    println!(
        " - LB 按钮：{}",
        format_button_action(&button_mapping.button_lb)
    );
    println!(
        " - RB 按钮：{}",
        format_button_action(&button_mapping.button_rb)
    );

    println!("按 Ctrl+C 退出程序。");
    println!("{}", "-".repeat(40));
}

/// 格式化按钮动作描述
fn format_button_action(action: &config::ButtonAction) -> String {
    match action {
        config::ButtonAction::LeftClick => "左鼠标点击".to_string(),
        config::ButtonAction::RightClick => "右鼠标点击".to_string(),
        config::ButtonAction::CloseWindow => "关闭窗口 (Cmd+W)".to_string(),
        config::ButtonAction::MissionControl => "调度中心".to_string(),
        config::ButtonAction::PrevTab => "上一个标签页".to_string(),
        config::ButtonAction::NextTab => "下一个标签页".to_string(),
        config::ButtonAction::CustomShortcut { modifiers, key } => {
            format!("自定义快捷键: {}+{}", modifiers.join("+"), key)
        }
        config::ButtonAction::None => "无操作".to_string(),
    }
}

/// 处理错误并根据恢复策略执行相应操作
fn handle_error_with_recovery(error: ControllerError) -> bool {
    let recovery_strategy = ErrorContext::suggest_recovery_strategy(&error);
    let context = ErrorContext::new(error, recovery_strategy);

    eprintln!("错误: {}", context.error);
    println!("建议: {}", context.user_message);

    match &context.recovery_strategy {
        RecoveryStrategy::Retry {
            max_attempts,
            delay_ms,
        } => {
            println!("将在 {}ms 后重试，最多重试 {} 次", delay_ms, max_attempts);
            thread::sleep(time::Duration::from_millis(*delay_ms));
            false // 继续运行
        }
        RecoveryStrategy::Reconnect => {
            println!("正在尝试重新连接设备...");
            thread::sleep(time::Duration::from_millis(1000));
            false // 继续运行
        }
        RecoveryStrategy::Skip => {
            println!("跳过当前操作，继续运行...");
            false // 继续运行
        }
        RecoveryStrategy::Exit => {
            println!("程序将退出。");
            true // 退出程序
        }
    }
}

/// 加载配置文件
fn load_configuration() -> ControllerResult<(ControllerConfig, ButtonMappingConfig)> {
    let config_path =
        ControllerConfig::default_config_path().map_err(|e| ControllerError::Config(e))?;

    let config = ControllerConfig::load_or_create_default(&config_path)
        .map_err(|e| ControllerError::Config(e))?;

    config.validate().map_err(|e| ControllerError::Config(e))?;

    let button_mapping = ButtonMappingConfig::default();

    Ok((config, button_mapping))
}

/// 主控制循环
fn run_control_loop(
    controller: HidController,
    mut input_handler: InputHandler,
    scroll_power: Arc<Mutex<f64>>,
    config: &ControllerConfig,
) -> ControllerResult<()> {
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 5;

    loop {
        match controller.read_state(config.analog_trigger_threshold) {
            Ok(Some(state)) => {
                retry_count = 0; // 重置重试计数

                // 处理输入
                if let Err(e) = input_handler.handle_input(&state, &scroll_power) {
                    if handle_error_with_recovery(e) {
                        return Err(ControllerError::InitializationFailed(
                            "用户选择退出".to_string(),
                        ));
                    }
                }
            }
            Ok(None) => continue, // 没有新数据，继续下一次循环
            Err(e) => {
                retry_count += 1;

                if retry_count >= MAX_RETRIES {
                    return Err(ControllerError::DeviceDisconnected);
                }

                let controller_error = ControllerError::HidDevice(e.to_string());
                if handle_error_with_recovery(controller_error) {
                    return Err(ControllerError::InitializationFailed(
                        "用户选择退出".to_string(),
                    ));
                }
            }
        }
    }
}

fn main() {
    println!("正在启动Xbox手柄控制器应用程序...");

    // 1. 加载配置
    let (config, button_mapping) = match load_configuration() {
        Ok((config, button_mapping)) => {
            println!("配置加载成功");
            (config, button_mapping)
        }
        Err(e) => {
            if handle_error_with_recovery(e) {
                return;
            }
            // 使用默认配置继续
            println!("使用默认配置继续运行");
            (ControllerConfig::default(), ButtonMappingConfig::default())
        }
    };

    println!("正在搜索 {}...", HidController::get_device_info());

    // 2. 初始化输入处理器
    let input_handler = match InputHandler::new(config.clone(), button_mapping.clone()) {
        Ok(handler) => handler,
        Err(e) => {
            handle_error_with_recovery(e);
            return;
        }
    };

    println!("{}", "-".repeat(40));

    // 3. 初始化 HID 控制器
    let controller = match HidController::new() {
        Ok(ctrl) => ctrl,
        Err(e) => {
            handle_error_with_recovery(e);
            return;
        }
    };

    print_instructions(&button_mapping);

    // 4. 启动滚动步调器线程
    let scroll_power = Arc::new(Mutex::new(0.0));
    let pacer_power = Arc::clone(&scroll_power);
    let pacer_config = config.clone();
    thread::spawn(move || run_pacer_loop(pacer_power, pacer_config));

    // 5. 运行主控制循环
    if let Err(e) = run_control_loop(controller, input_handler, scroll_power, &config) {
        handle_error_with_recovery(e);
    }

    println!("应用程序已退出。");
}
