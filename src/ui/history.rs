/// Run history — persists workflow run events to ~/.zenith/logs/<run-id>/
///
/// Directory structure:
///   ~/.zenith/logs/<run-id>/
///     summary.json   — job name, status, timing, step count
///     steps.jsonl    — one JSON object per line, one per step event

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id:          String,
    pub job:             String,
    pub status:          RunOutcome,
    pub started_at_secs: u64,
    pub finished_at_secs: Option<u64>,
    pub step_count:      usize,
    pub steps_ok:        usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunOutcome { Running, Success, Failed }

impl std::fmt::Display for RunOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunOutcome::Running => write!(f, "running"),
            RunOutcome::Success => write!(f, "success"),
            RunOutcome::Failed  => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepEvent {
    pub step_idx:   usize,
    pub name:       String,
    pub status:     StepStatus,
    pub at_secs:    u64,
    pub log_lines:  Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus { Started, Done, Failed, Cached }

// ─── Logger (writer side) ─────────────────────────────────────────────────────

pub struct RunLogger {
    pub run_id:  String,
    log_dir:     PathBuf,
    job:         String,
    started_at:  u64,
    step_count:  usize,
    steps_ok:    usize,
}

impl RunLogger {
    pub fn new(job: &str) -> Self {
        let run_id = uuid::Uuid::new_v4().simple().to_string();
        let log_dir = logs_dir().join(&run_id);
        let _ = std::fs::create_dir_all(&log_dir);
        let started_at = now_secs();

        let logger = Self {
            run_id: run_id.clone(),
            log_dir,
            job: job.to_string(),
            started_at,
            step_count: 0,
            steps_ok: 0,
        };
        logger.write_summary(RunOutcome::Running);
        logger
    }

    pub fn log_step_start(&self, step_idx: usize, name: &str) {
        let ev = StepEvent {
            step_idx, name: name.to_string(),
            status: StepStatus::Started,
            at_secs: now_secs(), log_lines: vec![],
        };
        self.append_event(&ev);
    }

    pub fn log_step_done(&mut self, step_idx: usize, name: &str, ok: bool, lines: Vec<String>) {
        self.step_count += 1;
        if ok { self.steps_ok += 1; }
        let ev = StepEvent {
            step_idx, name: name.to_string(),
            status: if ok { StepStatus::Done } else { StepStatus::Failed },
            at_secs: now_secs(), log_lines: lines,
        };
        self.append_event(&ev);
    }

    pub fn log_step_cached(&mut self, step_idx: usize, name: &str) {
        self.step_count += 1;
        self.steps_ok += 1;
        let ev = StepEvent {
            step_idx, name: name.to_string(),
            status: StepStatus::Cached,
            at_secs: now_secs(), log_lines: vec![],
        };
        self.append_event(&ev);
    }

    pub fn finalize(&self, success: bool) {
        self.write_summary(if success { RunOutcome::Success } else { RunOutcome::Failed });
    }

    fn write_summary(&self, status: RunOutcome) {
        let summary = RunSummary {
            run_id: self.run_id.clone(),
            job: self.job.clone(),
            status,
            started_at_secs: self.started_at,
            finished_at_secs: Some(now_secs()),
            step_count: self.step_count,
            steps_ok: self.steps_ok,
        };
        if let Ok(json) = serde_json::to_string_pretty(&summary) {
            let _ = std::fs::write(self.log_dir.join("summary.json"), json);
        }
    }

    fn append_event(&self, ev: &StepEvent) {
        if let Ok(line) = serde_json::to_string(ev) {
            let path = self.log_dir.join("steps.jsonl");
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}", line);
            }
        }
    }
}

// ─── Reader (API side) ────────────────────────────────────────────────────────

pub fn logs_dir() -> PathBuf {
    crate::sandbox::zenith_home().join("logs")
}

/// List all run summaries, sorted newest first.
pub fn list_runs(limit: usize) -> Vec<RunSummary> {
    let dir = logs_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else { return vec![] };

    let mut runs: Vec<RunSummary> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let path = e.path().join("summary.json");
            let raw = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&raw).ok()
        })
        .collect();

    runs.sort_by(|a, b| b.started_at_secs.cmp(&a.started_at_secs));
    runs.truncate(limit);
    runs
}

/// Read all step events for a run.
pub fn get_steps(run_id: &str) -> Vec<StepEvent> {
    let path = logs_dir().join(run_id).join("steps.jsonl");
    let Ok(raw) = std::fs::read_to_string(path) else { return vec![] };
    raw.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Read the summary for a single run.
pub fn get_run(run_id: &str) -> Option<RunSummary> {
    let path = logs_dir().join(run_id).join("summary.json");
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
