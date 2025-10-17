use bkbase::models::Asset;
use redis::Connection;
use serde::Deserialize;
use crate::domains::common::Ticker;
use crate::utils::get_period_ms;
use crate::utils::redis_util::read_redis_spread;

#[derive(Deserialize, Debug, Clone)]
pub struct SpreadEmaConfig {
    pub period: String,
    pub intval: u64,
}

#[derive(Debug, Clone)]
pub struct SpreadEma {
    pub period: String,
    intval: u64,
    decay: f64,
    alpha: f64,
    last_update_ms: u64,
    pub spread: f64,
    pub init: bool,
}

impl SpreadEma {
    pub fn new(config: &SpreadEmaConfig, asset: &Asset, redis: Option<&mut Connection>) -> Self {
        let period_ms = get_period_ms(&config.period);
        let length = period_ms / config.intval;
        let decay = (length - 1) as f64 / (length + 1) as f64;
        let alpha = 2f64 / (length + 1) as f64;
        let mut init = false;
        let spread = if redis.is_none() {
            0.0
        } else {
            match read_redis_spread(asset, &config.period, redis.unwrap()) {
                Some(spread) => {
                    init = true;
                    spread
                },
                None => 0.0,
            }
        };
        SpreadEma {
            period: config.period.clone(),
            intval: config.intval,
            decay,
            alpha,
            last_update_ms: 0,
            spread,
            init,
        }
    }

    pub fn update(&mut self, ticker: &Ticker, ts: u64) {
        let spread = ticker.spread();
        if self.last_update_ms + self.intval <= ts {
            if self.last_update_ms == 0 && !self.init {
                self.spread = spread;
            } else {
                self.spread = self.spread * self.decay + spread * self.alpha;
            }
            self.last_update_ms = ts;
        }
    }

}