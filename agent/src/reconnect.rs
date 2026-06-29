use std::time::Duration;

pub struct ReconnectBackoff {
    current_delay: Duration,
    max_delay: Duration,
    factor: f64,
}

impl ReconnectBackoff {
    pub fn new() -> Self {
        ReconnectBackoff {
            current_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            factor: 2.0,
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current_delay;
        let next_secs = (self.current_delay.as_secs_f64() * self.factor).min(self.max_delay.as_secs_f64());
        self.current_delay = Duration::from_secs_f64(next_secs);
        delay
    }

    pub fn reset(&mut self) {
        self.current_delay = Duration::from_secs(1);
    }
}
