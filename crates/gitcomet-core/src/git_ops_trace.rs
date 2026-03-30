#[derive(Clone, Copy, Debug, Default)]
pub struct GitOpTraceCounters {
    pub calls: u64,
    pub total_nanos: u64,
}

impl GitOpTraceCounters {
    pub fn total_millis(self) -> f64 {
        self.total_nanos as f64 / 1_000_000.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GitOpTraceSnapshot {
    pub status: GitOpTraceCounters,
    pub log_walk: GitOpTraceCounters,
    pub diff: GitOpTraceCounters,
    pub blame: GitOpTraceCounters,
    pub ref_enumerate: GitOpTraceCounters,
}

#[derive(Debug, Default)]
pub struct GitOpTraceCaptureGuard;

pub fn capture() -> GitOpTraceCaptureGuard {
    GitOpTraceCaptureGuard
}

pub fn snapshot() -> GitOpTraceSnapshot {
    GitOpTraceSnapshot::default()
}
