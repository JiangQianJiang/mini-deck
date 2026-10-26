use crate::shortcut::Shortcut;
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Button {
    pub id: u8,
    pub name: String,
    pub shortcut: String,
    pub position: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub buttons: Vec<Button>,
}
impl Default for Config {
    fn default() -> Self {
        let entries = [
            ("复制", "Ctrl+C"),
            ("粘贴", "Ctrl+V"),
            ("剪切", "Ctrl+X"),
            ("撤销", "Ctrl+Z"),
            ("重做", "Ctrl+Y"),
            ("全选", "Ctrl+A"),
            ("保存", "Ctrl+S"),
            ("查找", "Ctrl+F"),
            ("任务管理器", "Ctrl+Shift+Esc"),
        ];
        Self {
            buttons: entries
                .iter()
                .enumerate()
                .map(|(i, (name, shortcut))| Button {
                    id: i as u8,
                    name: name.to_string(),
                    shortcut: shortcut.to_string(),
                    position: i as u8,
                })
                .collect(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.buttons.len() != 9 {
            return Err("配置必须包含九个按钮".into());
        }
        let mut ids = Vec::new();
        let mut positions = Vec::new();
        for b in &self.buttons {
            if b.name.trim().is_empty()
                || b.name.chars().count() > 40
                || b.position > 8
                || ids.contains(&b.id)
                || positions.contains(&b.position)
            {
                return Err("按钮名称、ID 或位置无效".into());
            }
            Shortcut::parse(&b.shortcut)?;
            ids.push(b.id);
            positions.push(b.position);
        }
        Ok(())
    }
    pub async fn load(path: &Path) -> Result<Self, String> {
        match tokio::fs::read(path).await {
            Ok(bytes) => {
                if bytes.len() > 65536 {
                    return Err("配置文件超过 64 KiB".into());
                }
                let config: Self =
                    serde_json::from_slice(&bytes).map_err(|e| format!("配置解析失败：{e}"))?;
                config.validate()?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save(path).await?;
                Ok(config)
            }
            Err(e) => Err(format!("配置读取失败：{e}")),
        }
    }
    pub async fn save(&self, path: &Path) -> Result<(), String> {
        use tokio::io::AsyncWriteExt;
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let temp = path.with_extension("json.tmp");
        let result = async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .await?;
            file.write_all(&bytes).await?;
            file.sync_all().await?;
            drop(file);
            #[cfg(not(windows))]
            tokio::fs::rename(&temp, path).await?;
            #[cfg(windows)]
            {
                let from_path = temp.clone();
                let to_path = path.to_path_buf();
                tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                    use std::os::windows::ffi::OsStrExt;
                    let from: Vec<u16> =
                        from_path.as_os_str().encode_wide().chain(Some(0)).collect();
                    let to: Vec<u16> = to_path.as_os_str().encode_wide().chain(Some(0)).collect();
                    // FFI 边界：以零结尾的路径在调用期间有效。
                    let ok = unsafe {
                        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                            from.as_ptr(),
                            to.as_ptr(),
                            windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                                | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
                        )
                    };
                    if ok == 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                })
                .await
                .map_err(std::io::Error::other)??;
            }
            Ok::<(), std::io::Error>(())
        }
        .await;
        // 不删除其他进程创建的临时文件；残留文件需要人工检查后清除。
        result.map_err(|e| format!("配置保存失败：{e}；请检查临时文件 {}", temp.display()))
    }
}
