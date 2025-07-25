use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 控制器配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfig {
    /// 模拟扳机阈值
    pub analog_trigger_threshold: u8,
    /// 左摇杆死区
    pub joystick_deadzone: i16,
    /// 右摇杆死区
    pub right_joystick_deadzone: i16,
    /// 陀螺仪死区
    pub gyro_deadzone: i16,
    /// 页面导航触发阈值
    pub nav_trigger_threshold: i16,
    /// 主导轴系数
    pub dominant_axis_factor: f64,
    /// 左摇杆灵敏度
    pub joystick_sensitivity: f64,
    /// 陀螺仪灵敏度
    pub gyro_sensitivity: f64,
    /// 直接滚动灵敏度
    pub direct_scroll_sensitivity: f64,
    /// 步调器循环频率 (Hz)
    pub pacer_loop_hz: u64,
    /// 重连配置
    pub reconnection: ReconnectionConfig,
    /// 按键绑定配置
    pub button_mappings: HashMap<String, ButtonAction>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            analog_trigger_threshold: 20,
            joystick_deadzone: 1000,
            right_joystick_deadzone: 5000,
            gyro_deadzone: 10,
            nav_trigger_threshold: 32001,
            dominant_axis_factor: 1.5,
            joystick_sensitivity: 15.0,
            gyro_sensitivity: 0.08,
            direct_scroll_sensitivity: 20.0,
            pacer_loop_hz: 75,
            reconnection: ReconnectionConfig::default(),
            button_mappings: Self::default_button_mappings(),
        }
    }
}

impl ControllerConfig {
    /// 创建默认按键绑定配置
    fn default_button_mappings() -> HashMap<String, ButtonAction> {
        let mut mappings = HashMap::new();
        
        // 单独按键
        mappings.insert("A".to_string(), ButtonAction::LeftClick);
        mappings.insert("B".to_string(), ButtonAction::RightClick);
        mappings.insert("X".to_string(), ButtonAction::CloseWindow);
        mappings.insert("Y".to_string(), ButtonAction::MissionControl);
        mappings.insert("LB".to_string(), ButtonAction::PrevTab);
        mappings.insert("RB".to_string(), ButtonAction::NextTab);
        mappings.insert("DPad_Up".to_string(), ButtonAction::Refresh);
        mappings.insert("DPad_Down".to_string(), ButtonAction::None);
        mappings.insert("DPad_Left".to_string(), ButtonAction::None);
        mappings.insert("DPad_Right".to_string(), ButtonAction::NewTab);
        
        // 组合键
        mappings.insert("LT+X".to_string(), ButtonAction::QuitApp);
        
        mappings
    }

    /// 从文件加载配置，如果文件不存在则创建默认配置文件
    pub fn load_or_create_default<P: AsRef<Path>>(config_path: P) -> Result<Self, String> {
        let path = config_path.as_ref();

        if path.exists() {
            Self::load_from_file(path)
        } else {
            let default_config = Self::default();
            default_config.save_to_file(path)?;
            println!("已创建默认配置文件: {}", path.display());
            Ok(default_config)
        }
    }

    /// 从文件加载配置
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content =
            fs::read_to_string(path.as_ref()).map_err(|e| format!("读取配置文件失败: {}", e))?;

        toml::from_str(&content).map_err(|e| format!("解析配置文件失败: {}", e))
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let content = toml::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {}", e))?;

        // 确保父目录存在
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
        }

        fs::write(path.as_ref(), content).map_err(|e| format!("写入配置文件失败: {}", e))?;

        Ok(())
    }

    /// 验证配置参数的合理性
    pub fn validate(&self) -> Result<(), String> {
        if self.joystick_sensitivity <= 0.0 {
            return Err("摇杆灵敏度必须大于0".to_string());
        }

        if self.gyro_sensitivity <= 0.0 {
            return Err("陀螺仪灵敏度必须大于0".to_string());
        }

        if self.pacer_loop_hz == 0 {
            return Err("步调器频率必须大于0".to_string());
        }

        if self.joystick_deadzone < 0 {
            return Err("摇杆死区不能为负数".to_string());
        }

        if self.right_joystick_deadzone < 0 {
            return Err("右摇杆死区不能为负数".to_string());
        }

        if self.gyro_deadzone < 0 {
            return Err("陀螺仪死区不能为负数".to_string());
        }

        if self.dominant_axis_factor <= 1.0 {
            return Err("主导轴系数必须大于1.0".to_string());
        }

        Ok(())
    }

    /// 获取默认配置文件路径
    pub fn default_config_path() -> Result<std::path::PathBuf, String> {
        let home_dir = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;

        Ok(home_dir
            .join(".config")
            .join("controller")
            .join("config.toml"))
    }

    /// 获取按键绑定
    pub fn get_button_action(&self, button_combo: &str) -> Option<&ButtonAction> {
        self.button_mappings.get(button_combo)
    }
}

/// 按钮动作枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ButtonAction {
    /// 鼠标左键
    LeftClick,
    /// 鼠标右键
    RightClick,
    /// 关闭窗口
    CloseWindow,
    /// 调度中心
    MissionControl,
    /// 上一个标签页
    PrevTab,
    /// 下一个标签页
    NextTab,
    /// 退出应用程序 (Cmd+Q)
    QuitApp,
    /// 新建标签页 (Cmd+T)
    NewTab,
    /// 刷新页面 (Cmd+R)
    Refresh,
    /// 自定义快捷键
    CustomShortcut { modifiers: Vec<String>, key: String },
    /// 无操作
    None,
}

/// 重连配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionConfig {
    /// 是否启用自动重连
    pub enable_auto_reconnect: bool,
    /// 重连尝试间隔（毫秒）
    pub reconnect_interval_ms: u64,
    /// 最大重连尝试次数（0表示无限制）
    pub max_reconnect_attempts: u32,
    /// 是否显示重连消息
    pub show_reconnect_messages: bool,
    /// 最大静默失败次数（超过此次数后开始显示重连消息）
    pub max_silent_failures: u32,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enable_auto_reconnect: true,
            reconnect_interval_ms: 2000,
            max_reconnect_attempts: 0, // 无限制
            show_reconnect_messages: true,
            max_silent_failures: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = ControllerConfig::default();
        assert_eq!(config.analog_trigger_threshold, 20);
        assert_eq!(config.joystick_sensitivity, 15.0);
        assert_eq!(config.pacer_loop_hz, 75);
    }

    #[test]
    fn test_config_validation() {
        let mut config = ControllerConfig::default();
        assert!(config.validate().is_ok());

        config.joystick_sensitivity = -1.0;
        assert!(config.validate().is_err());

        config.joystick_sensitivity = 15.0;
        config.pacer_loop_hz = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let original_config = ControllerConfig::default();
        original_config.save_to_file(&config_path).unwrap();

        let loaded_config = ControllerConfig::load_from_file(&config_path).unwrap();
        assert_eq!(
            original_config.joystick_sensitivity,
            loaded_config.joystick_sensitivity
        );
        assert_eq!(original_config.pacer_loop_hz, loaded_config.pacer_loop_hz);
    }

    #[test]
    fn test_load_or_create_default() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("new_config.toml");

        // 文件不存在时应该创建默认配置
        let config = ControllerConfig::load_or_create_default(&config_path).unwrap();
        assert!(config_path.exists());

        // 再次加载应该读取现有文件
        let config2 = ControllerConfig::load_or_create_default(&config_path).unwrap();
        assert_eq!(config.joystick_sensitivity, config2.joystick_sensitivity);
    }
}
