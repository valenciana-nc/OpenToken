use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};
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
