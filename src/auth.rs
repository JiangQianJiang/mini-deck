use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    digest: String,
    id: String,
    name: String,
    expires_at: u64,
    remembered: bool,
}
#[derive(Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub expires_at: u64,
    pub remembered: bool,
    pub current: bool,
}
#[derive(Clone)]
pub struct Auth {
    pub code: String,
    created: Instant,
    attempts: u8,
    blocked_until: Option<Instant>,
    sessions: HashMap<String, Session>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn random_hex(bytes: usize) -> Result<String, String> {
    let mut data = vec![0; bytes];
    getrandom::getrandom(&mut data).map_err(|e| format!("系统随机数失败：{e}"))?;
    Ok(data.iter().map(|b| format!("{b:02x}")).collect())
}
impl Auth {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            code: random_hex(4)?,
            created: Instant::now(),
            attempts: 0,
            blocked_until: None,
            sessions: HashMap::new(),
        })
    }
    pub async fn load(path: &Path) -> Result<Self, String> {
        let mut auth = Self::new()?;
        let bytes = match tokio::fs::read(path).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(auth),
            Err(e) => return Err(format!("设备文件读取失败：{e}")),
        };
        if bytes.len() > 65536 {
            return Err("设备文件过大".into());
        }
        let sessions: Vec<Session> =
            serde_json::from_slice(&bytes).map_err(|e| format!("设备文件解析失败：{e}"))?;
        if sessions.len() > 16 {
            return Err("设备文件超过 16 个会话".into());
        }
        for session in sessions {
            if session.digest.len() != 64
                || !session.digest.bytes().all(|b| b.is_ascii_hexdigit())
                || session.id.len() != 16
                || !session.id.bytes().all(|b| b.is_ascii_hexdigit())
                || session.name.chars().count() > 40
                || !session.remembered
            {
                return Err("设备文件内容无效".into());
            }
            if session.expires_at <= now() {
                continue;
            }
            if auth.sessions.values().any(|s| s.id == session.id)
                || auth
                    .sessions
                    .insert(session.digest.clone(), session)
                    .is_some()
            {
                return Err("设备文件包含重复会话".into());
            }
        }
        Ok(auth)
    }
    pub fn pair(&mut self, code: &str) -> Result<String, String> {
        self.pair_device(code, false, "临时设备")
    }
    pub fn pair_device(
        &mut self,
        code: &str,
        remembered: bool,
        name: &str,
    ) -> Result<String, String> {
        let instant = Instant::now();
        if self.created.elapsed() > Duration::from_secs(600) {
            return Err("配对码已过期，请在电脑重启程序".into());
        }
        if self.blocked_until.is_some_and(|until| instant < until) {
            return Err("尝试过多，请稍后再试".into());
        }
        if code != self.code {
            self.attempts += 1;
            if self.attempts >= 5 {
                self.blocked_until = Some(instant + Duration::from_secs(60));
                self.attempts = 0;
            }
            return Err("配对码错误".into());
        }
        if name.trim().is_empty() || name.chars().count() > 40 {
            return Err("设备名称须为 1–40 字".into());
        }
        self.prune();
        if self.sessions.len() >= 16 {
            return Err("会话已满，请从已配对设备撤销旧会话".into());
        }
        let token = random_hex(32)?;
        let hash = digest(&token);
        self.sessions.insert(
            hash.clone(),
            Session {
                digest: hash,
                id: random_hex(8)?,
                name: name.trim().into(),
                expires_at: now() + if remembered { 30 * 86400 } else { 86400 },
                remembered,
            },
        );
        self.attempts = 0;
        Ok(token)
    }
    fn prune(&mut self) {
        let time = now();
        self.sessions.retain(|_, session| session.expires_at > time);
    }
    pub fn valid(&mut self, token: &str) -> bool {
        self.prune();
        self.sessions.contains_key(&digest(token))
    }
    pub fn logout(&mut self, token: &str) {
        self.sessions.remove(&digest(token));
    }
    pub fn devices(&mut self, token: &str) -> Vec<Device> {
        self.prune();
        let hash = digest(token);
        let mut devices: Vec<_> = self
            .sessions
            .values()
            .map(|s| Device {
                id: s.id.clone(),
                name: s.name.clone(),
                expires_at: s.expires_at,
                remembered: s.remembered,
                current: s.digest == hash,
            })
            .collect();
        devices.sort_by(|a, b| a.id.cmp(&b.id));
        devices
    }
    pub fn revoke(&mut self, id: &str) -> bool {
        let previous = self.sessions.len();
        self.sessions.retain(|_, s| s.id != id);
        previous != self.sessions.len()
    }
    pub fn snapshot(&self) -> Result<Vec<u8>, String> {
        let sessions: Vec<_> = self
            .sessions
            .values()
            .filter(|s| s.remembered && s.expires_at > now())
            .collect();
        serde_json::to_vec_pretty(&sessions).map_err(|e| e.to_string())
    }
}

pub fn session_path(config: &Path) -> std::path::PathBuf {
    let mut name = config.as_os_str().to_os_string();
    name.push(".devices.json");
    name.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_and_cooldown() {
        let mut auth = Auth::new().unwrap();
        let code = auth.code.clone();
        let token = auth.pair(&code).unwrap();
        auth.sessions.get_mut(&digest(&token)).unwrap().expires_at = now() - 1;
        assert!(!auth.valid(&token));
        for _ in 0..5 {
            assert!(auth.pair("错误").is_err());
        }
        assert!(auth.pair(&code).is_err());
        auth.blocked_until = Some(Instant::now() - Duration::from_secs(1));
        assert!(auth.pair(&code).is_ok());
        auth.created = Instant::now() - Duration::from_secs(601);
        assert!(auth.pair(&code).is_err());
    }
    #[tokio::test]
    async fn remembered_survives_restart_without_storing_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let mut auth = Auth::new().unwrap();
        let code = auth.code.clone();
        let permanent = auth.pair_device(&code, true, "手机").unwrap();
        let temporary = auth.pair(&code).unwrap();
        let bytes = auth.snapshot().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains(&permanent));
        crate::storage::write_atomic(&path, bytes).await.unwrap();
        let mut loaded = Auth::load(&path).await.unwrap();
        assert!(loaded.valid(&permanent));
        assert!(!loaded.valid(&temporary));
        let id = loaded.devices(&permanent)[0].id.clone();
        assert!(loaded.revoke(&id));
        crate::storage::write_atomic(&path, loaded.snapshot().unwrap())
            .await
            .unwrap();
        assert!(!Auth::load(&path).await.unwrap().valid(&permanent));
    }
}
