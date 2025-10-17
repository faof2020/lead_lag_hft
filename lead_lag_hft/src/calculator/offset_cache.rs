use std::collections::HashMap;
use bkbase::models::Asset;
use redis::Connection;
use crate::calculator::offset_ema::OffsetEma;
use crate::domains::common::Ticker;
use anyhow::{anyhow, Result};
use crate::offset_taker_strategy::offset_taker_config::OffsetTakerConfig;
use crate::redis_reporter::RedisReporter;
use crate::utils::redis_util::REDIS_OFFSET_KET;

pub struct OffsetCache {
    lead2lag: HashMap<Asset, Asset>,
    lag2offset: HashMap<Asset, HashMap<String, OffsetEma>>,
    lead_max_delay: u64,
    lag_max_delay: u64,
    lead_max_expiration: u64,
    pub init: bool,
}

impl OffsetCache {

    pub fn new() -> Self {
        OffsetCache {
            lead2lag: HashMap::new(),
            lag2offset: HashMap::new(),
            lead_max_delay: 0,
            lag_max_delay: 0,
            lead_max_expiration: 0,
            init: false,
        }
    }

    pub fn init(
        &mut self,
        lead2lag: &HashMap<Asset, Asset>,
        strategy_config: &OffsetTakerConfig,
        mut redis: Option<&mut Connection>,
    ) {
        for (lead, lag) in lead2lag.iter() {
            let mut offset_map = HashMap::new();
            for config in strategy_config.offset_configs.iter() {
                offset_map.insert(config.period.clone(), OffsetEma::new(
                    config, lag, redis.as_deref_mut()
                ));
            }
            self.lag2offset.insert(lag.clone(), offset_map);
            self.lead2lag.insert(lead.clone(), lag.clone());
        }
        self.lead_max_delay = strategy_config.lead_max_delay;
        self.lag_max_delay = strategy_config.lag_max_delay;
        self.lead_max_expiration = strategy_config.lead_max_expiration;
        self.init = true;
    }

    pub fn update(
        &mut self, lead: &Ticker, lag: &Ticker, now_ms: u64,
        mut redis_reporter: Option<&mut RedisReporter>
    ) -> Result<()> {
        if lead.get_delay() > self.lead_max_delay {
            return Err(anyhow!("{:?} tick delay {} ms.", lead.asset, lead.get_delay()));
        }
        if lag.get_delay() > self.lag_max_delay {
            return Err(anyhow!("{:?} tick delay {} ms.", lag.asset, lag.get_delay()));
        }
        if now_ms - lead.receive_ms > self.lead_max_expiration {
            return Err(anyhow!("lead tick expired. expire ms: {}", now_ms - lead.receive_ms));
        }
        if !self.lag2offset.contains_key(&lag.asset) {
            return Err(anyhow!("offset cache not contain lag: {:?}", lag.asset));
        }
        let offset_map = self.lag2offset.get_mut(&lag.asset).unwrap();
        for (period, offset) in offset_map.iter_mut() {
            offset.update(lead, lag, now_ms);
            if redis_reporter.is_some() {
                let reporter = redis_reporter.as_deref_mut().unwrap();
                reporter.record(
                    REDIS_OFFSET_KET,
                    &format!("{:?}_{}_{}", lag.asset, period, "bid2bid"),
                    offset.b2b, now_ms
                );
                reporter.record(
                    REDIS_OFFSET_KET,
                    &format!("{:?}_{}_{}", lag.asset, period, "bid2ask"),
                    offset.b2a, now_ms
                );
                reporter.record(
                    REDIS_OFFSET_KET,
                    &format!("{:?}_{}_{}", lag.asset, period, "ask2bid"),
                    offset.a2b, now_ms
                );
                reporter.record(
                    REDIS_OFFSET_KET,
                    &format!("{:?}_{}_{}", lag.asset, period, "ask2ask"),
                    offset.a2a, now_ms
                );
            }
        }
        Ok(())
    }

    pub fn get_offset(&self, asset: &Asset, period: &str) -> Option<&OffsetEma> {
        let mut lag_asset = asset;
        if !self.lag2offset.contains_key(asset) {
            if self.lead2lag.contains_key(asset) {
                lag_asset = self.lead2lag.get(asset).unwrap();
            } else {
                return None;
            }
        }
        let offset_map = self.lag2offset.get(lag_asset).unwrap();
        offset_map.get(period)
    }

    pub fn get_all_offset(&self, asset: &Asset) -> Option<Vec<&OffsetEma>> {
        let mut lag_asset = asset;
        if !self.lag2offset.contains_key(asset) {
            if self.lead2lag.contains_key(asset) {
                lag_asset = self.lead2lag.get(asset).unwrap();
            } else {
                return None;
            }
        }
        let offset_map = self.lag2offset.get(lag_asset).unwrap();
        let mut ret = vec![];
        for (_, o) in offset_map.iter() {
            ret.push(o);
        }
        Some(ret)
    }

}