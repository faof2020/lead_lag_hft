use std::collections::HashMap;
use std::str::FromStr;
use bkbase::models::{Asset, AssetVec};
use serde::Deserialize;
use crate::common_config::StrategyConfig;

#[derive(Deserialize, Debug, Clone)]
pub struct NewCoinMakerConfig {
    pub report_measurement: String,
    pub trade_assets: Vec<TradeAssetConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TradeAssetConfig {
    pub asset: String,
    pub trading: bool,
    pub tau_p: String,
    pub tau_o: String,
    pub pos_unit_usd: f64,
    pub pos_limit: f64,
    pub sigma_multi: f64,
    pub sigma_min_bps: f64,
    pub order_min_bps_diff: f64,
    pub order_min_tick_diff: f64,
}

impl StrategyConfig for NewCoinMakerConfig {
    fn get_market_assets(&self) -> AssetVec {
        let mut ret = vec![];
        for trade_asset_config in &self.trade_assets {
            let asset = Asset::from_str(trade_asset_config.asset.as_str()).unwrap();
            if !ret.contains(&asset) {
                ret.push(asset);
            }
        }
        AssetVec::from(ret)
    }

    fn get_trade_assets(&self) -> AssetVec {
        self.get_market_assets()
    }

    fn get_asset_trading(&self) -> HashMap<Asset, bool> {
        let mut ret = HashMap::new();
        for trade_asset_config in &self.trade_assets {
            let asset = Asset::from_str(trade_asset_config.asset.as_str()).unwrap();
            ret.insert(asset, trade_asset_config.trading);
        }
        ret
    }
}