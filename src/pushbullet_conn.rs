use conn::{Conn, Event};
use failure::Error;
use reqwest;
use std::sync::mpsc::Sender;

pub struct PushbulletConn {
    token: String,
    sender: Sender<Event>,
    channels: Vec<String>,
}

impl PushbulletConn {
    pub fn new(token: String, sender: Sender<Event>) -> Result<PushbulletConn, Error> {
        let client = reqwest::Client::new();
        let mut header = reqwest::header::Headers::new();
        header.set_raw("Access-Token", token.clone());
        let res = client
            .get("https://api.pushbullet.com/v2/users/me")
            .headers(header)
            .send()?
            .text()?;
        sender.send(Event::Error(format!("{}", res))).unwrap();
        Ok(Self {
            token,
            sender,
            channels: Vec::new(),
        })
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
