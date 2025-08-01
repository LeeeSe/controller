use crate::error::{ControllerError, ControllerResult};
use hidapi::{HidApi, HidDevice};
use std::collections::HashSet;

// --- HID设备标识 ---
pub const TARGET_VENDOR_ID: u16 = 0x045E;

// 支持的Xbox手柄产品ID列表
pub const SUPPORTED_PRODUCT_IDS: &[u16] = &[
    0x028E, // Xbox 360 Controller (有线)
    0x02E0, // Xbox Wireless Controller (蓝牙)
    0x02FD, // Xbox One Controller (有线)
    0x02EA, // Xbox One Controller (蓝牙)
    0x0719, // Xbox 360 Wireless Receiver
];

// --- 按钮掩码定义 ---
pub const BUTTON_LB: u8 = 0x01;
pub const BUTTON_RB: u8 = 0x02;
pub const BUTTON_A: u8 = 0x10;
pub const BUTTON_B: u8 = 0x20;
pub const BUTTON_X: u8 = 0x40;
pub const BUTTON_Y: u8 = 0x80;

// 方向键按钮掩码定义（偏移量2）- 使用独特的值避免冲突
pub const DPAD_UP: u8 = 0x01 | 0x80; // 0x81
pub const DPAD_DOWN: u8 = 0x02 | 0x80; // 0x82
pub const DPAD_LEFT: u8 = 0x04 | 0x80; // 0x84
pub const DPAD_RIGHT: u8 = 0x08 | 0x80; // 0x88

// --- HID报告偏移量定义 ---
const BUTTONS_BYTE_2_OFFSET: usize = 2; // 方向键所在字节
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
    /// 获取规范化的右摇杆X值，解决 -32768/32767 不对称问题
    pub fn normalized_rx(&self) -> i16 {
        // 使用 saturating_abs() 和 saturating_neg() 避免溢出
        self.rx.saturating_abs().min(i16::MAX) * self.rx.signum()
    }

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
        let button_byte_2 = buf[BUTTONS_BYTE_2_OFFSET]; // 方向键
        let button_byte_3 = buf[BUTTONS_BYTE_3_OFFSET]; // 面部按钮和肩部按钮
        let mut pressed_buttons = HashSet::new();

        // 解析方向键 (使用原始掩码值检测)
        if (button_byte_2 & 0x01) != 0 {
            pressed_buttons.insert(DPAD_UP);
        }
        if (button_byte_2 & 0x02) != 0 {
            pressed_buttons.insert(DPAD_DOWN);
        }
        if (button_byte_2 & 0x04) != 0 {
            pressed_buttons.insert(DPAD_LEFT);
        }
        if (button_byte_2 & 0x08) != 0 {
            pressed_buttons.insert(DPAD_RIGHT);
        }

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
                (raw_gyro_yaw as i16).saturating_sub(4096)
            } else {
                raw_gyro_yaw as i16
            },
            gyro_pitch: if raw_gyro_pitch >= 2048 {
                (raw_gyro_pitch as i16).saturating_sub(4096)
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
    pub fn new() -> ControllerResult<Self> {
        let api = HidApi::new()
            .map_err(|e| ControllerError::HidDevice(format!("HidApi 初始化失败: {}", e)))?;

        let device = Self::find_and_open_device(&api).ok_or(ControllerError::DeviceNotFound)?;

        Ok(Self { device })
    }

    /// 查找并打开目标 HID 设备
    fn find_and_open_device(api: &HidApi) -> Option<HidDevice> {
        // 搜索所有支持的产品ID
        for &product_id in SUPPORTED_PRODUCT_IDS {
            if let Some(dev_info) = api
                .device_list()
                .find(|d| d.vendor_id() == TARGET_VENDOR_ID && d.product_id() == product_id)
            {
                let device_name = dev_info.product_string().unwrap_or("未知设备");
                println!("找到设备: {} (PID: {:#06X})", device_name, product_id);

                match dev_info.open_device(api) {
                    Ok(device) => return Some(device),
                    Err(e) => {
                        eprintln!("无法打开设备 {}: {}", device_name, e);
                        continue;
                    }
                }
            }
        }
        None
    }

    /// 读取HID设备数据并解析为控制器状态
    pub fn read_state(
        &self,
        analog_trigger_threshold: u8,
    ) -> ControllerResult<Option<ControllerState>> {
        let mut buf = [0u8; 64];

        match self.device.read_timeout(&mut buf, 10) {
            Ok(0) => Ok(None), // 没有数据
            Ok(_) => {
                let state = ControllerState::from_buffer(&buf, analog_trigger_threshold);
                Ok(Some(state))
            }
            Err(e) => Err(ControllerError::HidDevice(format!("读取设备时出错: {}", e))),
        }
    }

    /// 尝试重新连接设备（用于重连逻辑）
    pub fn try_reconnect() -> ControllerResult<Self> {
        let api = HidApi::new()
            .map_err(|e| ControllerError::HidDevice(format!("HidApi 初始化失败: {}", e)))?;

        let device = Self::find_and_open_device(&api).ok_or(ControllerError::DeviceNotFound)?;

        Ok(Self { device })
    }

    /// 获取设备信息字符串
    pub fn get_device_info() -> String {
        let pids: Vec<String> = SUPPORTED_PRODUCT_IDS
            .iter()
            .map(|&pid| format!("{:#06X}", pid))
            .collect();

        format!(
            "Xbox手柄设备 (VID: {:#06X}, 支持的PID: {})",
            TARGET_VENDOR_ID,
            pids.join(", ")
        )
    }
}
