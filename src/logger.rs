use crate::conn::ConnEvent;
use log::{Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

use futures::channel::mpsc::UnboundedSender;
use futures::sink::SinkExt;

pub struct Logger {
    file_output: Mutex<File>,
    sender: UnboundedSender<ConnEvent>,
}

impl Logger {
    pub fn new(sender: UnboundedSender<ConnEvent>) -> Self {
        let log_path = dirs::home_dir()
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
        let mut send = self.sender.clone();
        tokio::spawn(async move { send.send(ConnEvent::Error(message)).await });
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file_output.lock() {
            let _ = f.flush();
        }
    }
}
