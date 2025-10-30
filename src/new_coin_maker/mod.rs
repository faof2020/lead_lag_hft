use std::collections::HashMap;
use std::str::FromStr;
use bkbase::models::{Asset, TradeData};
use anyhow::{anyhow, Result};
use bkbase::utils::time::now_ms;
use crate::models::basic_pricing::{BasicMaker, BasicMakerContext};
use crate::new_coin_maker::new_coin_maker_config::NewCoinMakerConfig;
use crate::new_coin_maker::new_coin_maker_model::NewCoinMakerModel;
use crate::strategy::{Strategy, StrategyBehavior};

pub mod new_coin_maker_config;
mod new_coin_maker_model;

pub struct NewCoinMakerStrategy {
    asset_model_map: HashMap<Asset, NewCoinMakerModel>,
    max_usd_pos_map: HashMap<Asset, f64>,
    asset_pricing_map: HashMap<Asset, BasicMaker>,
    min_bps_diff_map: HashMap<Asset, f64>,
    min_tick_diff_map: HashMap<Asset, f64>,
}

impl NewCoinMakerStrategy {
    pub fn new() -> Self {
        NewCoinMakerStrategy {
            asset_model_map: HashMap::new(),
            max_usd_pos_map: HashMap::new(),
            asset_pricing_map: HashMap::new(),
            min_bps_diff_map: HashMap::new(),
            min_tick_diff_map: HashMap::new(),
        }
    }
}

impl StrategyBehavior<NewCoinMakerConfig> for NewCoinMakerStrategy {
    fn on_tick(&mut self, base: &mut Strategy<NewCoinMakerConfig>, asset: Asset) -> Result<()> {
        let now_ms = now_ms();
        let ticker = base.ticker_map.get(&asset).unwrap().clone();
        let model = self.asset_model_map.get(&asset).unwrap();
        if !model.is_ready() {
            tracing::warn!("{:?} model is not ready.", asset);
            return Ok(());
        }
        let (theo_ask, theo_bid) = model.get_quote_price();
        let position = base.get_asset_usd_position(&asset);
        if let Err(e) = &position {
            tracing::warn!("{:?}", e);
            return Ok(());
        }
        let position = position?;
        if !self.min_bps_diff_map.contains_key(&asset) {
            tracing::warn!("{:?} min bps diff not found", asset);
            return Ok(());
        }
        if !self.min_tick_diff_map.contains_key(&asset) {
            tracing::warn!("{:?} min tick diff not found", asset);
            return Ok(());
        }
        let min_bps_diff = *self.min_bps_diff_map.get(&asset).unwrap();
        let min_tick_diff = *self.min_tick_diff_map.get(&asset).unwrap();
        let pricing_ctx = BasicMakerContext {
            theo_bid,
            theo_ask,
            ticker,
            position_usd: position,
            min_bps_diff,
            min_tick_diff,
            now_ms,
        };
        if !self.asset_pricing_map.contains_key(&asset) {
            tracing::warn!("{:?} priding model not found", asset);
            return Ok(());
        }
        let pricing_model = self.asset_pricing_map.get(&asset).unwrap();
        if !base.trade_rule_map.contains_key(&asset) {
            tracing::warn!("{:?} trade rule not found", asset);
            return Ok(());
        }
        let trade_rule = base.trade_rule_map.get(&asset).unwrap();
        let (makers, _) = pricing_model.get_maker_ctx(pricing_ctx, trade_rule);
        for maker_ctx in makers.iter() {
            if let Err(e) = base.do_maker(maker_ctx.maker.clone()) {
                tracing::warn!("{:?}", e);
            }
        }
        Ok(())
    }

    fn on_init(&mut self, base: &mut Strategy<NewCoinMakerConfig>) -> Result<()> {
        for asset_trade_config in base.config.strategy_config.trade_assets.iter() {
            let asset = Asset::from_str(&asset_trade_config.asset)?;
            self.asset_model_map.insert(asset.clone(), NewCoinMakerModel::new(asset_trade_config));
            let max_pos_usd = asset_trade_config.pos_unit_usd * asset_trade_config.pos_limit;
            self.max_usd_pos_map.insert(asset.clone(), max_pos_usd);
            self.asset_pricing_map.insert(asset.clone(), BasicMaker::new(
                asset_trade_config.pos_unit_usd, asset_trade_config.pos_limit,
            ));
            self.min_bps_diff_map.insert(asset.clone(), asset_trade_config.order_min_bps_diff);
            self.min_tick_diff_map.insert(asset.clone(), asset_trade_config.order_min_tick_diff);
        }
        Ok(())
    }

    fn on_trade(&mut self, _base: &mut Strategy<NewCoinMakerConfig>, asset: Asset, trades: Vec<TradeData>) -> Result<()> {
        if !self.asset_model_map.contains_key(&asset) {
            tracing::warn!("get {:?} trades, not in config file.", asset);
        }
        let model = self.asset_model_map.get_mut(&asset).unwrap();
        for trade in trades.iter() {
            model.update(trade);
        }
        Ok(())
    }

    fn asset_max_pos_usd(&mut self, asset: Asset) -> Result<f64> {
        if !self.max_usd_pos_map.contains_key(&asset) {
            return Err(anyhow!("get {} max usd position error.", asset));
        }
        Ok(*self.max_usd_pos_map.get(&asset).unwrap())
    }
}