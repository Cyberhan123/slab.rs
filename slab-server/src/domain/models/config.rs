use crate::api::v1::config::schema::SetConfigBody;

#[derive(Debug, Clone)]
pub struct ConfigEntryView {
    pub key: String,
    pub value: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SetConfigValueCommand {
    pub name: Option<String>,
    pub value: String,
}

impl From<SetConfigBody> for SetConfigValueCommand {
    fn from(body: SetConfigBody) -> Self {
        Self {
            name: body.name,
            value: body.value,
        }
    }
}
