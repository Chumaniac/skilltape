#[derive(Debug, Clone)]
pub struct TapeIdGenerator {
    prefix: u64,
    counter: u64,
}

impl TapeIdGenerator {
    pub fn new(prefix: u64) -> Self {
        Self { prefix, counter: 0 }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> String {
        let id = format!("tape_{:020}-{:020}", self.prefix, self.counter);
        self.counter += 1;
        id
    }
}
