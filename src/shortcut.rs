use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub keys: Vec<u16>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyEvent {
    pub key: u16,
    pub up: bool,
    pub extended: bool,
}
impl Shortcut {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 80 {
            return Err("快捷键过长".into());
        }
        let mut modifiers = Vec::new();
        let mut primary = None;
        for part in text.split('+') {
            let part = part.trim().to_ascii_uppercase();
            let modifier = match part.as_str() {
                "CTRL" => Some(0x11),
                "ALT" => Some(0x12),
                "SHIFT" => Some(0x10),
                "WIN" => Some(0x5b),
                _ => None,
            };
            if let Some(key) = modifier {
                if modifiers.contains(&key) {
                    return Err("修饰键重复".into());
                }
                modifiers.push(key);
            } else {
                let key = match part.as_str() {
                    "ENTER" => 0x0d,
                    "ESC" => 0x1b,
                    "TAB" => 9,
                    "SPACE" => 0x20,
                    "LEFT" => 0x25,
                    "UP" => 0x26,
                    "RIGHT" => 0x27,
                    "DOWN" => 0x28,
                    s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
                        s.as_bytes()[0] as u16
                    }
                    s if s.starts_with('F') => s[1..]
                        .parse::<u16>()
                        .ok()
                        .filter(|n| (1..=12).contains(n))
                        .map(|n| 0x6f + n)
                        .ok_or("功能键必须为 F1–F12")?,
                    _ => return Err("不支持的按键".into()),
                };
                if primary.replace(key).is_some() {
                    return Err("只能包含一个主键".into());
                }
            }
        }
        let primary = primary.ok_or("缺少主键")?;
        modifiers.sort_by_key(|k| match k {
            0x11 => 0,
            0x12 => 1,
            0x10 => 2,
            _ => 3,
        });
        modifiers.push(primary);
        Ok(Self { keys: modifiers })
    }
    pub fn events(&self) -> Vec<KeyEvent> {
        self.keys
            .iter()
            .map(|&key| KeyEvent {
                key,
                up: false,
                extended: extended(key),
            })
            .chain(self.keys.iter().rev().map(|&key| KeyEvent {
                key,
                up: true,
                extended: extended(key),
            }))
            .collect()
    }
    pub fn releases(&self) -> Vec<KeyEvent> {
        self.events().into_iter().filter(|e| e.up).collect()
    }
}
fn extended(key: u16) -> bool {
    matches!(key, 0x25..=0x28 | 0x5b)
}
