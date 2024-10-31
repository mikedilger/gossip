use std::ops::Add;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum RelayTestResult {
    #[default]
    Unknown,
    Pass,
    Fail(String),
}

impl Add for RelayTestResult {
    type Output = Self;

    fn add(self, other: RelayTestResult) -> RelayTestResult {
        match (self, other) {
            (RelayTestResult::Fail(s), _) => RelayTestResult::Fail(s),
            (_, RelayTestResult::Fail(s)) => RelayTestResult::Fail(s),
            (RelayTestResult::Unknown, _) => RelayTestResult::Unknown,
            (_, RelayTestResult::Unknown) => RelayTestResult::Unknown,
            _ => RelayTestResult::Pass,
        }
    }
}

impl RelayTestResult {
    pub fn tick(&self) -> char {
        match *self {
            RelayTestResult::Unknown => '❓',
            RelayTestResult::Pass => '✅',
            RelayTestResult::Fail(_) => '❌',
        }
    }

    pub fn hover(&self) -> Option<&str> {
        match *self {
            RelayTestResult::Unknown => None,
            RelayTestResult::Pass => None,
            RelayTestResult::Fail(ref s) => Some(s),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RelayTestResults {
    pub outbox: RelayTestResult,
    pub inbox: RelayTestResult,
    pub public_inbox: RelayTestResult,
    pub test_failed: bool,
}

impl RelayTestResults {
    pub fn dm(&self) -> RelayTestResult {
        self.inbox.clone() + self.outbox.clone()
    }

    pub fn fail() -> RelayTestResults {
        RelayTestResults {
            test_failed: true,
            ..Default::default()
        }
    }
}
