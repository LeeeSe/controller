use enigo::{
    Axis, Button as EnigoButton, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::{thread, time};

// hid 和 mouse_position 模块假设在其他文件中，如原始结构所示。
mod hid;
use hid::{
    BUTTON_A, BUTTON_B, BUTTON_LB, BUTTON_RB, BUTTON_X, BUTTON_Y, ControllerState, HidController,
};

// --- 应用程序配置和阈值 ---
const ANALOG_TRIGGER_THRESHOLD: u8 = 20;
const JOYSTICK_DEADZONE: i16 = 1000;
const RIGHT_JOYSTICK_DEADZONE: i16 = 5000;
const GYRO_DEADZONE: i16 = 10;
const NAV_TRIGGER_THRESHOLD: i16 = 32001; // 页面导航的触发阈值
const DOMINANT_AXIS_FACTOR: f64 = 1.5;

const JOYSTICK_SENSITIVITY: f64 = 15.0;
const GYRO_SENSITIVITY: f64 = 0.08;
const DIRECT_SCROLL_SENSITIVITY: f64 = 20.0;
const PACER_LOOP_HZ: u64 = 75;

/// 应用死区和二次加速曲线
fn apply_deadzone_and_curve(value: i16, deadzone: i16) -> f64 {
    let val_f = value as f64;
    let deadzone_f = deadzone as f64;
    if val_f.abs() < deadzone_f {
        return 0.0;
    }

    let max_val = i16::MAX as f64;
    let normalized_val = (val_f.abs() - deadzone_f) / (max_val - deadzone_f);
    normalized_val.powi(2).copysign(val_f)
}

/// "步调器"线程用于发送平滑滚动事件
fn run_pacer_loop(scroll_power: Arc<Mutex<f64>>) {
    // 每个需要发送输入的线程都需要自己的 Enigo 实例。
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(enigo) => enigo,
        Err(e) => {
            eprintln!("在步调器线程中初始化 Enigo 时出错: {}", e);
            return;
        }
    };
    let loop_interval = time::Duration::from_secs_f64(1.0 / PACER_LOOP_HZ as f64);
    loop {
        let power = *scroll_power.lock().unwrap();
        if power.abs() > 0.01 {
            let scroll_delta = power.round() as i32;
            if scroll_delta != 0 {
                // 正值向下滚动，负值向上滚动
                if let Err(e) = enigo.scroll(-1 * scroll_delta, Axis::Vertical) {
                    eprintln!("滚动时出错: {}", e);
                }
            }
        }
        thread::sleep(loop_interval);
    }
}

/// 打印操作说明
fn print_instructions() {
    println!("设备已连接！控制器现在可以控制鼠标了。");
    println!(" - 左摇杆：移动光标");
    println!(" - 右摇杆上/下：滚动页面（平滑且松开时停止）");
    println!(" - 右摇杆左/右：导航前进/后退（在浏览器等应用中）");
    println!(" - 按住LT + 移动控制器：陀螺仪瞄准");
    println!(" - A/B 按钮：左/右鼠标点击");
    println!(" - LB/RB 按钮：切换标签页");
    println!(" - X 按钮：关闭窗口 (Cmd+W)");
    println!(" - Y 按钮：调度中心");
    println!("按 Ctrl+C 退出程序。");
    println!("{}", "-".repeat(40));
}

/// 使用 Enigo 处理按钮按下和释放事件
fn handle_button_events(
    enigo: &mut Enigo,
    newly_pressed: &HashSet<u8>,
    newly_released: &HashSet<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    for &button in newly_pressed {
        match button {
            BUTTON_A => enigo.button(EnigoButton::Left, Press)?,
            BUTTON_B => enigo.button(EnigoButton::Right, Press)?,
            BUTTON_X => {
                // 关闭窗口 (Cmd+W)
                enigo.key(Key::Meta, Press)?;
                enigo.key(Key::Unicode('w'), Click)?;
                enigo.key(Key::Meta, Release)?;
            }
            BUTTON_Y => {
                // 调度中心
                enigo.key(Key::MissionControl, Click)?;
            }
            BUTTON_LB => {
                // 上一个标签页 (Cmd+Shift+[)
                enigo.key(Key::Meta, Press)?;
                enigo.key(Key::Shift, Press)?;
                enigo.key(Key::Unicode('['), Click)?;
                enigo.key(Key::Shift, Release)?;
                enigo.key(Key::Meta, Release)?;
            }
            BUTTON_RB => {
                // 下一个标签页 (Cmd+Shift+])
                enigo.key(Key::Meta, Press)?;
                enigo.key(Key::Shift, Press)?;
                enigo.key(Key::Unicode(']'), Click)?;
                enigo.key(Key::Shift, Release)?;
                enigo.key(Key::Meta, Release)?;
            }
            _ => (),
        }
    }

    for &button in newly_released {
        match button {
            BUTTON_A => enigo.button(EnigoButton::Left, Release)?,
            BUTTON_B => enigo.button(EnigoButton::Right, Release)?,
            _ => (),
        }
    }
    Ok(())
}

/// 计算鼠标（光标）移动增量
fn handle_mouse_movement(state: &ControllerState) -> (f64, f64) {
    let mut delta_x = 0.0;
    let mut delta_y = 0.0;

    // 左摇杆
    delta_x += apply_deadzone_and_curve(state.lx, JOYSTICK_DEADZONE) * JOYSTICK_SENSITIVITY;
    delta_y += apply_deadzone_and_curve(state.ly, JOYSTICK_DEADZONE) * JOYSTICK_SENSITIVITY;

    // 陀螺仪（仅当按住LT时）
    if state.lt > ANALOG_TRIGGER_THRESHOLD {
        if state.gyro_yaw.abs() > GYRO_DEADZONE {
            delta_x += state.gyro_yaw as f64 * GYRO_SENSITIVITY;
        }
        if state.gyro_pitch.abs() > GYRO_DEADZONE {
            delta_y += state.gyro_pitch as f64 * GYRO_SENSITIVITY;
        }
    }

    (delta_x, delta_y)
}

/// 处理右摇杆滚动和导航功能
fn handle_right_stick(
    enigo: &mut Enigo,
    state: &ControllerState,
    scroll_power: &Arc<Mutex<f64>>,
    nav_flags: &mut (bool, bool),
) -> Result<(), Box<dyn std::error::Error>> {
    let (nav_triggered_left, nav_triggered_right) = nav_flags;
    let (rx_abs, ry_abs) = (state.rx.abs(), state.ry.abs());

    // 滚动（Y轴优先）
    let mut current_scroll_power = 0.0;
    if ry_abs > RIGHT_JOYSTICK_DEADZONE && (ry_abs as f64 > rx_abs as f64 * DOMINANT_AXIS_FACTOR) {
        let normalized_ry = apply_deadzone_and_curve(state.ry, RIGHT_JOYSTICK_DEADZONE);
        // 反向以实现自然滚动方向
        current_scroll_power = -normalized_ry * DIRECT_SCROLL_SENSITIVITY;
    }
    *scroll_power.lock().unwrap() = current_scroll_power;

    // 导航（X轴优先）
    if rx_abs > NAV_TRIGGER_THRESHOLD && (rx_abs as f64 > ry_abs as f64 * DOMINANT_AXIS_FACTOR) {
        if state.rx > 0 && !*nav_triggered_right {
            // 前进：Cmd + ]
            enigo.key(Key::Meta, Press)?;
            enigo.key(Key::Unicode(']'), Click)?;
            enigo.key(Key::Meta, Release)?;
            *nav_triggered_right = true;
        } else if state.rx < 0 && !*nav_triggered_left {
            // 后退：Cmd + [
            enigo.key(Key::Meta, Press)?;
            enigo.key(Key::Unicode('['), Click)?;
            enigo.key(Key::Meta, Release)?;
            *nav_triggered_left = true;
        }
    }

    // 重置导航标志以防止连续触发
    if state.rx.abs() < NAV_TRIGGER_THRESHOLD {
        *nav_triggered_right = false;
        *nav_triggered_left = false;
    }
    Ok(())
}

/// 主控制循环
fn run_control_loop(controller: HidController, enigo: &mut Enigo, scroll_power: Arc<Mutex<f64>>) {
    let mut last_buttons = HashSet::new();
    let mut nav_flags = (false, false); // (左触发, 右触发)

    loop {
        match controller.read_state(ANALOG_TRIGGER_THRESHOLD) {
            Ok(Some(state)) => {
                // 1. 处理按钮事件
                let newly_pressed = &state.pressed_buttons - &last_buttons;
                let newly_released = &last_buttons - &state.pressed_buttons;
                if !newly_pressed.is_empty() || !newly_released.is_empty() {
                    if let Err(e) = handle_button_events(enigo, &newly_pressed, &newly_released) {
                        eprintln!("处理按钮事件时出错: {}", e);
                    }
                }
                last_buttons = state.pressed_buttons.clone();

                // 2. 处理光标移动（摇杆 + 陀螺仪）
                let (dx, dy) = handle_mouse_movement(&state);
                if dx.abs() >= 0.01 || dy.abs() >= 0.01 {
                    if let Err(e) = enigo.move_mouse(dx as i32, dy as i32, Coordinate::Rel) {
                        eprintln!("移动鼠标时出错: {}", e);
                    }
                }

                // 3. 处理右摇杆（滚动 + 导航）
                if let Err(e) = handle_right_stick(enigo, &state, &scroll_power, &mut nav_flags) {
                    eprintln!("处理右摇杆时出错: {}", e);
                }
            }
            Ok(None) => continue, // 没有新数据，继续下一次循环
            Err(e) => {
                eprintln!("{}。程序将退出。", e);
                break;
            }
        }
    }
}

fn main() {
    println!("正在搜索 {}...", HidController::get_device_info());

    // 在主线程初始化 Enigo 用于输入模拟
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(enigo) => enigo,
        Err(e) => {
            eprintln!("初始化 Enigo 时出错: {}。程序将退出。", e);
            return;
        }
    };

    println!("{}", "-".repeat(40));

    // 初始化 HID 控制器
    let controller = match HidController::new() {
        Ok(ctrl) => ctrl,
        Err(e) => {
            eprintln!("错误: {}", e);
            return;
        }
    };

    print_instructions();

    // 启动滚动步调器线程
    let scroll_power = Arc::new(Mutex::new(0.0));
    let pacer_power = Arc::clone(&scroll_power);
    thread::spawn(move || run_pacer_loop(pacer_power));

    // 运行主控制循环
    run_control_loop(controller, &mut enigo, scroll_power);
}
