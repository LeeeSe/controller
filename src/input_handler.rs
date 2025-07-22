use crate::config::{ButtonAction, ButtonMappingConfig, ControllerConfig};
use crate::error::{ControllerError, ControllerResult};
use crate::hid::{BUTTON_A, BUTTON_B, BUTTON_LB, BUTTON_RB, BUTTON_X, BUTTON_Y, ControllerState};
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
    button_mapping: ButtonMappingConfig,
    last_buttons: HashSet<u8>,
    nav_flags: (bool, bool), // (左触发, 右触发)
}

impl InputHandler {
    /// 创建新的输入处理器
    pub fn new(
        config: ControllerConfig,
        button_mapping: ButtonMappingConfig,
    ) -> ControllerResult<Self> {
        let enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| {
            ControllerError::InitializationFailed(format!("Enigo初始化失败: {}", e))
        })?;

        Ok(Self {
            enigo,
            config,
            button_mapping,
            last_buttons: HashSet::new(),
            nav_flags: (false, false),
        })
    }

    /// 处理控制器状态更新
    pub fn handle_input(
        &mut self,
        state: &ControllerState,
        scroll_power: &Arc<Mutex<f64>>,
    ) -> ControllerResult<()> {
        // 1. 处理按钮事件
        self.handle_button_events(state)?;

        // 2. 处理光标移动（摇杆 + 陀螺仪）
        self.handle_mouse_movement(state)?;

        // 3. 处理右摇杆（滚动 + 导航）
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
        let action = match button {
            BUTTON_A => &self.button_mapping.button_a,
            BUTTON_B => &self.button_mapping.button_b,
            BUTTON_X => &self.button_mapping.button_x,
            BUTTON_Y => &self.button_mapping.button_y,
            BUTTON_LB => &self.button_mapping.button_lb,
            BUTTON_RB => &self.button_mapping.button_rb,
            _ => return Ok(()),
        };

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

        // 左摇杆
        delta_x += self.apply_deadzone_and_curve(state.lx, self.config.joystick_deadzone)
            * self.config.joystick_sensitivity;
        delta_y += self.apply_deadzone_and_curve(state.ly, self.config.joystick_deadzone)
            * self.config.joystick_sensitivity;

        // 陀螺仪（仅当按住LT时）
        if state.lt > self.config.analog_trigger_threshold {
            if state.gyro_yaw.abs() > self.config.gyro_deadzone {
                delta_x += state.gyro_yaw as f64 * self.config.gyro_sensitivity;
            }
            if state.gyro_pitch.abs() > self.config.gyro_deadzone {
                delta_y += state.gyro_pitch as f64 * self.config.gyro_sensitivity;
            }
        }

        // 只有当移动量足够大时才移动鼠标
        if delta_x.abs() >= 0.01 || delta_y.abs() >= 0.01 {
            self.enigo
                .move_mouse(delta_x as i32, delta_y as i32, Coordinate::Rel)
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
        let (rx_abs, ry_abs) = (state.rx.abs(), state.ry.abs());

        // 滚动（Y轴优先）
        let mut current_scroll_power = 0.0;
        if ry_abs > self.config.right_joystick_deadzone
            && (ry_abs as f64 > rx_abs as f64 * self.config.dominant_axis_factor)
        {
            let normalized_ry =
                self.apply_deadzone_and_curve(state.ry, self.config.right_joystick_deadzone);
            // 反向以实现自然滚动方向
            current_scroll_power = -normalized_ry * self.config.direct_scroll_sensitivity;
        }

        // 更新滚动力度
        if let Ok(mut power) = scroll_power.lock() {
            *power = current_scroll_power;
        }

        // 导航（X轴优先）
        if rx_abs > self.config.nav_trigger_threshold
            && (rx_abs as f64 > ry_abs as f64 * self.config.dominant_axis_factor)
        {
            if state.rx > 0 && !self.nav_flags.1 {
                // 前进：Cmd + ]
                self.execute_shortcut(&[Key::Meta], Key::Unicode(']'))?;
                self.nav_flags.1 = true;
            } else if state.rx < 0 && !self.nav_flags.0 {
                // 后退：Cmd + [
                self.execute_shortcut(&[Key::Meta], Key::Unicode('['))?;
                self.nav_flags.0 = true;
            }
        }

        // 重置导航标志以防止连续触发
        if state.rx.abs() < self.config.nav_trigger_threshold {
            self.nav_flags.1 = false;
            self.nav_flags.0 = false;
        }

        Ok(())
    }

    /// 应用死区和二次加速曲线
    fn apply_deadzone_and_curve(&self, value: i16, deadzone: i16) -> f64 {
        let val_f = value as f64;
        let deadzone_f = deadzone as f64;

        if val_f.abs() < deadzone_f {
            return 0.0;
        }

        let max_val = i16::MAX as f64;
        let normalized_val = (val_f.abs() - deadzone_f) / (max_val - deadzone_f);
        normalized_val.powi(2).copysign(val_f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ControllerConfig;

    fn create_test_input_handler() -> InputHandler {
        let config = ControllerConfig::default();
        let button_mapping = ButtonMappingConfig::default();

        // 对于测试，我们需要模拟 Enigo 的创建
        // 这里可能需要使用 mock 或者跳过需要系统权限的测试
        InputHandler::new(config, button_mapping).unwrap()
    }

    #[test]
    fn test_apply_deadzone_and_curve() {
        let handler = create_test_input_handler();

        // 在死区内应该返回0
        assert_eq!(handler.apply_deadzone_and_curve(500, 1000), 0.0);

        // 超出死区应该有输出
        let result = handler.apply_deadzone_and_curve(2000, 1000);
        assert!(result > 0.0);

        // 负值应该保持符号
        let result = handler.apply_deadzone_and_curve(-2000, 1000);
        assert!(result < 0.0);
    }

    #[test]
    fn test_parse_key_string() {
        assert!(matches!(
            InputHandler::parse_key_string_static("cmd"),
            Ok(Key::Meta)
        ));
        assert!(matches!(
            InputHandler::parse_key_string_static("shift"),
            Ok(Key::Shift)
        ));
        assert!(matches!(
            InputHandler::parse_key_string_static("a"),
            Ok(Key::Unicode('a'))
        ));
        assert!(InputHandler::parse_key_string_static("invalid_key").is_err());
    }
}
