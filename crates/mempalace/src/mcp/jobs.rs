use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mining::progress::MineProgress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MineJob {
    pub job_id: String,
    pub dir: String,
    pub wing: String,
    pub status: MineJobStatus,
    pub progress: Option<MineProgress>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MineJobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, MineJob>>>,
    max_concurrent: usize,
}

impl JobManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent,
        }
    }

    pub async fn submit_job(&self, dir: String, wing: String) -> String {
        let mut jobs = self.jobs.lock().await;

        for (id, job) in jobs.iter() {
            if job.dir == dir
                && job.wing == wing
                && matches!(job.status, MineJobStatus::Queued | MineJobStatus::Running)
            {
                return id.clone();
            }
        }

        let job_id = Uuid::new_v4().to_string()[..8].to_string();
        let job = MineJob {
            job_id: job_id.clone(),
            dir,
            wing,
            status: MineJobStatus::Queued,
            progress: None,
            error: None,
        };
        jobs.insert(job_id.clone(), job);
        job_id
    }

    pub async fn mark_running(&self, job_id: &str, progress: MineProgress) {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = MineJobStatus::Running;
            job.progress = Some(progress);
        }
    }

    pub async fn update_progress(&self, job_id: &str, progress: MineProgress) {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.progress = Some(progress);
        }
    }

    pub async fn mark_done(&self, job_id: &str, progress: MineProgress) {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = MineJobStatus::Done;
            job.progress = Some(progress);
        }
    }

    pub async fn mark_failed(&self, job_id: &str, error: String) {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = MineJobStatus::Failed;
            job.error = Some(error);
        }
    }

    pub async fn cancel_job(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(job_id) {
            if matches!(job.status, MineJobStatus::Queued | MineJobStatus::Running) {
                job.status = MineJobStatus::Cancelled;
                return true;
            }
        }
        false
    }

    pub async fn get_job(&self, job_id: &str) -> Option<MineJob> {
        let jobs = self.jobs.lock().await;
        jobs.get(job_id).cloned()
    }

    pub async fn list_active(&self) -> Vec<MineJob> {
        let jobs = self.jobs.lock().await;
        jobs.values()
            .filter(|j| matches!(j.status, MineJobStatus::Queued | MineJobStatus::Running))
            .cloned()
            .collect()
    }

    pub async fn list_all(&self) -> Vec<MineJob> {
        let jobs = self.jobs.lock().await;
        jobs.values().cloned().collect()
    }

    pub async fn running_count(&self) -> usize {
        let jobs = self.jobs.lock().await;
        jobs.values()
            .filter(|j| j.status == MineJobStatus::Running)
            .count()
    }

    pub async fn can_start(&self) -> bool {
        self.running_count().await < self.max_concurrent
    }
}
