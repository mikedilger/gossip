/// A queue of up to three status messages for the UI, generally
/// representing errors that occurred in disconnected backend processes.
pub struct StatusQueue {
    head: usize,
    messages: [String; 5],
}

impl Default for StatusQueue {
    fn default() -> StatusQueue {
        StatusQueue {
            head: 0,
            messages: ["".to_owned(), "".to_owned(), "".to_owned(), "".to_owned(), "".to_owned()],
        }
    }
}

impl StatusQueue {
    pub fn new(initial: String) -> StatusQueue {
        let mut sq: StatusQueue = Default::default();
        sq.write(initial);
        sq
    }

    pub fn read_all(&self) -> [String; 5] {
        [
            self.messages[self.head].clone(),
            self.messages[(self.head + 1) % 5].clone(),
            self.messages[(self.head + 2) % 5].clone(),
            self.messages[(self.head + 3) % 5].clone(),
            self.messages[(self.head + 4) % 5].clone(),
        ]
    }

    pub fn read_last(&self) -> String {
        self.messages[self.head].clone()
    }

    pub fn write(&mut self, message: String) {
        self.head = (self.head + 4) % 5; // like -1, but modular safe
        self.messages[self.head] = message;
    }

    pub fn dismiss(&mut self, offset: usize) {
        self.messages[(self.head + offset) % 5] = "".to_owned();
    }
}
