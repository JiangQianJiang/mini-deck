use super::*;
use std::sync::Mutex;
#[derive(Default)]
pub struct MockKeyboardBackend {
    pub calls: Mutex<Vec<Shortcut>>,
    pub fail: bool,
}
impl KeyboardBackend for MockKeyboardBackend {
    fn is_mock(&self) -> bool {
        true
    }
    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), KeyboardError> {
        if self.fail {
            return Err(KeyboardError("模拟键盘故障".into()));
        }
        self.calls
            .lock()
            .map_err(|_| KeyboardError("模拟锁损坏".into()))?
            .push(shortcut.clone());
        Ok(())
    }
}
