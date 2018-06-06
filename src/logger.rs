use conn::Event;
use log::{Log, Metadata, Record};
use std::fs::File;
use std::io::Write;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

pub struct Logger {
    file_output: Mutex<File>,
    sender: SyncSender<Event>,
}

impl Logger {
    pub fn new(sender: SyncSender<Event>) -> Self {
        let log_path = ::std::env::home_dir().unwrap().join(".omnichat_log");
        Logger {
            file_output: Mutex::new(File::create(&log_path).unwrap()),
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
        self.file_output
            .lock()
            .unwrap()
            .write_all(message.as_bytes())
            .unwrap();
        self.sender.send(Event::Error(message)).unwrap();
    }
    fn flush(&self) {
        self.file_output.lock().unwrap().flush().unwrap();
    }
}
