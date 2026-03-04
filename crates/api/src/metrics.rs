use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct ApiMetrics {
    requests_total: AtomicU64,
    requests_in_flight: AtomicI64,
    requests_2xx: AtomicU64,
    requests_4xx: AtomicU64,
    requests_5xx: AtomicU64,
    request_duration_ms_sum: AtomicU64,
    request_duration_ms_count: AtomicU64,
}

impl ApiMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_request_start(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_in_flight.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_request_end(&self, status_code: u16, duration_ms: u64) {
        self.requests_in_flight.fetch_sub(1, Ordering::Relaxed);

        match status_code {
            200..=299 => {
                self.requests_2xx.fetch_add(1, Ordering::Relaxed);
            }
            400..=499 => {
                self.requests_4xx.fetch_add(1, Ordering::Relaxed);
            }
            500..=599 => {
                self.requests_5xx.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        self.request_duration_ms_sum
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.request_duration_ms_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn render_prometheus(&self) -> String {
        let requests_total = self.requests_total.load(Ordering::Relaxed);
        let requests_in_flight = self.requests_in_flight.load(Ordering::Relaxed);
        let requests_2xx = self.requests_2xx.load(Ordering::Relaxed);
        let requests_4xx = self.requests_4xx.load(Ordering::Relaxed);
        let requests_5xx = self.requests_5xx.load(Ordering::Relaxed);
        let duration_sum_seconds =
            self.request_duration_ms_sum.load(Ordering::Relaxed) as f64 / 1000.0;
        let duration_count = self.request_duration_ms_count.load(Ordering::Relaxed);

        format!(
            "# HELP http_requests_total Total HTTP requests\n\
# TYPE http_requests_total counter\n\
http_requests_total {requests_total}\n\
# HELP http_requests_in_flight In-flight HTTP requests\n\
# TYPE http_requests_in_flight gauge\n\
http_requests_in_flight {requests_in_flight}\n\
# HELP http_requests_by_class_total HTTP requests by status class\n\
# TYPE http_requests_by_class_total counter\n\
http_requests_by_class_total{{class=\"2xx\"}} {requests_2xx}\n\
http_requests_by_class_total{{class=\"4xx\"}} {requests_4xx}\n\
http_requests_by_class_total{{class=\"5xx\"}} {requests_5xx}\n\
# HELP http_request_duration_seconds_sum Sum of request durations in seconds\n\
# TYPE http_request_duration_seconds_sum counter\n\
http_request_duration_seconds_sum {duration_sum_seconds}\n\
# HELP http_request_duration_seconds_count Number of observed request durations\n\
# TYPE http_request_duration_seconds_count counter\n\
http_request_duration_seconds_count {duration_count}\n"
        )
    }
}
