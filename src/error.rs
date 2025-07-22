use std::fmt;

/// 控制器应用程序的自定义错误类型
#[derive(Debug)]
pub enum ControllerError {
    /// HID 设备相关错误
    HidDevice(String),
    /// 输入模拟错误
    InputSimulation(String),
    /// 配置相关错误
    Config(String),
    /// 设备未找到
    DeviceNotFound,
    /// 设备连接丢失
    DeviceDisconnected,
    /// 初始化失败
    InitializationFailed(String),
    /// IO 错误
    Io(std::io::Error),
    /// 序列化/反序列化错误
    Serialization(String),
}

impl fmt::Display for ControllerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControllerError::HidDevice(msg) => write!(f, "HID设备错误: {}", msg),
            ControllerError::InputSimulation(msg) => write!(f, "输入模拟错误: {}", msg),
            ControllerError::Config(msg) => write!(f, "配置错误: {}", msg),
            ControllerError::DeviceNotFound => write!(f, "未找到匹配的HID设备"),
            ControllerError::DeviceDisconnected => write!(f, "设备连接已断开"),
            ControllerError::InitializationFailed(msg) => write!(f, "初始化失败: {}", msg),
            ControllerError::Io(err) => write!(f, "IO错误: {}", err),
            ControllerError::Serialization(msg) => write!(f, "序列化错误: {}", msg),
        }
    }
}

impl std::error::Error for ControllerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ControllerError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ControllerError {
    fn from(err: std::io::Error) -> Self {
        ControllerError::Io(err)
    }
}

impl From<toml::de::Error> for ControllerError {
    fn from(err: toml::de::Error) -> Self {
        ControllerError::Serialization(format!("TOML解析错误: {}", err))
    }
}

impl From<toml::ser::Error> for ControllerError {
    fn from(err: toml::ser::Error) -> Self {
        ControllerError::Serialization(format!("TOML序列化错误: {}", err))
    }
}

impl From<hidapi::HidError> for ControllerError {
    fn from(err: hidapi::HidError) -> Self {
        ControllerError::HidDevice(format!("HidApi错误: {}", err))
    }
}

/// 控制器应用程序的结果类型别名
pub type ControllerResult<T> = Result<T, ControllerError>;

/// 错误恢复策略
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// 重试操作
    Retry { max_attempts: u32, delay_ms: u64 },
    /// 重新连接设备
    Reconnect,
    /// 跳过当前操作
    Skip,
    /// 退出程序
    Exit,
}

/// 错误处理上下文
#[derive(Debug)]
pub struct ErrorContext {
    pub error: ControllerError,
    pub recovery_strategy: RecoveryStrategy,
    pub user_message: String,
}

impl ErrorContext {
    pub fn new(error: ControllerError, recovery_strategy: RecoveryStrategy) -> Self {
        let user_message = match &error {
            ControllerError::DeviceNotFound => "请检查Xbox手柄是否正确连接到计算机。".to_string(),
            ControllerError::DeviceDisconnected => "手柄连接丢失，正在尝试重新连接...".to_string(),
            ControllerError::HidDevice(_) => "手柄通信出现问题，请检查设备连接。".to_string(),
            ControllerError::InputSimulation(_) => {
                "系统输入模拟失败，请检查系统权限设置。".to_string()
            }
            ControllerError::Config(_) => "配置文件有误，将使用默认设置。".to_string(),
            ControllerError::InitializationFailed(_) => {
                "程序初始化失败，请重启应用程序。".to_string()
            }
            _ => "发生了意外错误。".to_string(),
        };

        Self {
            error,
            recovery_strategy,
            user_message,
        }
    }

    /// 根据错误类型建议恢复策略
    pub fn suggest_recovery_strategy(error: &ControllerError) -> RecoveryStrategy {
        match error {
            ControllerError::DeviceNotFound => RecoveryStrategy::Retry {
                max_attempts: 5,
                delay_ms: 2000,
            },
            ControllerError::DeviceDisconnected => RecoveryStrategy::Reconnect,
            ControllerError::HidDevice(_) => RecoveryStrategy::Retry {
                max_attempts: 3,
                delay_ms: 1000,
            },
            ControllerError::InputSimulation(_) => RecoveryStrategy::Skip,
            ControllerError::Config(_) => RecoveryStrategy::Skip,
            ControllerError::InitializationFailed(_) => RecoveryStrategy::Exit,
            _ => RecoveryStrategy::Skip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = ControllerError::DeviceNotFound;
        assert_eq!(error.to_string(), "未找到匹配的HID设备");

        let error = ControllerError::HidDevice("测试错误".to_string());
        assert_eq!(error.to_string(), "HID设备错误: 测试错误");
    }

    #[test]
    fn test_error_context() {
        let error = ControllerError::DeviceNotFound;
        let context = ErrorContext::new(error, RecoveryStrategy::Reconnect);

        assert!(context.user_message.contains("Xbox手柄"));
        assert!(matches!(
            context.recovery_strategy,
            RecoveryStrategy::Reconnect
        ));
    }

    #[test]
    fn test_recovery_strategy_suggestion() {
        let error = ControllerError::DeviceNotFound;
        let strategy = ErrorContext::suggest_recovery_strategy(&error);

        if let RecoveryStrategy::Retry {
            max_attempts,
            delay_ms,
        } = strategy
        {
            assert_eq!(max_attempts, 5);
            assert_eq!(delay_ms, 2000);
        } else {
            panic!("Expected Retry strategy");
        }
    }

    #[test]
    fn test_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "文件未找到");
        let controller_error: ControllerError = io_error.into();

        assert!(matches!(controller_error, ControllerError::Io(_)));
    }
}
