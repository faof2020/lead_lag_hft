use bkbase::models::Asset;
use redis::Connection;
use serde::Deserialize;
use crate::domains::common::Ticker;
use crate::utils::get_period_ms;
use crate::utils::redis_util::read_redis_delay;

#[derive(Deserialize, Debug, Clone)]
pub struct DelayEmaConfig {
    pub period: String,
    pub intval: u64,
}

#[derive(Debug, Clone)]
pub struct DelayEma {
    pub period: String,
    intval: u64,
    decay: f64,
    alpha: f64,
    last_update_ms: u64,
    pub delay: f64,
    pub init: bool,
}

impl DelayEma {
    pub fn new(config: &DelayEmaConfig, asset: &Asset, redis: Option<&mut Connection>) -> Self {
        let period_ms = get_period_ms(&config.period);
        let length = period_ms / config.intval;
        let decay = (length - 1) as f64 / (length + 1) as f64;
        let alpha = 2f64 / (length + 1) as f64;
        let mut init = false;
        let delay = if redis.is_none() {
            0.0
        } else {
            match read_redis_delay(asset, &config.period, redis.unwrap()) {
                Some(spread) => {
                    init = true;
                    spread
                },
                None => 0.0,
            }
        };
        DelayEma {
            period: config.period.clone(),
            intval: config.intval,
            decay,
            alpha,
            last_update_ms: 0,
            delay,
            init,
        }
    }

    pub fn update(&mut self, ticker: &Ticker, ts: u64) {
        let delay = ticker.get_delay();
        if self.last_update_ms + self.intval <= ts {
            if self.last_update_ms == 0 && !self.init {
                self.delay = delay as f64;
            } else {
                self.delay = self.delay * self.decay + self.alpha * delay as f64;
            }
            self.last_update_ms = ts;
        }
    }

}