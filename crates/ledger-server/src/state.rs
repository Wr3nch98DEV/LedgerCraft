use ledger_core::Ledger;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

pub struct Org {
    pub path: PathBuf,
    pub ledger: Ledger,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub user_id: i64,
    pub role: String,
    pub perms: Vec<String>,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub org: Mutex<Option<Org>>,
    pub sessions: Mutex<HashMap<String, Session>>,
}

pub fn default_data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("LEDGERCRAFT_DATA") { return PathBuf::from(d); }
    #[cfg(windows)]
    { if let Ok(a) = std::env::var("APPDATA") { return PathBuf::from(a).join("LedgerCraft"); } }
    if let Ok(h) = std::env::var("HOME") { return PathBuf::from(h).join(".ledgercraft"); }
    PathBuf::from("ledgercraft-data")
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> AppState {
        AppState { data_dir, org: Mutex::new(None), sessions: Mutex::new(HashMap::new()) }
    }

    pub fn orgs_dir(&self) -> PathBuf { self.data_dir.join("orgs") }

    /// Re-open the most recently used organisation on startup.
    pub fn auto_open(&self) {
        let last = std::fs::read_to_string(self.data_dir.join("last_org.txt")).ok().map(|s| PathBuf::from(s.trim()));
        let path = last.filter(|p| p.exists()).or_else(|| {
            std::fs::read_dir(self.orgs_dir()).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().map(|x| x == "lcdb").unwrap_or(false))
        });
        if let Some(p) = path { let _ = self.open_org(&p); }
    }

    pub fn open_org(&self, path: &std::path::Path) -> Result<(), String> {
        let ledger = Ledger::open(path).map_err(|e| e.to_string())?;
        *self.org.lock().unwrap() = Some(Org { path: path.to_path_buf(), ledger });
        self.sessions.lock().unwrap().clear();
        std::fs::write(self.data_dir.join("last_org.txt"), path.display().to_string()).ok();
        Ok(())
    }

    pub fn list_orgs(&self) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let current = self.org.lock().unwrap().as_ref().map(|o| o.path.clone());
        if let Ok(rd) = std::fs::read_dir(self.orgs_dir()) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x != "lcdb").unwrap_or(true) { continue; }
                let name = if Some(&p) == current.as_ref() {
                    self.org.lock().unwrap().as_ref().and_then(|o| o.ledger.setting("org_name").ok().flatten())
                } else {
                    rusqlite_name(&p)
                };
                out.push(serde_json::json!({"file": p.file_name().unwrap().to_string_lossy(), "name": name.unwrap_or_else(|| "(not set up)".into()), "current": Some(&p) == current.as_ref(),
                    "size": e.metadata().map(|m| m.len()).unwrap_or(0)}));
            }
        }
        out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        out
    }
}

fn rusqlite_name(p: &std::path::Path) -> Option<String> {
    let l = Ledger::open(p).ok()?;
    l.setting("org_name").ok().flatten()
}
