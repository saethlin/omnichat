use std::sync::mpsc::Sender;
use std::thread;
use std::sync::{Arc, RwLock};
use conn::{Conn, Event, Message};
use conn::ConnError::IrcError;
use failure::Error;
use irc::client::IrcClient;
use irc::client::Client;
use futures::Stream;
use futures::Future;

pub struct IrcConn {
    sender: Sender<Event>,
    name: String,
    channel_names: Vec<String>,
    client: IrcClient,
}

impl IrcConn {
    pub fn new(
        nickname: String,
        server: String,
        port: u16,
        sender: Sender<Event>,
    ) -> Result<Box<Conn>, Error> {
        let mut config = ::irc::client::data::Config::default();
        config.nickname = Some(nickname);
        config.server = Some(server.clone());
        config.port = Some(port);
        let client = IrcClient::from_config(config)?;

        let stream = client.stream();
        let server_name = server.clone();
        let thread_sender = sender.clone();
        thread::spawn(move || {
            use irc;
            stream
                .for_each(|ev| {
                    if let irc::proto::Message {
                        command: irc::proto::Command::PRIVMSG(source, contents),
                        ..
                    } = ev
                    {
                        thread_sender
                            .send(Event::Message(Message {
                                sender: source,
                                contents: contents,
                                channel: "test".to_string(),
                                is_mention: false,
                                server: server_name.clone(),
                            }))
                            .unwrap();
                    }
                    Ok(())
                })
                .wait()
                .unwrap();
        });

        return Ok(Box::new(IrcConn {
            sender: sender,
            name: server,
            channel_names: vec!["test".to_string()],
            client: client,
        }));
    }
}

impl Conn for IrcConn {
    fn send_channel_message(&mut self, channel: &str, contents: &str) {}

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_str()))
    }

    fn name(&self) -> &str {
        &self.name
    }
}
