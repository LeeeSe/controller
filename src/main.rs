use enigo::{Axis, Enigo, Mouse, Settings};
use std::sync::{Arc, Mutex};
use std::{thread, time};

// 模块导入
mod config;
mod connection_manager;
mod error;
mod hid;
mod input_handler;

use config::{ButtonMappingConfig, ControllerConfig};
use connection_manager::ConnectionManager;
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
                    .smooth_scroll(-1 * scroll_delta, Axis::Vertical)
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
    println!(
        " - 上方向键：{}",
        format_button_action(&button_mapping.dpad_up)
    );
    println!(
        " - 下方向键：{}",
        format_button_action(&button_mapping.dpad_down)
    );
    println!(
        " - 左方向键：{}",
        format_button_action(&button_mapping.dpad_left)
    );
    println!(
        " - 右方向键：{}",
        format_button_action(&button_mapping.dpad_right)
    );
    println!(
        " - LT + X 组合键：{}",
        format_button_action(&button_mapping.lt_x_combo)
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
        config::ButtonAction::QuitApp => "退出应用程序 (Cmd+Q)".to_string(),
        config::ButtonAction::NewTab => "新建标签页 (Cmd+T)".to_string(),
        config::ButtonAction::Refresh => "刷新页面 (Cmd+R)".to_string(),
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

/// 主控制循环（支持自动重连）
fn run_control_loop_with_reconnect(
    mut connection_manager: ConnectionManager,
    mut input_handler: InputHandler,
    scroll_power: Arc<Mutex<f64>>,
    config: &ControllerConfig,
    button_mapping: &ButtonMappingConfig,
) -> ControllerResult<()> {
    let mut current_controller: Option<HidController> = None;
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 5;

    // 尝试初始连接
    match connection_manager.initial_connect() {
        Ok(controller) => {
            current_controller = Some(controller);
            print_instructions(button_mapping);
        }
        Err(e) => {
            if !connection_manager.should_continue() {
                return Err(e);
            }
            println!("初始连接失败，开始等待设备连接...");
        }
    }

    loop {
        // 检查是否应该继续运行
        if !connection_manager.should_continue() {
            break;
        }

        // 如果没有控制器，尝试重连
        if current_controller.is_none() {
            if let Some(reconnect_result) = connection_manager.try_reconnect() {
                match reconnect_result {
                    Ok(controller) => {
                        current_controller = Some(controller);
                        retry_count = 0;
                        print_instructions(button_mapping);
                        continue;
                    }
                    Err(_) => {
                        connection_manager.wait_reconnect_interval();
                        continue;
                    }
                }
            } else {
                // 重连被禁用或达到最大重试次数
                break;
            }
        }

        // 有控制器时，尝试读取状态
        if let Some(controller) = &current_controller {
            match controller.read_state(config.analog_trigger_threshold) {
                Ok(Some(state)) => {
                    retry_count = 0;

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
                Err(_) => {
                    retry_count += 1;

                    if retry_count >= MAX_RETRIES {
                        // 设备断开
                        connection_manager.handle_disconnect();
                        current_controller = None;
                        retry_count = 0;
                    }
                }
            }
        }
    }

    Ok(())
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

    // 2. 初始化连接管理器
    let connection_manager = ConnectionManager::new(&config);

    // 3. 初始化输入处理器
    let input_handler = match InputHandler::new(config.clone(), button_mapping.clone()) {
        Ok(handler) => handler,
        Err(e) => {
            handle_error_with_recovery(e);
            return;
        }
    };

    println!("{}", "-".repeat(40));

    // 4. 启动滚动步调器线程
    let scroll_power = Arc::new(Mutex::new(0.0));
    let pacer_power = Arc::clone(&scroll_power);
    let pacer_config = config.clone();
    thread::spawn(move || run_pacer_loop(pacer_power, pacer_config));

    // 5. 运行主控制循环（支持自动重连）
    if let Err(e) = run_control_loop_with_reconnect(
        connection_manager,
        input_handler,
        scroll_power,
        &config,
        &button_mapping,
    ) {
        handle_error_with_recovery(e);
    }

    println!("应用程序已退出。");
}
