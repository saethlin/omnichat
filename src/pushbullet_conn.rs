use conn::{Conn, Event};
use failure::Error;
use std::sync::mpsc::SyncSender;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
enum Message {
    Push { targets: Vec<String>, push: Push },
    Nop,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
enum Push {
    SmsChanged {
        source_device_iden: String,
        notifications: Vec<Notification>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct Notification {
    thread_id: String,
    title: String,
    body: String,
    timestamp: u64,
}

pub struct PushbulletConn {
    _token: String,
    _sender: SyncSender<Event>,
    channels: Vec<String>,
}

impl PushbulletConn {
    pub fn new(token: String, sender: SyncSender<Event>) -> Result<Box<PushbulletConn>, Error> {
        /*
        let client = reqwest::Client::new();
        let mut header = reqwest::header::Headers::new();

        header.set_raw("Access-Token", token.clone());
        let res = client
            .get("https://api.pushbullet.com/v2/users/me")
            .headers(header)
            .send()?
            .text()?;
        sender.send(Event::Error(format!("{}", res))).unwrap();
        */
        let thread_sender = sender.clone();
        let url = format!("wss://stream.pushbullet.com/websocket/{}", token);
        let mut websocket = ::websocket::ClientBuilder::new(&url)?.connect_secure(None)?;
        ::std::thread::spawn(move || {
            use websocket::OwnedMessage::Text;
            while let Ok(Text(message_text)) = websocket.recv_message() {
                if let Ok(Message::Push {
                    push: Push::SmsChanged { notifications, .. },
                    ..
                }) = ::serde_json::from_str::<Message>(&message_text)
                {
                    for notification in notifications {
                        thread_sender
                            .send(Event::Error(format!(
                                "{}: {}",
                                notification.title, notification.body
                            ))).unwrap();
                    }
                }
            }
        });
        Ok(Box::new(Self {
            _token: token,
            _sender: sender,
            channels: vec!["test".to_string()],
        }))
    }
}

impl Conn for PushbulletConn {
    fn name(&self) -> &str {
        "Pushbullet"
    }

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channels.iter().map(|s| s.as_str()))
    }
}
