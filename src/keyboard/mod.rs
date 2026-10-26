use crate::shortcut::Shortcut;
use std::fmt;
pub mod mock;
#[cfg(windows)]
pub mod windows;
#[derive(Debug, Clone)]
pub struct KeyboardError(pub String);
impl fmt::Display for KeyboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for KeyboardError {}
pub trait KeyboardBackend: Send + Sync {
    fn is_mock(&self) -> bool {
        false
    }
    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), KeyboardError>;
}

// 将失败恢复逻辑留在平台无关层，便于无需注入真实按键即可测试。
pub fn submit_sequence(
    shortcut: &Shortcut,
    mut submit: impl FnMut(&[crate::shortcut::KeyEvent]) -> usize,
) -> Result<(), KeyboardError> {
    let events = shortcut.events();
    let sent = submit(&events);
    if sent == events.len() {
        return Ok(());
    }
    let releases = shortcut.releases();
    let cleaned = submit(&releases);
    Err(KeyboardError(format!(
        "注入 {sent}/{} 个事件；释放 {cleaned}/{}（可能被 UIPI 阻止）",
        events.len(),
        releases.len()
    )))
}
