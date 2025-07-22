use hidapi::{HidApi, HidDevice};
use std::collections::HashSet;

// --- HID设备标识 ---
pub const TARGET_VENDOR_ID: u16 = 0x045E;
pub const TARGET_PRODUCT_ID: u16 = 0x028E;

// --- 按钮掩码定义 ---
pub const BUTTON_LB: u8 = 0x01;
pub const BUTTON_RB: u8 = 0x02;
pub const BUTTON_A: u8 = 0x10;
pub const BUTTON_B: u8 = 0x20;
pub const BUTTON_X: u8 = 0x40;
pub const BUTTON_Y: u8 = 0x80;

// --- HID报告偏移量定义 ---
const BUTTONS_BYTE_3_OFFSET: usize = 3;
const LT_OFFSET: usize = 4;
const LX_OFFSET: usize = 6;
const LY_OFFSET: usize = 8;
const RX_OFFSET: usize = 10;
const RY_OFFSET: usize = 12;
const GYRO_YAW_LOW_OFFSET: usize = 14;
const GYRO_PITCH_LOW_OFFSET: usize = 15;
const GYRO_HIGH_NIBBLES_OFFSET: usize = 16;

/// 封装了手柄所有输入状态的结构体
#[derive(Clone, Debug)]
pub struct ControllerState {
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
    pub lt: u8,
    pub gyro_yaw: i16,
    pub gyro_pitch: i16,
    pub pressed_buttons: HashSet<u8>,
}

impl ControllerState {
    /// 从 HID 缓冲区解析手柄状态
    pub fn from_buffer(buf: &[u8], analog_trigger_threshold: u8) -> Self {
        let lt = buf[LT_OFFSET];

        // 解析陀螺仪数据（仅当LT按下时）
        let (raw_gyro_yaw, raw_gyro_pitch) = if lt > analog_trigger_threshold {
            let high_nibbles = buf[GYRO_HIGH_NIBBLES_OFFSET];
            let yaw_high = (high_nibbles & 0xF0) >> 4;
            let pitch_high = high_nibbles & 0x0F;
            let raw_yaw = (yaw_high as u16) << 8 | buf[GYRO_YAW_LOW_OFFSET] as u16;
            let raw_pitch = (pitch_high as u16) << 8 | buf[GYRO_PITCH_LOW_OFFSET] as u16;
            (raw_yaw, raw_pitch)
        } else {
            (0, 0)
        };

        // 解析按钮状态
        let button_byte_3 = buf[BUTTONS_BYTE_3_OFFSET];
        let mut pressed_buttons = HashSet::new();

        if (button_byte_3 & BUTTON_A) != 0 {
            pressed_buttons.insert(BUTTON_A);
        }
        if (button_byte_3 & BUTTON_B) != 0 {
            pressed_buttons.insert(BUTTON_B);
        }
        if (button_byte_3 & BUTTON_X) != 0 {
            pressed_buttons.insert(BUTTON_X);
        }
        if (button_byte_3 & BUTTON_Y) != 0 {
            pressed_buttons.insert(BUTTON_Y);
        }
        if (button_byte_3 & BUTTON_LB) != 0 {
            pressed_buttons.insert(BUTTON_LB);
        }
        if (button_byte_3 & BUTTON_RB) != 0 {
            pressed_buttons.insert(BUTTON_RB);
        }

        Self {
            lx: i16::from_le_bytes([buf[LX_OFFSET], buf[LX_OFFSET + 1]]),
            ly: i16::from_le_bytes([buf[LY_OFFSET], buf[LY_OFFSET + 1]]).saturating_neg(),
            rx: i16::from_le_bytes([buf[RX_OFFSET], buf[RX_OFFSET + 1]]),
            ry: i16::from_le_bytes([buf[RY_OFFSET], buf[RY_OFFSET + 1]]).saturating_neg(),
            lt,
            gyro_yaw: if raw_gyro_yaw >= 2048 {
                raw_gyro_yaw as i16 - 4096
            } else {
                raw_gyro_yaw as i16
            },
            gyro_pitch: if raw_gyro_pitch >= 2048 {
                raw_gyro_pitch as i16 - 4096
            } else {
                raw_gyro_pitch as i16
            },
            pressed_buttons,
        }
    }
}

/// HID设备管理器，负责设备的查找、连接和数据读取
pub struct HidController {
    device: HidDevice,
}

impl HidController {
    /// 查找并连接到目标HID设备
    pub fn new() -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| format!("HidApi 初始化失败: {}", e))?;

        let device = Self::find_and_open_device(&api)
            .ok_or_else(|| "未找到匹配的HID设备。请检查VID/PID或设备连接。".to_string())?;

        Ok(Self { device })
    }

    /// 查找并打开目标 HID 设备
    fn find_and_open_device(api: &HidApi) -> Option<HidDevice> {
        let dev_info = api
            .device_list()
            .find(|d| d.vendor_id() == TARGET_VENDOR_ID && d.product_id() == TARGET_PRODUCT_ID)?;

        println!(
            "找到设备: {}",
            dev_info.product_string().unwrap_or("未知设备")
        );

        match dev_info.open_device(api) {
            Ok(device) => Some(device),
            Err(e) => {
                eprintln!("无法打开设备: {}", e);
                None
            }
        }
    }

    /// 读取HID设备数据并解析为控制器状态
    pub fn read_state(
        &self,
        analog_trigger_threshold: u8,
    ) -> Result<Option<ControllerState>, String> {
        let mut buf = [0u8; 64];

        match self.device.read_timeout(&mut buf, 10) {
            Ok(0) => Ok(None), // 没有数据
            Ok(_) => {
                let state = ControllerState::from_buffer(&buf, analog_trigger_threshold);
                Ok(Some(state))
            }
            Err(e) => Err(format!("读取设备时出错: {}", e)),
        }
    }

    /// 获取设备信息字符串
    pub fn get_device_info() -> String {
        format!(
            "手柄设备 (VID: {:#06X}, PID: {:#06X})",
            TARGET_VENDOR_ID, TARGET_PRODUCT_ID
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_state_parsing() {
        let mut buf = [0u8; 64];

        // 模拟一些按钮按下的状态
        buf[BUTTONS_BYTE_3_OFFSET] = BUTTON_A | BUTTON_B;
        buf[LT_OFFSET] = 30; // 高于阈值

        let state = ControllerState::from_buffer(&buf, 20);

        assert!(state.pressed_buttons.contains(&BUTTON_A));
        assert!(state.pressed_buttons.contains(&BUTTON_B));
        assert!(!state.pressed_buttons.contains(&BUTTON_X));
        assert_eq!(state.lt, 30);
    }

    #[test]
    fn test_button_masks() {
        // 确保按钮掩码值正确
        assert_eq!(BUTTON_LB, 0x01);
        assert_eq!(BUTTON_RB, 0x02);
        assert_eq!(BUTTON_A, 0x10);
        assert_eq!(BUTTON_B, 0x20);
        assert_eq!(BUTTON_X, 0x40);
        assert_eq!(BUTTON_Y, 0x80);
    }
}
