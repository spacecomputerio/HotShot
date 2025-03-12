use std::{net::SocketAddr, time};
#[allow(unused_imports)]
use std::io::Write as _;
use std::fs::File;
use hotshot_types::traits::metrics as hsmetrics;
// use warp::Filter;

/// A counter metric wrapper around the Prometheus Counter
#[derive(Debug, Clone)]
pub struct PrometheusCounter {
    /// The Prometheus counter
    counter: prometheus::Counter,
}

impl PrometheusCounter {
    /// Create a new Prometheus counter
    #[must_use]
    pub fn new(counter: prometheus::Counter) -> Self {
        Self { counter }
    }
}

impl hsmetrics::Counter for PrometheusCounter {
    #[allow(clippy::cast_precision_loss)]
    fn add(&self, amount: usize) {
        self.counter.inc_by(amount as f64);
    }
}

/// A counter family metric wrapper around the Prometheus CounterVec
#[derive(Debug, Clone)]
pub struct PrometheusCounterFamily {
    /// The Prometheus counter family
    counter_family: prometheus::CounterVec,
}

impl PrometheusCounterFamily {
    /// Create a new Prometheus counter family
    #[must_use]
    pub fn new(counter_family: prometheus::CounterVec) -> Self {
        Self { counter_family }
    }
}

impl hsmetrics::MetricsFamily<Box<dyn hsmetrics::Counter>> for PrometheusCounterFamily {
    fn create(&self, labels: Vec<String>) -> Box<dyn hsmetrics::Counter> {
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        Box::new(PrometheusCounter::new(self.counter_family.with_label_values(&vals)))
    }
}

/// A gauge metric wrapper around the Prometheus Gauge
#[derive(Debug, Clone)]
pub struct PrometheusGauge {
    /// The Prometheus gauge
    gauge: prometheus::Gauge,
}

impl PrometheusGauge {
    /// Create a new Prometheus gauge
    #[must_use]
    pub fn new(gauge: prometheus::Gauge) -> Self {
        Self { gauge }
    }
}

impl hsmetrics::Gauge for PrometheusGauge {
    /// Set the gauge value
    #[allow(clippy::cast_precision_loss)]
    fn set(&self, value: usize) {
        self.gauge.set(value as f64);
    }

    /// Update the gauge value
    #[allow(clippy::cast_precision_loss)]
    fn update(&self, delta: i64) {
        self.gauge.add(delta as f64);
    }
}

/// A gauge family metric wrapper around the Prometheus GaugeVec
#[derive(Debug, Clone)]
pub struct PrometheusGaugeFamily {
    /// The Prometheus gauge family
    gauge_family: prometheus::GaugeVec,
}

impl PrometheusGaugeFamily {
    /// Create a new Prometheus gauge family
    #[must_use]
    pub fn new(gauge_family: prometheus::GaugeVec) -> Self {
        Self { gauge_family }
    }
}

impl hsmetrics::MetricsFamily<Box<dyn hsmetrics::Gauge>> for PrometheusGaugeFamily {
    fn create(&self, labels: Vec<String>) -> Box<dyn hsmetrics::Gauge> {
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        Box::new(PrometheusGauge::new(self.gauge_family.with_label_values(&vals)))
    }
}

/// A histogram metric wrapper around the Prometheus Histogram
#[derive(Debug, Clone)]
pub struct PrometheusTextGaugeFamily {
    /// The Prometheus gauge family
    gauge_family: prometheus::GaugeVec,
}

impl PrometheusTextGaugeFamily {
    /// Create a new Prometheus gauge family
    #[must_use]
    pub fn new(gauge_family: prometheus::GaugeVec) -> Self {
        Self { gauge_family }
    }
}

impl hsmetrics::MetricsFamily<()> for PrometheusTextGaugeFamily {
    fn create(&self, labels: Vec<String>) {
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        self.gauge_family.with_label_values(&vals).set(1.0);
    }
}

/// A histogram metric wrapper around the Prometheus Histogram
#[derive(Debug, Clone)]
pub struct PrometheusHistogram {
    /// The Prometheus histogram
    histogram: prometheus::Histogram,
}

impl PrometheusHistogram {
    /// Create a new Prometheus histogram
    #[must_use]
    pub fn new(histogram: prometheus::Histogram) -> Self {
        Self { histogram }
    }
}

impl hsmetrics::Histogram for PrometheusHistogram {
    fn add_point(&self, point: f64) {
        self.histogram.observe(point);
    }
}

/// A histogram family metric wrapper around the Prometheus HistogramVec
#[derive(Debug, Clone)]
pub struct PrometheusHistogramFamily {
    /// The Prometheus histogram family
    histogram_family: prometheus::HistogramVec,
}

impl PrometheusHistogramFamily {
    /// Create a new Prometheus histogram family
    #[must_use]
    pub fn new(histogram_family: prometheus::HistogramVec) -> Self {
        Self { histogram_family }
    }
}

impl hsmetrics::MetricsFamily<Box<dyn hsmetrics::Histogram>> for PrometheusHistogramFamily {
    fn create(&self, labels: Vec<String>) -> Box<dyn hsmetrics::Histogram> {
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        Box::new(PrometheusHistogram::new(self.histogram_family.with_label_values(&vals)))
    }
}

 /// Flush the metrics to a file
 #[allow(clippy::missing_panics_doc)]
 pub fn flush_metrics(folder: &str, prefix: Option<String>, raw_metrics: &str) {
    if raw_metrics.is_empty() {
        return;
    }
    let file_prefix = match prefix {
        Some(p) => format!("metrics_{p}"),
        None => "metrics".to_string(),
    };
    let timestamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
    let file_path = format!("{folder}/{file_prefix}_{timestamp}.prom");
    match std::fs::create_dir_all(folder) {
       Ok(()) => tracing::info!("Successfully created metrics folder: {folder}"),
        Err(e) => {
            tracing::error!("Failed to create folder: {folder}: {e}");
            return;
        },
    }
    tracing::info!("Writing metrics to file: {file_path}");
    match File::create(file_path) {
        Ok(mut f) => {
            match f.write_all(raw_metrics.as_bytes()) {
                Ok(()) => {
                    tracing::info!("Successfully wrote metrics to file");
                    // flush/close file
                    match f.sync_all() {
                       Ok(()) => (),
                        Err(e) => tracing::error!("Failed to flush metrics to file: {}", e),
                    }
                },
                Err(e) => tracing::error!("Failed to write metrics to file: {}", e),
            }
        }
        Err(e) => tracing::error!("Failed to create file: {}", e),
    }
}

/// A metrics implementation that uses Prometheus as the backend
#[derive(Debug, Clone, Default)]
pub struct PrometheusMetrics {
    /// The Prometheus registry
    registry: prometheus::Registry,
    /// The port the metrics server is running on
    port: Option<u16>,
    /// The prefix/namespace for the metrics
    prefix: Option<String>,
    /// The folder where the metrics are stored
    folder: Option<String>,
    /// The interval in seconds to gather metrics
    metrics_interva_sec: usize,
}

impl PrometheusMetrics {
    /// Create a new Prometheus metrics instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: prometheus::Registry::new(),
            port: None,
            prefix: None,
            folder: None,
            metrics_interva_sec: 60,
        }
    }

    /// Create a new Prometheus metrics instance with a prefix/namespace
    #[must_use]
    pub fn new_with_prefix(
        registry: prometheus::Registry,
        port: Option<u16>,
        prefix: String,
        folder: Option<String>,
        metrics_interva_sec: usize,
    ) -> Self {
        Self {
            registry,
            port,
            prefix: Some(prefix),
            folder,
            metrics_interva_sec,
        }
    }

    /// Get the name of the metric with the prefix
    #[must_use]
    fn get_name(&self, name: String) -> String {
        match &self.prefix {
            Some(p) => format!("{p}_{name}"),
            None => name,
        }
    }

    /// Get the Prometheus registry
    #[must_use]
    pub fn get_registry(&self) -> prometheus::Registry {
        self.registry.clone()
    }

    /// Get the port the metrics server is running on
    #[must_use]
    pub fn get_port(&self) -> Option<u16> {
        self.port
    }

    /// Get the folder where the metrics are stored
    #[must_use]
    pub fn get_folder(&self) -> Option<String> {
        self.folder.clone()
    }

    /// Get the prefix/namespace for the metrics
    #[must_use]
    pub fn get_prefix(&self) -> Option<String> {
        self.prefix.clone()
    }

    /// Get the metrics interval in seconds
    #[must_use]
    pub fn get_interva_sec(&self) -> usize {
        self.metrics_interva_sec
    }

    /// Gather the metrics and return them as a string
    #[must_use]
    pub fn gather(&self) -> Option<String> {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();

        match encoder.encode_to_string(&metric_families) {
            Ok(raw) => Some(raw),
            Err(_) => None,
        }
    }
}

impl hsmetrics::Metrics for PrometheusMetrics {
    fn create_counter(&self, name: String, _unit_label: Option<String>) -> Box<dyn hsmetrics::Counter> {
        let formatted_name = self.get_name(name.clone());
        let counter = prometheus::Counter::new(formatted_name, format!("Counter for {name}")).unwrap();
        self.registry.register(Box::new(counter.clone())).unwrap();
        Box::new(PrometheusCounter::new(counter))
    }

    fn create_gauge(&self, name: String, _unit_label: Option<String>) -> Box<dyn hsmetrics::Gauge> {
        let formatted_name = self.get_name(name.clone());
        let gauge = prometheus::Gauge::new(formatted_name, format!("Gauge for {name}")).unwrap();
        self.registry.register(Box::new(gauge.clone())).unwrap();
        Box::new(PrometheusGauge::new(gauge))
    }

    fn create_histogram(&self, name: String, _unit_label: Option<String>) -> Box<dyn hsmetrics::Histogram> {
        let formatted_name = self.get_name(name.clone());
        let histogram = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(formatted_name, format!("Histogram for {name}")).buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 7.5, 10.0])).unwrap();
        self.registry.register(Box::new(histogram.clone())).unwrap();
        Box::new(PrometheusHistogram::new(histogram))
    }

    fn create_text(&self, name: String) {
        let formatted_name = self.get_name(name.clone());
        let gauge = prometheus::Gauge::new(formatted_name, format!("Gauge for {name}")).unwrap();
        self.registry.register(Box::new(gauge.clone())).unwrap();
        gauge.set(1.0);
    }

    fn counter_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::CounterFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        let counter_vec = prometheus::CounterVec::new(
            prometheus::Opts::new(formatted_name, format!("Counter family for {name}")),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(counter_vec.clone())).unwrap();
        Box::new(PrometheusCounterFamily::new(counter_vec))
    }

    fn gauge_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::GaugeFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        let gauge_vec = prometheus::GaugeVec::new(
            prometheus::Opts::new(formatted_name, format!("Gauge family for {name}")),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(gauge_vec.clone())).unwrap();
        Box::new(PrometheusGaugeFamily::new(gauge_vec))
    }

    fn histogram_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::HistogramFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        let histogram_vec = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(formatted_name, format!("Histogram family for {name}")).buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 7.5, 10.0]),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(histogram_vec.clone())).unwrap();
        Box::new(PrometheusHistogramFamily::new(histogram_vec)) 
    }

    fn text_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::TextFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(std::string::String::as_str).collect();
        let gauge_vec = prometheus::GaugeVec::new(
            prometheus::Opts::new(formatted_name, format!("Gauge family for {name}")),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(gauge_vec.clone())).unwrap();
        Box::new(PrometheusTextGaugeFamily::new(gauge_vec))
    }

    fn subgroup(&self, subgroup_name: String) -> Box<dyn hsmetrics::Metrics> {
        let prefix = match &self.prefix {
            Some(p) => format!("{p}_{subgroup_name}"),
            None => subgroup_name,
        };
        Box::new(PrometheusMetrics::new_with_prefix(self.registry.clone(), self.port, prefix, self.folder.clone(), self.metrics_interva_sec))
    }
}

/// Start the metrics server that should run forever on a particular port
pub async fn serve_metrics(bind_endpoint: SocketAddr, prom_registry: prometheus::Registry) {
    let prom_filter = warp::any().map(move || prom_registry.clone());
    // The `/metrics` route is standard for Prometheus deployments
    let route = warp::path("metrics")
        .and(prom_filter)
        .and_then( |prom_registry: prometheus::Registry| async move {
            tracing::debug!("Serving metrics");
            // Gather all metrics, encode them, and return them.
            let encoder = prometheus::TextEncoder::new();
            let metric_families = prom_registry.gather();

            match encoder.encode_to_string(&metric_families) {
                Ok(raw) => Ok(warp::reply::html(raw)),
                Err(_) => Err(warp::reject()),
            }
        });

    // Serve the route on the specified port
    warp::serve(route).run(bind_endpoint).await;
}
