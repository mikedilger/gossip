use super::Page;

#[derive(Debug, Clone)]
pub enum Message {
    SetPage(Page)
}
