use std::{net::SocketAddr, time};
#[allow(unused_imports)]
use std::io::Write as _;
use std::fs::File;
use hotshot_types::traits::metrics as hsmetrics;
// use warp::Filter;

/// A counter metric wrapper around the Prometheus Counter
#[derive(Debug, Clone)]
pub struct PrometheusCounter {
    counter: prometheus::Counter,
}

impl PrometheusCounter {
    /// Create a new Prometheus counter
    pub fn new(counter: prometheus::Counter) -> Self {
        Self { counter }
    }
}

impl hsmetrics::Counter for PrometheusCounter {
    fn add(&self, amount: usize) {
        self.counter.inc_by(amount as f64);
    }
}

/// A counter family metric wrapper around the Prometheus CounterVec
#[derive(Debug, Clone)]
pub struct PrometheusCounterFamily {
    counter_family: prometheus::CounterVec,
}

impl PrometheusCounterFamily {
    /// Create a new Prometheus counter family
    pub fn new(counter_family: prometheus::CounterVec) -> Self {
        Self { counter_family }
    }
}

impl hsmetrics::MetricsFamily<Box<dyn hsmetrics::Counter>> for PrometheusCounterFamily {
    fn create(&self, labels: Vec<String>) -> Box<dyn hsmetrics::Counter> {
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        Box::new(PrometheusCounter::new(self.counter_family.with_label_values(&vals)))
    }
}

/// A gauge metric wrapper around the Prometheus Gauge
#[derive(Debug, Clone)]
pub struct PrometheusGauge {
    gauge: prometheus::Gauge,
}

impl PrometheusGauge {
    /// Create a new Prometheus gauge
    pub fn new(gauge: prometheus::Gauge) -> Self {
        Self { gauge }
    }
}

impl hsmetrics::Gauge for PrometheusGauge {
    /// Set the gauge value
    fn set(&self, value: usize) {
        self.gauge.set(value as f64);
    }

    /// Update the gauge value
    fn update(&self, delta: i64) {
        self.gauge.add(delta as f64);
    }
}

/// A gauge family metric wrapper around the Prometheus GaugeVec
#[derive(Debug, Clone)]
pub struct PrometheusGaugeFamily {
    gauge_family: prometheus::GaugeVec,
}

impl PrometheusGaugeFamily {
    /// Create a new Prometheus gauge family
    pub fn new(gauge_family: prometheus::GaugeVec) -> Self {
        Self { gauge_family }
    }
}

impl hsmetrics::MetricsFamily<Box<dyn hsmetrics::Gauge>> for PrometheusGaugeFamily {
    fn create(&self, labels: Vec<String>) -> Box<dyn hsmetrics::Gauge> {
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        Box::new(PrometheusGauge::new(self.gauge_family.with_label_values(&vals)))
    }
}

/// A histogram metric wrapper around the Prometheus Histogram
#[derive(Debug, Clone)]
pub struct PrometheusTextGaugeFamily {
    gauge_family: prometheus::GaugeVec,
}

impl PrometheusTextGaugeFamily {
    /// Create a new Prometheus gauge family
    pub fn new(gauge_family: prometheus::GaugeVec) -> Self {
        Self { gauge_family }
    }
}

impl hsmetrics::MetricsFamily<()> for PrometheusTextGaugeFamily {
    fn create(&self, labels: Vec<String>) -> () {
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        self.gauge_family.with_label_values(&vals).set(1.0);
        ()
    }
}

/// A histogram metric wrapper around the Prometheus Histogram
#[derive(Debug, Clone)]
pub struct PrometheusHistogram {
    histogram: prometheus::Histogram,
}

impl PrometheusHistogram {
    /// Create a new Prometheus histogram
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
    histogram_family: prometheus::HistogramVec,
}

impl PrometheusHistogramFamily {
    /// Create a new Prometheus histogram family
    pub fn new(histogram_family: prometheus::HistogramVec) -> Self {
        Self { histogram_family }
    }
}

impl hsmetrics::MetricsFamily<Box<dyn hsmetrics::Histogram>> for PrometheusHistogramFamily {
    fn create(&self, labels: Vec<String>) -> Box<dyn hsmetrics::Histogram> {
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        Box::new(PrometheusHistogram::new(self.histogram_family.with_label_values(&vals)))
    }
}

 /// Flush the metrics to a file
 pub fn flush_metrics(folder: String, prefix: Option<String>, raw_metrics: String) {
    if raw_metrics.is_empty() {
        return;
    }
    let file_prefix = match prefix {
        Some(p) => format!("metrics_{p}"),
        None => "metrics".to_string(),
    };
    let timestamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
    let file = format!("{folder}/{file_prefix}_{timestamp}.prom");
    tracing::info!("Writing metrics to file: {}", file);
    match File::create(file) {
        Ok(mut f) => {
            match f.write_all(raw_metrics.as_bytes()) {
                Ok(_) => tracing::info!("Successfully wrote metrics to file"),
                Err(e) => tracing::error!("Failed to write metrics to file: {}", e),
            }
        }
        Err(e) => tracing::error!("Failed to create file: {}", e),
    }
}

/// A metrics implementation that uses Prometheus as the backend
#[derive(Debug, Clone, Default)]
pub struct PrometheusMetrics {
    registry: prometheus::Registry,
    port: Option<u16>,
    prefix: Option<String>,
    folder: Option<String>,
}

impl PrometheusMetrics {
    /// Create a new Prometheus metrics instance
    pub fn new() -> Self {
        Self {
            registry: prometheus::Registry::new(),
            port: None,
            prefix: None,
            folder: None,
        }
    }

    /// Create a new Prometheus metrics instance with a prefix/namespace
    pub fn new_with_prefix(
        registry: prometheus::Registry,
        port: Option<u16>,
        prefix: String,
        folder: Option<String>,
    ) -> Self {
        Self {
            registry,
            port,
            prefix: Some(prefix),
            folder,
        }
    }

    fn get_name(&self, name: String) -> String {
        match &self.prefix {
            Some(p) => format!("{p}_{name}"),
            None => name,
        }
    }

    /// Get the Prometheus registry
    pub fn get_registry(&self) -> prometheus::Registry {
        self.registry.clone()
    }

    /// Get the port the metrics server is running on
    pub fn get_port(&self) -> Option<u16> {
        self.port
    }

    /// Get the folder where the metrics are stored
    pub fn get_folder(&self) -> Option<String> {
        self.folder.clone()
    }

    /// Get the prefix/namespace for the metrics
    pub fn get_prefix(&self) -> Option<String> {
        self.prefix.clone()
    }

    /// Gather the metrics and return them as a string
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
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let counter_vec = prometheus::CounterVec::new(
            prometheus::Opts::new(formatted_name, format!("Counter family for {name}")),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(counter_vec.clone())).unwrap();
        Box::new(PrometheusCounterFamily::new(counter_vec))
    }

    fn gauge_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::GaugeFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let gauge_vec = prometheus::GaugeVec::new(
            prometheus::Opts::new(formatted_name, format!("Gauge family for {name}")),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(gauge_vec.clone())).unwrap();
        Box::new(PrometheusGaugeFamily::new(gauge_vec))
    }

    fn histogram_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::HistogramFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let histogram_vec = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(formatted_name, format!("Histogram family for {name}")).buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 7.5, 10.0]),
            &vals,
        ).unwrap();
        self.registry.register(Box::new(histogram_vec.clone())).unwrap();
        Box::new(PrometheusHistogramFamily::new(histogram_vec)) 
    }

    fn text_family(&self, name: String, labels: Vec<String>) -> Box<dyn hsmetrics::TextFamily> {
        let formatted_name = self.get_name(name.clone());
        let vals: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
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
        Box::new(PrometheusMetrics::new_with_prefix(self.registry.clone(), self.port.clone(), prefix, self.folder.clone()))
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
