use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
pub struct Auth {
    pub code: String,
    created: Instant,
    attempts: u8,
    blocked_until: Option<Instant>,
    sessions: HashMap<String, Instant>,
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
    pub fn pair(&mut self, code: &str) -> Result<String, String> {
        let now = Instant::now();
        if self.created.elapsed() > Duration::from_secs(600) {
            return Err("配对码已过期，请在电脑重启程序".into());
        }
        if self.blocked_until.is_some_and(|until| now < until) {
            return Err("尝试过多，请稍后再试".into());
        }
        if code != self.code {
            self.attempts += 1;
            if self.attempts >= 5 {
                self.blocked_until = Some(now + Duration::from_secs(60));
                self.attempts = 0;
            }
            return Err("配对码错误".into());
        }
        self.sessions.retain(|_, expiry| *expiry > now);
        if self.sessions.len() >= 16 {
            return Err("会话已满，请在电脑重启程序撤销会话".into());
        }
        let token = random_hex(32)?;
        self.sessions
            .insert(token.clone(), now + Duration::from_secs(86400));
        self.attempts = 0;
        Ok(token)
    }
    pub fn valid(&mut self, token: &str) -> bool {
        let now = Instant::now();
        self.sessions.retain(|_, expiry| *expiry > now);
        self.sessions.contains_key(token)
    }
    pub fn logout(&mut self, token: &str) {
        self.sessions.remove(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_and_cooldown() {
        let mut auth = Auth::new().unwrap();
        let code = auth.code.clone();
        let token = auth.pair(&code).unwrap();
        auth.sessions
            .insert(token.clone(), Instant::now() - Duration::from_secs(1));
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
}
