use bkbase::models::Asset;
use redis::Connection;
use serde::Deserialize;
use crate::domains::common::Ticker;
use crate::utils::get_period_ms;
use crate::utils::redis_util::read_redis_offset;

#[derive(Deserialize, Debug, Clone)]
pub struct OffsetEmaConfig {
    pub period: String,
    pub intval: u64,
}

#[derive(Debug, Clone)]
pub struct OffsetEma {
    pub period: String,
    intval: u64,
    decay: f64,
    alpha: f64,
    last_update_ms: u64,
    // A2B指lag的A对lead的B，对应就是A/B-1
    pub b2b: f64,
    pub b2a: f64,
    pub a2b: f64,
    pub a2a: f64,
    pub init: bool,
}

impl OffsetEma {
    pub fn new(config: &OffsetEmaConfig, asset: &Asset, redis: Option<&mut Connection>) -> Self {
        let period_ms = get_period_ms(&config.period);
        let length = period_ms / config.intval;
        let decay = (length - 1) as f64 / (length + 1) as f64;
        let alpha = 2f64 / (length + 1) as f64;
        let mut init = false;
        let (b2b, b2a, a2b, a2a) = if redis.is_none() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            match read_redis_offset(asset, &config.period, redis.unwrap()) {
                Some(v) => {
                    init = true;
                    (v[0], v[1], v[2], v[3])
                },
                None => (0.0, 0.0, 0.0, 0.0),
            }
        };
        OffsetEma {
            period: config.period.clone(),
            intval: config.intval,
            decay,
            alpha,
            last_update_ms: 0,
            b2b,
            b2a,
            a2b,
            a2a,
            init,
        }
    }

    pub fn update(&mut self, lead: &Ticker, lag: &Ticker, ts: u64) {
        let b2b = lag.bp1 / lead.bp1 - 1.0;
        let b2a = lag.bp1 / lead.ap1 - 1.0;
        let a2b = lag.ap1 / lead.bp1 - 1.0;
        let a2a = lag.ap1 / lead.ap1 - 1.0;
        if self.last_update_ms + self.intval <= ts {
            if self.last_update_ms == 0 && !self.init {
                self.b2b = b2b;
                self.b2a = b2a;
                self.a2b = a2b;
                self.a2a = a2a;
            } else {
                self.b2b = self.b2b * self.decay + self.alpha * b2b;
                self.b2a = self.b2a * self.decay + self.alpha * b2a;
                self.a2b = self.a2b * self.decay + self.alpha * a2b;
                self.a2a = self.a2a * self.decay + self.alpha * a2a;
            }
            self.last_update_ms = ts;
            if !self.init {
                self.init = true;
            }
        }
    }
}