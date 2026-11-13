use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::OnceLock,
};

static FAILURE_LOG: OnceLock<PathBuf> = OnceLock::new();
pub fn set_failure_log(path: PathBuf) {
    let _ = FAILURE_LOG.set(path);
}
/// Fatal paths cannot rely on the tracing writer or a GUI-subsystem console.
/// Never include panic payloads: provider/library errors can contain credentials.
pub fn record_failure(detail: &str) {
    let entry = format!(
        "{} pid={} {detail}\n",
        chrono::Utc::now().to_rfc3339(),
        std::process::id()
    );
    if let Some(path) = FAILURE_LOG.get() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(entry.as_bytes());
            let _ = file.sync_data();
        }
    }
    eprintln!("{entry}");
}
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        record_failure(&format!(
            "fatal Rust panic at {location}; payload omitted; backtrace:\n{}",
            std::backtrace::Backtrace::force_capture()
        ));
        std::process::exit(1);
    }));
}

/// Runs on tracing-appender's single background writer, never the UI thread.
pub struct RotatingLog {
    path: PathBuf,
    file: File,
    bytes: u64,
}
impl RotatingLog {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes = file.metadata()?.len();
        Ok(Self { path, file, bytes })
    }
}
impl Write for RotatingLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.bytes + buf.len() as u64 > 2 * 1024 * 1024 {
            self.file.flush()?;
            // Windows allows rename of an open append handle, but rotating by copying
            // and truncating the same handle also works with Notepad holding the file.
            let old = self.path.with_extension("log.1");
            let older = self.path.with_extension("log.2");
            if old.exists() {
                let _ = std::fs::copy(&old, older);
            }
            std::fs::copy(&self.path, old)?;
            self.file.set_len(0)?;
            self.bytes = 0;
        }
        let n = self.file.write(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panic_probe() {
        if let Some(path) = std::env::var_os("OPENTOKEN_TEST_PANIC_LOG") {
            set_failure_log(path.into());
            install_panic_hook();
            panic!("PRIVATE_PANIC_PAYLOAD_MUST_NOT_BE_LOGGED");
        }
    }
    #[test]
    fn fatal_panic_is_durable_nonzero_and_does_not_log_the_payload() {
        let path = std::env::temp_dir().join(format!("opentoken-panic-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "logging::tests::panic_probe", "--nocapture"])
            .env("OPENTOKEN_TEST_PANIC_LOG", &path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let log = std::fs::read_to_string(&path).unwrap();
        assert!(log.contains("fatal Rust panic at"));
        assert!(log.contains("logging.rs:"));
        assert!(log.contains("backtrace:"));
        assert!(!log.contains("PRIVATE_PANIC_PAYLOAD"));
        std::fs::remove_file(path).unwrap();
    }
}
