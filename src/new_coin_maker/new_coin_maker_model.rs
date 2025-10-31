use std::str::FromStr;
use bkbase::models::{Asset, TradeData};
use redis::Connection;
use crate::calculator::tema::TemaMs;
use crate::new_coin_maker::new_coin_maker_config::TradeAssetConfig;
use crate::redis_reporter::RedisReporter;

pub const REDIS_KET: &str = "new_coin_maker";

pub struct NewCoinMakerModel {
    pub asset: Asset,
    pub value_tema: TemaMs,
    pub volume_tema: TemaMs,
    pub value_diff_tema: TemaMs,
    pub volume_diff_tema: TemaMs,
    value_key: String,
    volume_key: String,
    value_diff_key: String,
    volume_diff_key: String,
    sigma_multi: f64,
    sigma_min_bps: f64,
}

impl NewCoinMakerModel {
    pub fn new(config: &TradeAssetConfig, mut redis: Option<&mut Connection>) -> Self {
        let asset = Asset::from_str(&config.asset).unwrap();
        let value_key = format!("{:?}_{}_value", asset, config.tau_p);
        let volume_key = format!("{:?}_{}_volume", asset, config.tau_p);
        let value_diff_key = format!("{:?}_{}_value_diff", asset, config.tau_o);
        let volume_diff_key = format!("{:?}_{}_volume_diff", asset, config.tau_o);

        let value_tema = if redis.is_some() {
            TemaMs::new(
                &config.tau_p, redis.as_deref_mut(),
                Some(&value_key), Some(REDIS_KET)
            )
        } else {
            TemaMs::new(&config.tau_p, None, None, None)
        };
        let volume_tema = if redis.is_some() {
            TemaMs::new(
                &config.tau_p, redis.as_deref_mut(),
                Some(&volume_key), Some(REDIS_KET)
            )
        } else {
            TemaMs::new(&config.tau_p, None, None, None)
        };
        let value_diff_tema = if redis.is_some() {
            TemaMs::new(
                &config.tau_o, redis.as_deref_mut(),
                Some(&value_diff_key), Some(REDIS_KET)
            )
        } else {
            TemaMs::new(&config.tau_o, None, None, None)
        };
        let volume_diff_tema = if redis.is_some() {
            TemaMs::new(
                &config.tau_o, redis.as_deref_mut(),
                Some(&volume_diff_key), Some(REDIS_KET)
            )
        } else {
            TemaMs::new(&config.tau_o, None, None, None)
        };
        NewCoinMakerModel {
            asset,
            value_tema,
            volume_tema,
            value_diff_tema,
            volume_diff_tema,
            value_key,
            volume_key,
            value_diff_key,
            volume_diff_key,
            sigma_multi: config.sigma_multi,
            sigma_min_bps: config.sigma_min_bps,
        }
    }

    pub fn update(
        &mut self, trade: &TradeData,
        mut redis_reporter: Option<&mut RedisReporter>
    ) {
        let value = trade.price * trade.volume.abs();
        let volume = trade.volume.abs();
        self.value_tema.update(value, trade.transaction_time);
        self.volume_tema.update(volume, trade.transaction_time);
        let price_tema = self.value_tema.val / self.volume_tema.val;
        let diff = price_tema * f64::ln(trade.price / price_tema);
        let value_diff = diff.abs() * trade.volume.abs();
        self.value_diff_tema.update(value_diff, trade.transaction_time);
        self.volume_diff_tema.update(volume, trade.transaction_time);
        if redis_reporter.is_some() {
            let reporter = redis_reporter.as_deref_mut().unwrap();
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.value_key, "value"),
                self.value_tema.val,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.value_key, "last_ts"),
                self.value_tema.last_ts,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.volume_key, "value"),
                self.volume_tema.val,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.volume_key, "last_ts"),
                self.volume_tema.last_ts,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.value_diff_key, "value"),
                self.value_diff_tema.val,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.value_diff_key, "last_ts"),
                self.value_diff_tema.last_ts,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.volume_diff_key, "value"),
                self.volume_diff_tema.val,
                trade.transaction_time
            );
            reporter.record(
                REDIS_KET,
                &format!("{}_{}", self.volume_diff_key, "last_ts"),
                self.volume_diff_tema.last_ts,
                trade.transaction_time
            );
        }
    }

    pub fn is_ready(&self) -> bool {
        self.value_tema.is_ready()
    }

    pub fn get_tema_price(&self) -> f64 {
        if !self.value_tema.is_ready() || !self.volume_tema.is_ready() {
            panic!("{:?} model is not ready", self.asset);
        }
        self.value_tema.val / self.volume_tema.val
    }

    pub fn get_tema_sigma(&self) -> f64 {
        if !self.value_diff_tema.is_ready() || !self.volume_diff_tema.is_ready() {
            panic!("{:?} model is not ready", self.asset);
        }
        self.value_diff_tema.val / self.volume_diff_tema.val
    }

    pub fn get_quote_price(&self) -> (f64, f64) {
        let price_tema = self.get_tema_price();
        let mut sigma = self.get_tema_sigma() * self.sigma_multi;
        let sigma_min_price = self.sigma_min_bps * price_tema * 1e-4;
        sigma = sigma.max(sigma_min_price);
        let theo_bid = price_tema - sigma;
        let theo_ask = price_tema + sigma;
        (theo_ask, theo_bid)
    }
}