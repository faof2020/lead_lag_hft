pub struct TemaMs {
    pub last_ts: u64,
    pub val: f64,
    pub tau: f64,
}

impl TemaMs {
    pub fn new(tau: u64) -> Self {
        TemaMs {
            last_ts: 0,
            val: 0.0,
            tau: tau as f64,
        }
    }

    pub fn update(&mut self, new_val: f64, ts: u64) {
        if self.last_ts == 0 {
            self.val = new_val;
        } else {
            let dt = (ts - self.last_ts) as f64;
            let exp = -dt / self.tau;
            self.val = self.val * f64::exp(exp) + new_val / self.tau;
        }
        self.last_ts = ts;
    }

    pub fn is_ready(&self) -> bool {
        self.last_ts > 0
    }
}