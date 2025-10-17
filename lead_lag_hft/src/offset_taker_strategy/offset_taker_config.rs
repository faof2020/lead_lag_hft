use std::collections::HashMap;
use std::str::FromStr;
use bkbase::models::{Asset, AssetVec};
use serde::Deserialize;
use crate::calculator::offset_ema::OffsetEmaConfig;
use crate::common_config::StrategyConfig;

#[derive(Deserialize, Debug, Clone)]
pub struct OffsetTakerConfig {
    pub offset_configs: Vec<OffsetEmaConfig>,
    pub lead_max_delay: u64,
    pub lag_max_delay: u64,
    pub lead_max_expiration: u64,
    pub trade_assets: Vec<TradeAssetConfig>,
    pub report_measurement: String,
    pub order_report_measurement: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TradeAssetConfig {
    pub asset: String,
    pub lead_asset: String,
    pub trading: bool,
    pub pos_limit: f64,
    pub pos_unit_usd: f64,
    pub use_offset_period: String,
    pub taker_threshold: f64,
    pub bias_rate: Option<f64>,
}

impl StrategyConfig for OffsetTakerConfig {

    fn get_market_assets(&self) -> AssetVec {
        let mut ret = vec![];
        for trade_asset_config in &self.trade_assets {
            let lead = Asset::from_str(trade_asset_config.lead_asset.as_str()).unwrap();
            let lag = Asset::from_str(trade_asset_config.asset.as_str()).unwrap();
            if !ret.contains(&lead) {
                ret.push(lead);
            }
            if !ret.contains(&lag) {
                ret.push(lag);
            }
        }
        AssetVec::from(ret)
    }

    fn get_trade_assets(&self) -> AssetVec {
        let mut ret = vec![];
        for trade_asset_config in &self.trade_assets {
            let lag = Asset::from_str(trade_asset_config.asset.as_str()).unwrap();
            if !ret.contains(&lag) {
                ret.push(lag);
            }
        }
        AssetVec::from(ret)
    }

    fn get_asset_trading(&self) -> HashMap<Asset, bool> {
        let mut ret = HashMap::new();
        for trade_asset_config in &self.trade_assets {
            let lag = Asset::from_str(trade_asset_config.asset.as_str()).unwrap();
            ret.insert(lag, trade_asset_config.trading);
        }
        ret
    }

}