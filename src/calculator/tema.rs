use redis::Connection;
use crate::utils::get_period_ms;
use crate::utils::redis_util::read_redis_key;

pub struct TemaMs {
    pub last_ts: f64,
    pub val: f64,
    pub tau: String,
    pub tau_value: f64,
}

impl TemaMs {
    pub fn new(
        tau: &str,
        redis: Option<&mut Connection>,
        key: Option<&str>,
        hmap_key: Option<&str>,
    ) -> Self {
        let tau_value = get_period_ms(tau) as f64;
        let (val, last_ts) = if redis.is_some() && key.is_some() && hmap_key.is_some() && hmap_key.is_some() {
            let redis_client = redis.unwrap();
            (read_redis_key(
                &format!("{}_{}", key.unwrap(), "value"),
                hmap_key.unwrap(), redis_client
            ).unwrap_or(0.0),
             read_redis_key(
                 &format!("{}_{}", key.unwrap(), "last_ts"),
                 hmap_key.unwrap(), redis_client
             ).unwrap_or(0.0))
        } else {
            (0.0, 0.0)
        };
        TemaMs {
            last_ts,
            val,
            tau: tau.to_string(),
            tau_value,
        }
    }

    pub fn update(
        &mut self, new_val: f64, ts: u64,
    ) {
        if self.last_ts < 1.0 {
            self.val = new_val;
        } else {
            let dt = ts as f64 - self.last_ts;
            let exp = -dt / self.tau_value;
            self.val = self.val * f64::exp(exp) + new_val / self.tau_value;
        }
        self.last_ts = ts as f64;
    }

    pub fn is_ready(&self) -> bool {
        self.last_ts > 1.0
    }
}