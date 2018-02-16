use std::sync::mpsc::Sender;
use std::thread;
//use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, Event, Message};
use conn::ConnError::DiscordError;
use failure::Error;

use discord;
use discord::model::PublicChannel;

pub struct DiscordConn {
    sender: Sender<Event>,
    name: String,
    channels: Vec<PublicChannel>,
}

impl DiscordConn {
    pub fn new(
        token: String,
        server_name: String,
        sender: Sender<Event>,
    ) -> Result<Box<Conn>, Error> {
        use discord::model::PossibleServer::Online;

        let dis = discord::Discord::from_user_token(&token)?;
        let (connection, info) = dis.connect()?;

        let server = info.servers
            .iter()
            .filter_map(|s| {
                if let &Online(ref server) = s {
                    Some(server)
                } else {
                    None
                }
            })
            .find(|s| s.name == server_name)
            .ok_or(DiscordError)?;

        // Load message history
        let server = server.clone();
        let channels = server.channels.clone();
        let t_sender = sender.clone();
        let name = server_name.clone();
        thread::spawn(move || {
            for c in &server.channels {
                let messages = dis.get_messages(c.id, discord::GetMessages::MostRecent, None)
                    .unwrap_or_else(|e| {
                        t_sender.send(Event::Error(format!("{}", e)));
                        Vec::new()
                    });
                for m in messages.into_iter() {
                    t_sender
                        .send(Event::HistoryMessage(Message {
                            server: name.clone(),
                            channel: c.name.clone(),
                            sender: m.author.name,
                            contents: m.content,
                        }))
                        .expect("Sender died");
                }
            }
        });

        return Ok(Box::new(DiscordConn {
            sender: sender,
            name: server_name,
            channels: channels,
        }));

        Err(::failure::Error::from(DiscordError))
    }
}

impl Conn for DiscordConn {
    fn send_channel_message(&mut self, channel: &str, contents: &str) {}

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn channels(&self) -> Vec<&String> {
        self.channels.iter().map(|c| &c.name).collect()
    }

    fn autocomplete(&self, _word: &str) -> Option<String> {
        None
    }

    fn name(&self) -> &String {
        &self.name
    }
}
