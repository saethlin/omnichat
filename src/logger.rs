use conn::Event;
use log::{Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

pub struct Logger {
    file_output: Mutex<File>,
    sender: SyncSender<Event>,
}

impl Logger {
    pub fn new(sender: SyncSender<Event>) -> Self {
        let log_path = ::dirs::home_dir().unwrap().join(".omnichat_log");
        Logger {
            file_output: Mutex::new(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .unwrap(),
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
            record.file().unwrap(),
            record.line().unwrap()
        );
        let mut file_handle = self.file_output.lock().unwrap();
        let _ = write!(file_handle, "{}\n", message);
        file_handle.flush().unwrap();
        self.sender.send(Event::Error(message)).unwrap();
    }
    fn flush(&self) {
        self.file_output.lock().unwrap().flush().unwrap();
    }
}
