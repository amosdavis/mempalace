use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MineStatus {
    Running,
    Done,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MineProgress {
    pub status: MineStatus,
    pub dir: String,
    pub wing: String,
    pub job_id: Option<String>,
    pub files_total: usize,
    pub files_done: usize,
    pub files_skipped: usize,
    pub chunks_created: usize,
    pub errors_count: usize,
    pub current_file: String,
    pub start_unix: f64,
    pub end_unix: Option<f64>,
    pub pid: u32,
}

impl MineProgress {
    pub fn new(dir: &str, wing: &str, files_total: usize) -> Self {
        Self {
            status: MineStatus::Running,
            dir: dir.to_string(),
            wing: wing.to_string(),
            job_id: None,
            files_total,
            files_done: 0,
            files_skipped: 0,
            chunks_created: 0,
            errors_count: 0,
            current_file: String::new(),
            start_unix: now_secs(),
            end_unix: None,
            pid: std::process::id(),
        }
    }

    pub fn with_job_id(mut self, job_id: &str) -> Self {
        self.job_id = Some(job_id.to_string());
        self
    }

    /// Elapsed seconds since mining started.
    pub fn elapsed_secs(&self) -> f64 {
        let end = self.end_unix.unwrap_or_else(now_secs);
        (end - self.start_unix).max(0.0)
    }

    /// Estimated seconds remaining (None if no progress yet).
    pub fn eta_secs(&self) -> Option<f64> {
        if self.files_done == 0 || self.files_total == 0 {
            return None;
        }
        let elapsed = self.elapsed_secs();
        let rate = self.files_done as f64 / elapsed;
        let remaining = (self.files_total.saturating_sub(self.files_done)) as f64;
        Some(remaining / rate)
    }

    /// 0.0 – 1.0 fraction of files processed out of total expected.
    pub fn fraction(&self) -> f64 {
        if self.files_total == 0 { return 0.0; }
        (self.files_done as f64 / self.files_total as f64).min(1.0)
    }

    /// ASCII progress bar (width=20).
    pub fn bar(&self, width: usize) -> String {
        let filled = ((self.fraction() * width as f64).round() as usize).min(width);
        let empty = width - filled;
        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }

    /// Human-readable formatted status block (matches the "progress" CLI output).
    pub fn format_display(&self) -> String {
        let status_line = match self.status {
            MineStatus::Running => "⛏  Mining in progress",
            MineStatus::Done => "✓  Mining complete",
            MineStatus::Idle => "—  No mining in progress",
        };

        let mut lines = vec![
            status_line.to_string(),
            format!("   Dir     {}", self.dir),
            format!("   Wing    {}", self.wing),
        ];

        let pct = (self.fraction() * 100.0).round() as u32;
        lines.push(format!(
            "   Progress {}  {}%  ({}/{} files, {} chunks)",
            self.bar(20),
            pct,
            self.files_done,
            self.files_total,
            self.chunks_created,
        ));
        lines.push(format!("   Elapsed  {}", fmt_duration(self.elapsed_secs())));

        if let Some(eta) = self.eta_secs() {
            if matches!(self.status, MineStatus::Running) {
                lines.push(format!("   Remaining ~{}", fmt_duration(eta)));
            }
        }

        if !self.current_file.is_empty() {
            if matches!(self.status, MineStatus::Running) {
                lines.push(format!("   Current  {}", self.current_file));
            }
        }

        if self.errors_count > 0 {
            lines.push(format!("   Errors   {}", self.errors_count));
        }

        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// State file helpers
// ---------------------------------------------------------------------------

fn jobs_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mempalace")
        .join("jobs")
}

pub fn progress_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mempalace")
        .join("mine_progress.json")
}

pub fn progress_path_for_job(job_id: &str) -> PathBuf {
    jobs_dir().join(format!("{}.json", job_id))
}

pub fn write_progress(p: &MineProgress) {
    let path = if let Some(ref jid) = p.job_id {
        let jp = progress_path_for_job(jid);
        if let Some(dir) = jp.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        jp
    } else {
        let pp = progress_path();
        if let Some(dir) = pp.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        pp
    };

    if let Ok(json) = serde_json::to_string(p) {
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn read_progress() -> Option<MineProgress> {
    let bytes = std::fs::read(progress_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn read_progress_for_job(job_id: &str) -> Option<MineProgress> {
    let bytes = std::fs::read(progress_path_for_job(job_id)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn list_active_jobs() -> Vec<MineProgress> {
    let dir = jobs_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .filter_map(|e| {
            let bytes = std::fs::read(e.path()).ok()?;
            serde_json::from_slice::<MineProgress>(&bytes).ok()
        })
        .collect()
}

pub fn clear_progress() {
    let _ = std::fs::remove_file(progress_path());
}

pub fn clear_job_progress(job_id: &str) {
    let _ = std::fs::remove_file(progress_path_for_job(job_id));
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Format seconds → HH:MM:SS or MM:SS.
pub fn fmt_duration(secs: f64) -> String {
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}
