pub const ID_LENGTH: usize = 9;

macro_rules! make_id {
    ($name:ident, $($firstchar:expr),+) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            len: u8,
            buf: [u8; ID_LENGTH],
        }

        impl $name {
            #[inline]
            pub fn as_str(&self) -> &str {
                ::std::str::from_utf8(&self.buf[..self.len as usize]).unwrap()
            }
        }

        // TODO: This needs to eventually be TryFrom
        impl<'a> From<&'a str> for $name {
            #[inline]
            fn from(input: &'a str) -> Self {
                assert!(input.len() <= ID_LENGTH);
                match input.as_bytes().get(0) {
                   $(|Some($firstchar))* => {
                        let mut output = Self {
                            len: input.len() as u8,
                            buf: [0; ID_LENGTH],
                        };
                        output.buf[..input.len()].copy_from_slice(&input.as_bytes());
                        output
                   }
                   _ => {
                       panic!(concat!("Invalid start character for ", stringify!($name)));
                   }

                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct IdVisitor;

                impl<'de> ::serde::de::Visitor<'de> for IdVisitor {
                    type Value = $name;

                    #[inline]
                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        formatter.write_str(&format!("a {}-byte str", ID_LENGTH))
                    }

                    #[inline]
                    fn visit_str<E>(self, input: &str) -> Result<$name, E>
                    where
                        E: ::serde::de::Error,
                    {
                        if input.len() > ID_LENGTH || input.len() == 0 {
                            Err(E::custom(format!(
                                "{} must be a 1-{} byte string starting with one of {:?}, found {:?}",
                                stringify!($name),
                                ID_LENGTH,
                                [$($firstchar as char,)*],
                                input
                            )))
                        } else {
                            match input.as_bytes().get(0) {
                                $(|Some($firstchar))* => {
                                    let mut output = $name {
                                        len: input.len() as u8,
                                        buf: [0; ID_LENGTH],
                                    };
                                    output.buf[..input.len()].copy_from_slice(&input.as_bytes());
                                    Ok(output)

                                }
                                _ => {
                                    Err(E::custom(format!(
                                        "{} must be a {}-byte string starting with one of {:?}, found {:?}",
                                        stringify!($name),
                                        ID_LENGTH,
                                        [$($firstchar as char,)*],
                                        input
                                    )))
                                }
                            }
                        }
                    }
                }

                deserializer.deserialize_str(IdVisitor)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_str(::std::str::from_utf8(&self.buf[..self.len as usize]).unwrap())
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(
                    f,
                    "{}",
                    ::std::str::from_utf8(&self.buf[..self.len as usize]).unwrap()
                ).map(|_| ())
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{}", self)
            }
        }
    };
}

make_id!(BotId, b'B');
make_id!(UserId, b'U', b'W');
make_id!(ChannelId, b'C');
make_id!(GroupId, b'G');
make_id!(DmId, b'D');
make_id!(TeamId, b'T');
make_id!(AppId, b'A');
make_id!(FileId, b'F');
make_id!(UsergroupId, b'S');
make_id!(ReminderId, b'R');

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ConversationId {
    Channel(ChannelId),
    Group(GroupId),
    DirectMessage(DmId),
}

impl ConversationId {
    pub fn as_str(&self) -> &str {
        match &self {
            ConversationId::Channel(id) => id.as_str(),
            ConversationId::Group(id) => id.as_str(),
            ConversationId::DirectMessage(id) => id.as_str(),
        }
    }
}

impl ::std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match &self {
            ConversationId::Channel(c) => write!(f, "{}", c),
            ConversationId::Group(g) => write!(f, "{}", g),
            ConversationId::DirectMessage(d) => write!(f, "{}", d),
        }
    }
}

impl ::std::convert::From<ChannelId> for ConversationId {
    fn from(id: ChannelId) -> Self {
        ConversationId::Channel(id)
    }
}

impl ::std::convert::From<GroupId> for ConversationId {
    fn from(id: GroupId) -> Self {
        ConversationId::Group(id)
    }
}

impl ::std::convert::From<DmId> for ConversationId {
    fn from(id: DmId) -> Self {
        ConversationId::DirectMessage(id)
    }
}
