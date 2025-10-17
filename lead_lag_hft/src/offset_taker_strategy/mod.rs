use std::collections::HashMap;
use std::str::FromStr;
use crate::offset_taker_strategy::offset_taker_config::OffsetTakerConfig;
use crate::strategy::{Strategy, StrategyBehavior};
use bkbase::models::Asset;
use anyhow::{anyhow, Result};
use bkbase::utils::time::now_ms;
use serde_json::json;
use crate::calculator::offset_cache::OffsetCache;
use crate::domains::common::Ticker;
use crate::models::basic_linear_pricing::{BasicLinearTaker, BasicLinearTakerContext};
use crate::models::offset_theo_price::get_theo_taker_price;

pub mod offset_taker_config;

pub struct OffsetTakerStrategy {
    lead2lag: HashMap<Asset, Asset>,
    lag2lead: HashMap<Asset, Asset>,
    offset_cache: OffsetCache,
    max_usd_pos_map: HashMap<Asset, f64>,
    use_period_map: HashMap<Asset, String>,
    asset_pricing_map: HashMap<Asset, BasicLinearTaker>,
    report_measurement: String,
    report_order_measurement: String,
}

impl StrategyBehavior<OffsetTakerConfig> for OffsetTakerStrategy {

    fn on_tick(&mut self, base: &mut Strategy<OffsetTakerConfig>, asset: Asset) -> Result<()>{
        let now_ms = now_ms();
        if self.lead2lag.contains_key(&asset) {
            let lead_ticker = base.ticker_map.get(&asset).unwrap().clone();
            if !self.delay_check(&lead_ticker, base) {
                return Ok(());
            }
            let lag_asset = self.lead2lag.get(&asset).unwrap();
            if !base.ticker_map.contains_key(lag_asset) {
                tracing::warn!("{} ticker not found.", lag_asset);
                return Ok(());
            }
            let lag_ticker = base.ticker_map.get(lag_asset).unwrap().clone();
            base.batch_report_custom_data(
                &self.report_measurement,
                lag_asset,
                HashMap::from([("mid_price".to_string(), json!(lag_ticker.mid_price()))]),
            );
            if !self.use_period_map.contains_key(lag_asset) {
                tracing::warn!("{:?} trade offset period not found", lag_asset.pair.0);
                return Ok(());
            }
            let use_period = self.use_period_map.get(lag_asset).unwrap();
            let theo_price = get_theo_taker_price(
                &lead_ticker, &use_period, &self.offset_cache,
            );
            if let Err(e) = &theo_price {
                tracing::warn!("{:?}", e);
                return Ok(());
            }
            let (theo_ask, theo_bid) = theo_price?;
            let position = base.get_asset_usd_position(lag_asset);
            if let Err(e) = &position {
                tracing::warn!("{:?}", e);
                return Ok(());
            }
            let position = position?;
            if !self.asset_pricing_map.contains_key(lag_asset) {
                tracing::warn!("{:?} pricing model not found", lag_asset);
                return Ok(());
            }
            let pricing = self.asset_pricing_map.get(lag_asset).unwrap();
            if !base.trade_rule_map.contains_key(lag_asset) {
                tracing::warn!("{:?} trade rule not found", lag_asset);
                return Ok(());
            }
            let trade_rule = base.trade_rule_map.get(lag_asset).unwrap();
            let pricing_ctx = BasicLinearTakerContext {
                theo_bid,
                theo_ask,
                ticker: lag_ticker,
                position_usd: position,
                now_ms,
            };
            let (taker_ctx_vec, pricing_report) = pricing.get_taker_ctx(
                pricing_ctx, trade_rule
            );
            base.batch_report_custom_data(
                &self.report_measurement,
                lag_asset,
                HashMap::from([
                    ("buy_threshold".to_string(), json!(pricing_report.buy_threshold)),
                    ("buy_profit".to_string(), json!(pricing_report.buy_profit)),
                    ("sell_threshold".to_string(), json!(pricing_report.sell_threshold)),
                    ("sell_profit".to_string(), json!(pricing_report.sell_profit)),
                ]),
            );
            for ctx in taker_ctx_vec.iter() {
                if let Err(e) = base.do_taker(ctx.taker.clone()) {
                    tracing::warn!("{:?}", e);
                }
            }
        } else if self.lag2lead.contains_key(&asset) {
            if !self.offset_cache.init {
                return Ok(())
            }
            let lead_asset = self.lag2lead.get(&asset).unwrap();
            let lead_ticker = base.ticker_map.get(lead_asset);
            if lead_ticker.is_none() {
                tracing::warn!("{:?} get lead tick none when update offset", asset);
                return Ok(());
            }
            let lead_ticker = lead_ticker.unwrap();
            let lag_ticker = base.ticker_map.get(&asset).unwrap();
            let _ = self.offset_cache.update(
                lead_ticker, lag_ticker, now_ms, base.redis_reporter.as_mut()
            );
            let all_period_offset = self.offset_cache.get_all_offset(&asset);
            if all_period_offset.is_none() {
                return Ok(());
            }
            let all_period_offset = all_period_offset.unwrap();
            let mut data_map = HashMap::new();
            for offset in all_period_offset {
                let period = offset.period.clone();
                data_map.insert(format!("{}_bid", &period), json!(offset.b2a));
                data_map.insert(format!("{}_ask", &period), json!(offset.a2b));
            }
            base.batch_report_custom_data(
                &self.report_measurement,
                &asset,
                data_map,
            );
        } else {
            tracing::warn!("{:?} is not lead or lag", asset);
        }
        Ok(())
    }

    fn on_init(&mut self, base: &mut Strategy<OffsetTakerConfig>) -> Result<()> {
        let taker_fee = base.config.taker_fee;
        for trade_asset_config in base.config.strategy_config.trade_assets.iter() {
            let lead = Asset::from_str(trade_asset_config.lead_asset.as_str())?;
            let lag = Asset::from_str(trade_asset_config.asset.as_str())?;
            self.lead2lag.insert(lead.clone(), lag.clone());
            self.lag2lead.insert(lag.clone(), lead.clone());
            let max_pos_usd = trade_asset_config.pos_unit_usd * trade_asset_config.pos_limit;
            self.max_usd_pos_map.insert(lag.clone(), max_pos_usd);
            let use_period = trade_asset_config.use_offset_period.clone();
            self.use_period_map.insert(lag.clone(), use_period);
            let pricing = BasicLinearTaker::new(
                trade_asset_config.taker_threshold,
                taker_fee,
                trade_asset_config.pos_unit_usd,
                trade_asset_config.pos_limit,
                trade_asset_config.bias_rate
            );
            self.asset_pricing_map.insert(lag.clone(), pricing);
        }
        self.offset_cache.init(
            &self.lead2lag,
            &base.config.strategy_config,
            base.redis_conn.as_mut()
        );
        self.report_measurement = base.config.strategy_config.report_measurement.to_string();
        self.report_order_measurement = base.config.strategy_config.order_report_measurement.to_string();
        Ok(())
    }

    fn asset_max_pos_usd(&mut self, asset: Asset) -> Result<f64> {
        if !self.max_usd_pos_map.contains_key(&asset) {
            return Err(anyhow!("get {} max usd position error.", asset));
        }
        Ok(*self.max_usd_pos_map.get(&asset).unwrap())
    }
}

impl OffsetTakerStrategy {

    pub fn new() -> Self {
        OffsetTakerStrategy {
            lead2lag: HashMap::new(),
            lag2lead: HashMap::new(),
            offset_cache: OffsetCache::new(),
            max_usd_pos_map: HashMap::new(),
            use_period_map: HashMap::new(),
            asset_pricing_map: HashMap::new(),
            report_measurement: "".to_string(),
            report_order_measurement: "".to_string(),
        }
    }

    fn delay_check(&self, ticker: &Ticker, base: &Strategy<OffsetTakerConfig>) -> bool {
        if !base.delay_map.contains_key(&ticker.asset) {
            tracing::warn!("{:?} delay data is none", ticker.asset);
            return false;
        }
        let delay_ema = base.delay_map.get(&ticker.asset).unwrap();
        if ticker.get_delay() > base.config.strategy_config.lead_max_delay {
            // tracing::warn!("lead tick delay: {}", ticker.get_delay());
            return false;
        }
        if delay_ema.delay > base.config.strategy_config.lead_max_delay as f64 {
            tracing::warn!("lead tick ema delay: {}", ticker.get_delay());
            return false;
        }
        true
    }

}