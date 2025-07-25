use crate::config::ControllerConfig;
use crate::error::ControllerResult;
use crate::hid::HidController;
use std::{thread, time::Duration};

/// 连接状态枚举
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// 已连接状态
    Connected,
    /// 断开连接状态
    Disconnected,
    /// 等待重连状态
    WaitingReconnect,
    /// 重连中状态
    Reconnecting,
}

/// 连接管理器
pub struct ConnectionManager {
    state: ConnectionState,
    reconnect_config: crate::config::ReconnectionConfig,
    reconnect_attempts: u32,
    silent_failures: u32,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new(config: &ControllerConfig) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            reconnect_config: config.reconnection.clone(),
            reconnect_attempts: 0,
            silent_failures: 0,
        }
    }

    /// 获取当前连接状态
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// 尝试初始连接
    pub fn initial_connect(&mut self) -> ControllerResult<HidController> {
        self.state = ConnectionState::Reconnecting;

        match HidController::new() {
            Ok(controller) => {
                self.state = ConnectionState::Connected;
                self.reset_counters();
                Ok(controller)
            }
            Err(e) => {
                self.state = ConnectionState::Disconnected;
                Err(e)
            }
        }
    }

    /// 处理设备断开事件
    pub fn handle_disconnect(&mut self) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::Disconnected;
            self.silent_failures = 0;

            if self.reconnect_config.show_reconnect_messages {
                println!("手柄已断开连接，等待重新连接...");
            }
        }
    }

    /// 尝试重新连接
    /// 返回 Some(controller) 表示重连成功
    /// 返回 None 表示重连失败或不需要重连
    pub fn try_reconnect(&mut self) -> Option<ControllerResult<HidController>> {
        // 检查是否启用自动重连
        if !self.reconnect_config.enable_auto_reconnect {
            return None;
        }

        // 检查是否达到最大重试次数
        if self.reconnect_config.max_reconnect_attempts > 0
            && self.reconnect_attempts >= self.reconnect_config.max_reconnect_attempts
        {
            if self.reconnect_config.show_reconnect_messages {
                println!("已达到最大重连尝试次数，停止重连。");
            }
            return None;
        }

        // 只有在断开状态才尝试重连
        if self.state != ConnectionState::Disconnected
            && self.state != ConnectionState::WaitingReconnect
        {
            return None;
        }

        self.state = ConnectionState::Reconnecting;
        self.reconnect_attempts += 1;

        // 决定是否显示重连消息
        let should_show_message = self.reconnect_config.show_reconnect_messages
            && (self.silent_failures >= self.reconnect_config.max_silent_failures
                || self.reconnect_attempts % 10 == 1); // 每10次尝试显示一次

        if should_show_message {
            if self.reconnect_config.max_reconnect_attempts > 0 {
                println!(
                    "正在尝试重新连接手柄... (第 {}/{} 次)",
                    self.reconnect_attempts, self.reconnect_config.max_reconnect_attempts
                );
            } else {
                println!(
                    "正在尝试重新连接手柄... (第 {} 次)",
                    self.reconnect_attempts
                );
            }
        }

        match HidController::try_reconnect() {
            Ok(controller) => {
                self.state = ConnectionState::Connected;
                self.reset_counters();

                if self.reconnect_config.show_reconnect_messages {
                    println!("手柄已重新连接！");
                }

                Some(Ok(controller))
            }
            Err(e) => {
                self.state = ConnectionState::WaitingReconnect;
                self.silent_failures += 1;

                if should_show_message {
                    println!("重连失败: {}", e);
                }

                Some(Err(e))
            }
        }
    }

    /// 等待重连间隔
    pub fn wait_reconnect_interval(&self) {
        if self.state == ConnectionState::WaitingReconnect {
            thread::sleep(Duration::from_millis(
                self.reconnect_config.reconnect_interval_ms,
            ));
        }
    }

    /// 是否应该继续运行（用于主循环判断）
    pub fn should_continue(&self) -> bool {
        match self.state {
            ConnectionState::Connected => true,
            ConnectionState::Disconnected
            | ConnectionState::WaitingReconnect
            | ConnectionState::Reconnecting => self.reconnect_config.enable_auto_reconnect,
        }
    }

    /// 重置计数器
    fn reset_counters(&mut self) {
        self.reconnect_attempts = 0;
        self.silent_failures = 0;
    }

    /// 获取重连统计信息
    pub fn get_stats(&self) -> ReconnectStats {
        ReconnectStats {
            attempts: self.reconnect_attempts,
            silent_failures: self.silent_failures,
            state: self.state.clone(),
        }
    }
}

/// 重连统计信息
#[derive(Debug, Clone)]
pub struct ReconnectStats {
    pub attempts: u32,
    pub silent_failures: u32,
    pub state: ConnectionState,
}
