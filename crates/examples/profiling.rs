use pprof::{protos::Message, ProfilerGuard};

#[cfg(feature = "jemalloc")]
use jemalloc_pprof::activate_jemalloc_profiling;

#[cfg(feature = "jemalloc")]
use tokio::io::AsyncWriteExt;

#[cfg(feature = "jemalloc")]
use tikv_jemallocator::Jemalloc;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
/// The configuration for jemalloc.
/// This configuration enables profiling and passed to jemalloc through the `malloc_conf` symbol.
/// Only used if the `jemalloc` feature is enabled.
#[allow(non_upper_case_globals)]
#[export_name = "malloc_conf"]
pub static malloc_conf: &[u8] = b"prof:true,prof_active:true,lg_prof_sample:19\0";

/// Start the pprof profiler with the given directory.
/// The profiler will write the profile to the directory every `pprof_interval` seconds.
#[allow(clippy::missing_panics_doc)]
pub async fn start_profiler(pprof_dir: &str, pprof_interval: u32) {
    match std::fs::create_dir_all(pprof_dir) {
        Ok(()) => tracing::info!("Successfully created pprof dir: {pprof_dir}"),
        Err(e) => {
            tracing::error!("Failed to create pprof dir: {pprof_dir}: {e}");
            return;
        }
    }
    
    #[cfg(feature = "jemalloc")]
    activate_jemalloc_profiling().await;

    let pprof_frequency = 1000;
    let guard = match pprof::ProfilerGuardBuilder::default()
        .frequency(pprof_frequency)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build() {
        Ok(guard) => Arc::new(guard),
        Err(e) => {
            tracing::error!("Failed to create pprof guard: {e}");
            return;
        }
    };

    let mut interval = tokio::time::interval(Duration::from_secs(pprof_interval.into()));
    let pprof_dir_cpu = pprof_dir.to_string();
    #[cfg(feature = "jemalloc")]
    let pprof_dir_jemalloc = pprof_dir.to_string();
    loop {
        interval.tick().await;
        #[cfg(feature = "jemalloc")]
        tokio::spawn(dump_jemalloc(pprof_dir_jemalloc.clone()));
        let guard_cloned = Arc::clone(&guard);
        dump_cpu(&guard_cloned, &pprof_dir_cpu);
    }
}

/// Collect the jemalloc heap profile and write it to the given directory.
/// The profile will be written to a file with the format `jemalloc_{timestamp}.heap.pb.gz`.
#[cfg(feature = "jemalloc")]
#[allow(clippy::missing_panics_doc)]
async fn dump_jemalloc(pprof_dir: String) {
    let file_prefix = "jemalloc";
    let timestamp = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let file_path = format!("{pprof_dir}/{file_prefix}_{timestamp}.heap.pb.gz");

    let mut out_file = match tokio::fs::File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to create jemalloc file: {file_path}: {e}");
            return;
        }
    };

    let profile = get_jemalloc_prof().await; 
    if profile.is_empty() {
        return;
    }
    match out_file.write_all(&profile).await {
        Ok(()) => {
            tracing::info!("Successfully wrote pprof heap: {file_path}");
        }
        Err(e) => {
            tracing::error!("Failed to write pprof heap: {e}");
            return;
        }
    }
}

/// Get the jemalloc heap profile.
/// Returns an empty vector if the profile cannot be obtained.
#[cfg(feature = "jemalloc")]
async fn get_jemalloc_prof() -> Vec<u8> {
    let mut jemalloc_ctrl = jemalloc_pprof::PROF_CTL.as_ref().unwrap().lock().await;
    match jemalloc_ctrl.dump_pprof() {
        Ok(profile) => profile,
        Err(e) => {
            tracing::error!("Failed to get pprof profile: {e}");
            Vec::new()
        }
    }
}

/// Collect the CPU profile and write it to the given directory.
/// The profile will be written to a file with the format `profile_{timestamp}.pb`.
#[allow(clippy::missing_panics_doc)]
fn dump_cpu(guard: &Arc<ProfilerGuard<'static>>, pprof_dir: &str) {
    match guard.report().build() {
        Ok(report) => {
            let file_prefix = "profile";
            let timestamp = time::SystemTime::now()
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let file_path = format!("{pprof_dir}/{file_prefix}_{timestamp}.pb");

            let mut out_file = match File::create(&file_path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("Failed to create pprof file: {file_path}: {e}");
                    return;                }
            };
            let profile = match report.pprof() {
                Ok(profile) => profile,
                Err(e) => {
                    tracing::error!("Failed to get pprof profile: {e}");
                    return;
                }
            };

            let mut content = Vec::new();
            match profile.write_to_vec(&mut content) {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!("Failed to encode pprof profile: {e}");
                    return;
                }
            };
            match out_file.write_all(&content) {
                Ok(()) => {
                    tracing::info!("Successfully wrote pprof profile: {file_path}");
                }
                Err(e) => {
                    tracing::error!("Failed to write pprof profile: {e}");
                }
            }
        }
        Err(_) => {
            tracing::error!("Failed to get pprof report");
        }
    };
}