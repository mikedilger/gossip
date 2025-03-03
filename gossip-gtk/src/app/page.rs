
#[derive(Debug, Clone)]
pub enum Page {
    LoginPage,
    WaitForMigration,
    WaitForPruning(String),
    Tbd,
}
