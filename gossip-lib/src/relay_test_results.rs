use std::ops::Add;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RelayTestResult {
    #[default]
    Unknown,
    Pass,
    Fail,
}

impl Add for RelayTestResult {
    type Output = Self;

    fn add(self, other: RelayTestResult) -> RelayTestResult {
        match (self, other) {
            (RelayTestResult::Fail, _) => RelayTestResult::Fail,
            (_, RelayTestResult::Fail) => RelayTestResult::Fail,
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
            RelayTestResult::Fail => '❌',
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RelayTestResults {
    pub outbox: RelayTestResult,
    pub inbox: RelayTestResult,
    pub public_inbox: RelayTestResult,
    pub test_failed: bool,
}

impl RelayTestResults {
    pub fn dm(&self) -> RelayTestResult {
        self.inbox + self.outbox
    }

    pub fn fail() -> RelayTestResults {
        let mut results: RelayTestResults = Default::default();
        results.test_failed = true;
        results
    }
}
