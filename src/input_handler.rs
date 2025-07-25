use crate::config::{ButtonAction, ControllerConfig};
use crate::error::{ControllerError, ControllerResult};
use crate::hid::{
    BUTTON_A, BUTTON_B, BUTTON_LB, BUTTON_RB, BUTTON_X, BUTTON_Y, ControllerState, DPAD_DOWN,
    DPAD_LEFT, DPAD_RIGHT, DPAD_UP,
};
use enigo::{
    Button as EnigoButton, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// 输入处理器，负责将手柄输入转换为系统操作
pub struct InputHandler {
    enigo: Enigo,
    config: ControllerConfig,
    last_buttons: HashSet<u8>,
    nav_flags: (bool, bool), // (左触发, 右触发)
    screen_width: i32,
    screen_height: i32,
    lt_pressed: bool, // 跟踪LT是否按下，用于组合键检测
}

impl InputHandler {
    /// 创建新的输入处理器
    pub fn new(config: ControllerConfig) -> ControllerResult<Self> {
        let enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| {
            ControllerError::InitializationFailed(format!("Enigo初始化失败: {}", e))
        })?;

        // 获取屏幕尺寸（只需要获取一次）
        let (screen_width, screen_height) = enigo.main_display().map_err(|e| {
            ControllerError::InitializationFailed(format!("获取屏幕尺寸失败: {}", e))
        })?;

        Ok(Self {
            enigo,
            config,
            last_buttons: HashSet::new(),
            nav_flags: (false, false),
            screen_width: screen_width as i32,
            screen_height: screen_height as i32,
            lt_pressed: false,
        })
    }

    /// 处理控制器状态更新
    pub fn handle_input(
        &mut self,
        state: &ControllerState,
        scroll_power: &Arc<Mutex<f64>>,
    ) -> ControllerResult<()> {
        // 1. 更新扳机状态用于组合键检测
        self.lt_pressed = state.lt > self.config.analog_trigger_threshold;

        // 2. 处理按钮事件
        self.handle_button_events(state)?;

        // 3. 处理光标移动（摇杆 + 陀螺仪）
        self.handle_mouse_movement(state)?;

        // 4. 处理右摇杆（滚动 + 导航）
        self.handle_right_stick(state, scroll_power)?;

        Ok(())
    }

    /// 处理按钮按下和释放事件
    fn handle_button_events(&mut self, state: &ControllerState) -> ControllerResult<()> {
        let newly_pressed = &state.pressed_buttons - &self.last_buttons;
        let newly_released = &self.last_buttons - &state.pressed_buttons;

        // 处理按下事件
        for &button in &newly_pressed {
            self.execute_button_action(button, true)?;
        }

        // 处理释放事件
        for &button in &newly_released {
            self.execute_button_action(button, false)?;
        }

        self.last_buttons = state.pressed_buttons.clone();
        Ok(())
    }

    /// 执行按钮动作
    fn execute_button_action(&mut self, button: u8, pressed: bool) -> ControllerResult<()> {
        // 获取按钮名称
        let button_name = self.get_button_name(button);

        // 检查是否有组合键
        let mut tried_combos = Vec::new();

        // 检查双键组合 (LT + 按键)
        if self.lt_pressed {
            let combo = format!("LT+{}", button_name);
            tried_combos.push(combo.clone());
            if let Some(action) = self.config.get_button_action(&combo).cloned() {
                if pressed {
                    self.execute_action(&action, pressed)?;
                }
                return Ok(());
            }
        }

        // 检查单独按键
        if let Some(action) = self.config.get_button_action(&button_name).cloned() {
            self.execute_action(&action, pressed)?;
        }

        Ok(())
    }

    /// 获取按钮名称
    fn get_button_name(&self, button: u8) -> String {
        match button {
            BUTTON_A => "A".to_string(),
            BUTTON_B => "B".to_string(),
            BUTTON_X => "X".to_string(),
            BUTTON_Y => "Y".to_string(),
            BUTTON_LB => "LB".to_string(),
            BUTTON_RB => "RB".to_string(),
            DPAD_UP => "DPad_Up".to_string(),
            DPAD_DOWN => "DPad_Down".to_string(),
            DPAD_LEFT => "DPad_Left".to_string(),
            DPAD_RIGHT => "DPad_Right".to_string(),
            _ => format!("Unknown_{}", button),
        }
    }

    /// 执行具体的按键动作
    fn execute_action(&mut self, action: &ButtonAction, pressed: bool) -> ControllerResult<()> {
        match action {
            ButtonAction::LeftClick => {
                let direction = if pressed { Press } else { Release };
                self.enigo
                    .button(EnigoButton::Left, direction)
                    .map_err(|e| {
                        ControllerError::InputSimulation(format!("左键点击失败: {}", e))
                    })?;
            }
            ButtonAction::RightClick => {
                let direction = if pressed { Press } else { Release };
                self.enigo
                    .button(EnigoButton::Right, direction)
                    .map_err(|e| {
                        ControllerError::InputSimulation(format!("右键点击失败: {}", e))
                    })?;
            }
            ButtonAction::CloseWindow => {
                if pressed {
                    self.execute_shortcut(&[Key::Meta], Key::Unicode('w'))?;
                }
            }
            ButtonAction::MissionControl => {
                if pressed {
                    self.enigo.key(Key::MissionControl, Click).map_err(|e| {
                        ControllerError::InputSimulation(format!("调度中心失败: {}", e))
                    })?;
                }
            }
            ButtonAction::PrevTab => {
                if pressed {
                    self.execute_shortcut(&[Key::Meta, Key::Shift], Key::Unicode('['))?;
                }
            }
            ButtonAction::NextTab => {
                if pressed {
                    self.execute_shortcut(&[Key::Meta, Key::Shift], Key::Unicode(']'))?;
                }
            }
            ButtonAction::QuitApp => {
                if pressed {
                    self.execute_shortcut(&[Key::Meta], Key::Unicode('q'))?;
                }
            }
            ButtonAction::NewTab => {
                if pressed {
                    self.execute_shortcut(&[Key::Meta], Key::Unicode('t'))?;
                }
            }
            ButtonAction::Refresh => {
                if pressed {
                    self.execute_shortcut(&[Key::Meta], Key::Unicode('r'))?;
                }
            }
            ButtonAction::CustomShortcut { modifiers, key } => {
                if pressed {
                    let modifiers_clone = modifiers.clone();
                    let key_clone = key.clone();
                    self.execute_custom_shortcut(&modifiers_clone, &key_clone)?;
                }
            }
            ButtonAction::None => {}
        }

        Ok(())
    }

    /// 执行系统快捷键
    fn execute_shortcut(&mut self, modifiers: &[Key], key: Key) -> ControllerResult<()> {
        // 按下修饰键
        for modifier in modifiers {
            self.enigo
                .key(*modifier, Press)
                .map_err(|e| ControllerError::InputSimulation(format!("修饰键按下失败: {}", e)))?;
        }

        // 点击主键
        self.enigo
            .key(key, Click)
            .map_err(|e| ControllerError::InputSimulation(format!("主键点击失败: {}", e)))?;

        // 释放修饰键（逆序）
        for modifier in modifiers.iter().rev() {
            self.enigo
                .key(*modifier, Release)
                .map_err(|e| ControllerError::InputSimulation(format!("修饰键释放失败: {}", e)))?;
        }

        Ok(())
    }

    /// 执行自定义快捷键
    fn execute_custom_shortcut(&mut self, modifiers: &[String], key: &str) -> ControllerResult<()> {
        let modifier_keys: Result<Vec<Key>, _> = modifiers
            .iter()
            .map(|m| InputHandler::parse_key_string_static(m))
            .collect();

        let modifier_keys = modifier_keys?;
        let main_key = InputHandler::parse_key_string_static(key)?;

        self.execute_shortcut(&modifier_keys, main_key)
    }

    /// 解析键名字符串为 Key 枚举
    fn parse_key_string_static(key_str: &str) -> ControllerResult<Key> {
        match key_str.to_lowercase().as_str() {
            "cmd" | "meta" => Ok(Key::Meta),
            "ctrl" | "control" => Ok(Key::Control),
            "shift" => Ok(Key::Shift),
            "alt" | "option" => Ok(Key::Alt),
            "space" => Ok(Key::Space),
            "return" | "enter" => Ok(Key::Return),
            "tab" => Ok(Key::Tab),
            "escape" | "esc" => Ok(Key::Escape),
            "delete" | "del" => Ok(Key::Delete),
            "backspace" => Ok(Key::Backspace),
            "up" => Ok(Key::UpArrow),
            "down" => Ok(Key::DownArrow),
            "left" => Ok(Key::LeftArrow),
            "right" => Ok(Key::RightArrow),
            "plus" | "=" => Ok(Key::Unicode('=')),
            "minus" | "-" => Ok(Key::Unicode('-')),
            s if s.len() == 1 => Ok(Key::Unicode(s.chars().next().unwrap())),
            _ => Err(ControllerError::Config(format!(
                "无法识别的键名: {}",
                key_str
            ))),
        }
    }

    /// 计算鼠标（光标）移动增量
    fn handle_mouse_movement(&mut self, state: &ControllerState) -> ControllerResult<()> {
        let mut delta_x = 0.0;
        let mut delta_y = 0.0;

        // 左摇杆 - 使用统一的规范化函数
        delta_x += Self::normalize_joystick_value(state.lx, self.config.joystick_deadzone, 2.0)
            * self.config.joystick_sensitivity;
        delta_y += Self::normalize_joystick_value(state.ly, self.config.joystick_deadzone, 2.0)
            * self.config.joystick_sensitivity;

        // 陀螺仪（仅当按住LT时）
        if state.lt > self.config.analog_trigger_threshold {
            if state.gyro_yaw.saturating_abs() > self.config.gyro_deadzone {
                delta_x += state.gyro_yaw as f64 * self.config.gyro_sensitivity;
            }
            if state.gyro_pitch.saturating_abs() > self.config.gyro_deadzone {
                delta_y += state.gyro_pitch as f64 * self.config.gyro_sensitivity;
            }
        }

        // 只有当移动量足够大时才移动鼠标
        if delta_x.abs() >= 0.01 || delta_y.abs() >= 0.01 {
            // 获取当前光标位置
            let current_pos = self.enigo.location().map_err(|e| {
                ControllerError::InputSimulation(format!("获取光标位置失败: {}", e))
            })?;

            // 计算新位置
            let new_x = (current_pos.0 as f64 + delta_x).round() as i32;
            let new_y = (current_pos.1 as f64 + delta_y).round() as i32;

            // 限制光标在屏幕边界内（使用预先获取的屏幕尺寸）
            let clamped_x = new_x.max(0).min(self.screen_width - 1);
            let clamped_y = new_y.max(0).min(self.screen_height - 1);

            // 使用绝对坐标移动光标
            self.enigo
                .move_mouse(clamped_x, clamped_y, Coordinate::Abs)
                .map_err(|e| ControllerError::InputSimulation(format!("鼠标移动失败: {}", e)))?;
        }

        Ok(())
    }

    /// 处理右摇杆滚动和导航功能
    fn handle_right_stick(
        &mut self,
        state: &ControllerState,
        scroll_power: &Arc<Mutex<f64>>,
    ) -> ControllerResult<()> {
        let (rx_abs, ry_abs) = (state.rx.saturating_abs(), state.ry.saturating_abs());

        // 检查是否有LT + 右摇杆方向的组合键绑定
        if self.lt_pressed {
            // 检查垂直方向 (优先)
            if ry_abs > self.config.right_joystick_deadzone
                && (ry_abs as f64 > rx_abs as f64 * self.config.dominant_axis_factor)
            {
                let stick_direction = if state.ry > 0 {
                    "RStick_Down"
                } else {
                    "RStick_Up"
                };
                let combo = format!("LT+{}", stick_direction);

                if let Some(action) = self.config.get_button_action(&combo).cloned() {
                    // 执行自定义绑定，使用方向标志避免重复触发
                    if state.ry > 0 && !self.nav_flags.1 {
                        self.execute_action(&action, true)?;
                        self.nav_flags.1 = true;
                    } else if state.ry < 0 && !self.nav_flags.0 {
                        self.execute_action(&action, true)?;
                        self.nav_flags.0 = true;
                    }
                } else {
                    // 没有自定义绑定，使用默认滚动行为
                    let normalized_ry = Self::normalize_joystick_value(
                        state.ry,
                        self.config.right_joystick_deadzone,
                        2.0,
                    );
                    let current_scroll_power =
                        -normalized_ry * self.config.direct_scroll_sensitivity;
                    if let Ok(mut power) = scroll_power.lock() {
                        *power = current_scroll_power;
                    }
                }
            }
            // 检查水平方向
            else if rx_abs > self.config.nav_trigger_threshold
                && (rx_abs as f64 > ry_abs as f64 * self.config.dominant_axis_factor)
            {
                let normalized_rx = state.normalized_rx();
                let stick_direction = if normalized_rx > 0 {
                    "RStick_Right"
                } else {
                    "RStick_Left"
                };
                let combo = format!("LT+{}", stick_direction);

                if let Some(action) = self.config.get_button_action(&combo).cloned() {
                    // 执行自定义绑定
                    if normalized_rx > 0 && !self.nav_flags.1 {
                        self.execute_action(&action, true)?;
                        self.nav_flags.1 = true;
                    } else if normalized_rx < 0 && !self.nav_flags.0 {
                        self.execute_action(&action, true)?;
                        self.nav_flags.0 = true;
                    }
                } else {
                    // 没有自定义绑定，使用默认导航行为
                    if normalized_rx > 0 && !self.nav_flags.1 {
                        // 前进：Cmd + ]
                        self.execute_shortcut(&[Key::Meta], Key::Unicode(']'))?;
                        self.nav_flags.1 = true;
                    } else if normalized_rx < 0 && !self.nav_flags.0 {
                        // 后退：Cmd + [
                        self.execute_shortcut(&[Key::Meta], Key::Unicode('['))?;
                        self.nav_flags.0 = true;
                    }
                }
            }
        } else {
            // LT未按下，使用原有的滚动和导航逻辑

            // 滚动（Y轴优先）
            let mut current_scroll_power = 0.0;
            if ry_abs > self.config.right_joystick_deadzone
                && (ry_abs as f64 > rx_abs as f64 * self.config.dominant_axis_factor)
            {
                let normalized_ry = Self::normalize_joystick_value(
                    state.ry,
                    self.config.right_joystick_deadzone,
                    2.0,
                );
                // 反向以实现自然滚动方向
                current_scroll_power = -normalized_ry * self.config.direct_scroll_sensitivity;
            }

            // 更新滚动力度
            if let Ok(mut power) = scroll_power.lock() {
                *power = current_scroll_power;
            }

            // 导航（X轴优先）- 使用规范化的rx值避免不对称性问题
            let normalized_rx = state.normalized_rx();
            if rx_abs > self.config.nav_trigger_threshold
                && (rx_abs as f64 > ry_abs as f64 * self.config.dominant_axis_factor)
            {
                if normalized_rx > 0 && !self.nav_flags.1 {
                    // 前进：Cmd + ]
                    self.execute_shortcut(&[Key::Meta], Key::Unicode(']'))?;
                    self.nav_flags.1 = true;
                } else if normalized_rx < 0 && !self.nav_flags.0 {
                    // 后退：Cmd + [
                    self.execute_shortcut(&[Key::Meta], Key::Unicode('['))?;
                    self.nav_flags.0 = true;
                }
            }
        }

        // 重置导航标志以防止连续触发
        if rx_abs < self.config.nav_trigger_threshold
            && ry_abs < self.config.right_joystick_deadzone
        {
            self.nav_flags.1 = false;
            self.nav_flags.0 = false;
        }

        Ok(())
    }

    /// 规范化摇杆值的统一处理函数
    ///
    /// 优雅地处理 i16 边界值，避免溢出问题
    /// 使用 saturating_abs() 自动处理 i16::MIN 溢出
    fn normalize_joystick_value(value: i16, deadzone: i16, curve_power: f64) -> f64 {
        let abs_value = value.saturating_abs();
        let abs_deadzone = deadzone.saturating_abs();

        // 死区内返回0
        if abs_value <= abs_deadzone {
            return 0.0;
        }

        // 计算去除死区后的规范化值 [0.0, 1.0]
        let max_range = i16::MAX - abs_deadzone;
        let active_range = abs_value - abs_deadzone;
        let normalized = active_range as f64 / max_range as f64;

        // 应用指数曲线并恢复符号
        let curved = normalized.powf(curve_power);
        if value < 0 { -curved } else { curved }
    }
}
