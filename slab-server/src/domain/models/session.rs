#[derive(Debug, Clone)]
pub struct CreateSessionCommand {
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionView {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SessionMessageView {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
