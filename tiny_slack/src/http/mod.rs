#[derive(Debug, Deserialize)]
pub struct Error {
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Cursor(String); // TODO: Type safety goes here

#[derive(Deserialize)]
pub struct Paging {
    pub count: Option<u32>,
    pub page: Option<u32>,
    pub pages: Option<u32>,
    pub total: Option<u32>,
}

pub mod channels;
pub mod conversations;
pub mod emoji;
pub mod groups;
pub mod im;
pub mod reactions;
pub mod rtm;
pub mod users;
