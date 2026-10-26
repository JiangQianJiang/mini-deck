use super::*;
use crate::shortcut::KeyEvent;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
pub struct WindowsKeyboardBackend;
fn input(event: &KeyEvent) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: event.key,
                wScan: 0,
                dwFlags: (if event.up { KEYEVENTF_KEYUP } else { 0 })
                    | (if event.extended {
                        KEYEVENTF_EXTENDEDKEY
                    } else {
                        0
                    }),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
impl KeyboardBackend for WindowsKeyboardBackend {
    fn send_shortcut(&self, shortcut: &Shortcut) -> Result<(), KeyboardError> {
        submit_sequence(shortcut, |events| {
            let inputs: Vec<_> = events.iter().map(input).collect();
            // FFI 边界：缓冲区在同步调用期间有效，结构大小与 Win32 ABI 一致。
            unsafe {
                SendInput(
                    inputs.len() as u32,
                    inputs.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                ) as usize
            }
        })
    }
}
