use pprof::protos::Message;

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
    let pprof_frequency = 1000;
    let guard = match pprof::ProfilerGuardBuilder::default()
        .frequency(pprof_frequency)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::error!("Failed to create pprof guard: {e}");
            return;
        }
    };

    let file_prefix = "profile";
    let mut interval = tokio::time::interval(Duration::from_secs(pprof_interval.into()));
    loop {
        interval.tick().await;
        match guard.report().build() {
            Ok(report) => {
                let timestamp = time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let file_path = format!("{pprof_dir}/{file_prefix}_{timestamp}.pb");

                let mut file = match File::create(&file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        tracing::error!("Failed to create pprof file: {file_path}: {e}");
                        continue;
                    }
                };
                let profile = match report.pprof() {
                    Ok(profile) => profile,
                    Err(e) => {
                        tracing::error!("Failed to get pprof profile: {e}");
                        continue;
                    }
                };

                let mut content = Vec::new();
                match profile.write_to_vec(&mut content) {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::error!("Failed to encode pprof profile: {e}");
                        continue;
                    }
                };
                match file.write_all(&content) {
                    Ok(()) => {
                        tracing::info!("Successfully wrote pprof profile: {file_path}");
                    }
                    Err(e) => {
                        tracing::error!("Failed to write pprof profile: {e}");
                        continue;
                    }
                }
            }
            Err(_) => {
                tracing::error!("Failed to get pprof report");
            }
        };
    }
}