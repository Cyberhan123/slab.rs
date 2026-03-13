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
