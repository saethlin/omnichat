use conn::ConnEvent;
use log::{Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

pub struct Logger {
    file_output: Mutex<File>,
    sender: SyncSender<ConnEvent>,
}

impl Logger {
    pub fn new(sender: SyncSender<ConnEvent>) -> Self {
        let log_path = ::dirs::home_dir()
            .expect("You must have a home directory")
            .join(".omnichat_log");

        Logger {
            file_output: Mutex::new(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .expect("Couldn't open the path for the log file at $HOME/.omnichat_log"),
            ),
            sender,
        }
    }
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let message = format!(
            "{}, {}, {}",
            record.args(),
            record.file().unwrap_or("?"),
            record.line().unwrap_or(0)
        );

        if let Ok(mut file_handle) = self.file_output.lock() {
            let _ = writeln!(file_handle, "{}", message);
            let _ = file_handle.flush();
        }
        let _ = self.sender.send(ConnEvent::Error(message));
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file_output.lock() {
            let _ = f.flush();
        }
    }
}
