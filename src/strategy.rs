use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use bkbase::models::{Asset, AssetType, AssetVec, Exchange, TradeData};
use bklib::market::{get_bkmarket_mut, get_bkmarket_ref, init_bk_market};
use crate::common_config::*;
use anyhow::{anyhow, Result};
use bkbase::utils::time::now_ms;
use bkclient::models::MarketUpdateData;
use bklib::BkMarketClientConfig;
use bklib::legacy::BkLegacyClient;
use bklib::legacy::proto::{BkLegacyRequest, BkLegacyResponse};
use bklib::legacy::types::BkTradeRule;
use bklib::private::{BkPrivate, BkPrivateConfig, BkVirtualPositionRiskConfig};
use redis::{Client, Connection};
use serde_json::Value;
use crate::calculator::delay_ema::DelayEma;
use crate::calculator::spread_ema::SpreadEma;
use crate::utils::bk_util::{bk_get_trades, init_legacy};
use crate::domains::common::Ticker;
use crate::oms::{MakerContext, Oms, TakerContext};
use crate::redis_reporter::RedisReporter;
use crate::reporter::Reporter;
use crate::utils::redis_util::{REDIS_DELAY_KET, REDIS_SPREAD_KET};

pub trait StrategyBehavior<T> {
    fn on_tick(&mut self, strategy: &mut Strategy<T>, asset: Asset) -> Result<()>;
    fn on_init(&mut self, strategy: &mut Strategy<T>) -> Result<()>;
    fn on_trade(&mut self, strategy: &mut Strategy<T>, asset: Asset, trades: Vec<TradeData>) -> Result<()>;
    fn asset_max_pos_usd(&mut self, asset: Asset) -> Result<f64>;
}

pub struct Strategy<T> {
    pub(crate) config: CommonConfig<T>,
    pub(crate) redis_conn: Option<Connection>,
    pub(crate) redis_reporter: Option<RedisReporter>,
    market_assets: AssetVec,
    legacy_client: BkLegacyClient,
    legacy_exit: Arc<AtomicBool>,
    bk_privates: HashMap<Exchange, BkPrivate>,
    pub(crate) trade_rule_map: HashMap<Asset, BkTradeRule>,
    pub(crate) ticker_map: HashMap<Asset, Ticker>,
    spread_map: HashMap<Asset, SpreadEma>,
    pub(crate) delay_map: HashMap<Asset, DelayEma>,
    pub(crate) oms_map: HashMap<Asset, Oms>,
    reporter: Reporter,
    asset_last_id_map: HashMap<Asset, u64>,
}

impl<T> Strategy<T>
where T: StrategyConfig
{
    pub fn new() -> Self
    {
        let config = load_config_from_args::<T>();
        let market_assets = config.strategy_config.get_market_assets();
        let bk_user_info = config.get_bk_userinfo();
        let (bk_legacy, legacy_exit) = init_legacy(
            &config.instance_id,
            bk_user_info,
            market_assets.clone(),
            Some(config.legacy_core_id),
        ).unwrap();
        let (redis_conn, redis_reporter) = if config.redis_url.is_some() {
            let url = config.redis_url.as_ref().unwrap().clone();
            let client = Client::open(url.clone()).unwrap();
            (Some(client.get_connection().unwrap()), Some(RedisReporter::new(&url)))
        } else {
            (None, None)
        };
        let instance_id = config.instance_id.clone();
        Strategy {
            config,
            redis_conn,
            redis_reporter,
            market_assets,
            legacy_client: bk_legacy,
            legacy_exit,
            bk_privates: HashMap::new(),
            trade_rule_map: HashMap::new(),
            ticker_map: HashMap::new(),
            spread_map: HashMap::new(),
            delay_map: HashMap::new(),
            oms_map: HashMap::new(),
            reporter: Reporter::new(&instance_id),
            asset_last_id_map: HashMap::new(),
        }
    }

    fn init<B: StrategyBehavior<T>>(&mut self, behavior: &mut B) -> Result<()> {
        init_bk_market(true);
        let market_config = BkMarketClientConfig {
            disable_depth: false,
            disable_trade: false,
            worker_id: self.config.market_worker_id.clone(),
            assets: self.market_assets.clone(),
        };
        {
            let market = get_bkmarket_mut();
            market.add_market(market_config);
        }
        let private_config = BkPrivateConfig {
            virtual_position_risk_config: BkVirtualPositionRiskConfig {
                max_diff_value: 100.0,
                min_diff_value: 100.0,
                max_unsync_time: 30,
            },
            virtual_account_balance_id_blacklist: None,
            virtual_account_balance_id_whitelist: None,
        };
        let resp = self
            .legacy_client
            .send_request_block(BkLegacyRequest::GetTradeRule);
        let trade_rule_map = match *resp {
            BkLegacyResponse::GetTradeRule(data) => data,
            _ => {
                panic!("get trade rule resp type error");
            }
        }
            .unwrap();
        self.trade_rule_map = trade_rule_map.clone();
        let uid_map = self.config.get_uid_asset_map();
        for (uid, assets) in uid_map {
            tracing::info!("start bkprivate, usr_id: {}, assets: {:?}", uid, assets);
            let exchange = assets[0].exchange.clone();
            let mut exchange_trade_rule_map = HashMap::new();
            for asset in assets.iter() {
                exchange_trade_rule_map
                    .insert(asset.clone(), trade_rule_map.get(asset).unwrap().clone());
            }
            let bk_private = BkPrivate::new(
                &uid,
                private_config.clone(),
                assets,
                exchange_trade_rule_map,
            )?;
            self.bk_privates.insert(exchange, bk_private);
        }
        let asset_trading_map = self.config.strategy_config.get_asset_trading();
        for (asset, trading) in asset_trading_map.iter() {
            if asset.asset_type == AssetType::SPOT {
                return Err(anyhow!("{:?} not supported asset type", asset));
            }
            let is_trading = self.config.trading && *trading;
            self.oms_map.insert(asset.clone(), Oms::new(asset, is_trading, &self.config));
        }

        behavior.on_init(self)
    }

    pub fn run<B: StrategyBehavior<T>>(&mut self, behavior: &mut B) -> Result<()> {
        self.init(behavior)?;
        loop {
            let market_update = get_bkmarket_mut().tick();
            for (_, bk_private) in self.bk_privates.iter_mut() {
                let _ = bk_private.tick()?;
            }
            if self.legacy_exit.load(Ordering::Relaxed) {
                tracing::warn!("legacy exit.");
                return Ok(());
            }
            if let Some((asset, update)) = market_update {
                let now_ms = now_ms();
                match update {
                    MarketUpdateData::TRADE(_) => {
                        let trade_last_id = if self.asset_last_id_map.contains_key(&asset) {
                            *self.asset_last_id_map.get(&asset).unwrap()
                        } else {
                            0
                        };
                        let (trades, last_id) = bk_get_trades(&asset, trade_last_id);
                        if last_id > trade_last_id {
                            self.asset_last_id_map.insert(asset.clone(), last_id);
                        }
                        if let Err(e) = behavior.on_trade(self, asset, trades) {
                            tracing::warn!("{:?}", e);
                        }
                    },
                    _ => {}
                }
                self.reporter.report_global(&mut self.legacy_client, now_ms);
                if !self.market_assets.contains(&asset) {
                    continue;
                }
                // 先获取当前价格
                let ticker = self.update_ticker_cache(&asset, now_ms);
                if ticker.is_none() {
                    continue;
                }
                let ticker = ticker.unwrap();
                if let Err(e) = self.sync_order_position(&asset, &ticker) {
                    tracing::warn!("{:?}", e);
                    continue;
                }
                if let Err(e) = behavior.on_tick(self, asset) {
                    tracing::warn!("{:?}", e);
                }
            }
        }
    }

    fn update_ticker_cache(&mut self, asset: &Asset, now_ms: u64) -> Option<Ticker> {
        let bk_market = get_bkmarket_ref();
        let asset_snap = bk_market.asset_map.get(asset);
        if asset_snap.is_none() {
            tracing::warn!("market asset not found: {:?}", asset);
            return None;
        }
        let depth = asset_snap.unwrap().virtual_depth.clone();
        if depth.is_none() {
            tracing::warn!("market asset depth not found: {:?}", asset);
            return None;
        }
        let depth = unsafe { depth.unwrap_unchecked() };
        let ticker = Ticker::from_depth(&depth);
        if ticker.is_none() {
            tracing::warn!("depth can not convert to ticker: {:?}", depth);
            return None;
        }
        let ticker = ticker.unwrap();
        if !self.ticker_map.contains_key(asset) {
            self.ticker_map.insert(asset.clone(), ticker.clone());

            self.spread_map.insert(asset.clone(), SpreadEma::new(
                &self.config.spread_ema_config, asset, self.redis_conn.as_mut()
            ));
            let spread = self.spread_map.get_mut(asset).unwrap();
            spread.update(&ticker, now_ms);

            self.delay_map.insert(asset.clone(), DelayEma::new(
                &self.config.delay_ema_config, asset, self.redis_conn.as_mut()
            ));
            let delay = self.delay_map.get_mut(asset).unwrap();
            delay.update(&ticker, now_ms);

            if self.redis_reporter.is_some() {
                let redis_reporter = self.redis_reporter.as_mut().unwrap();
                redis_reporter.record(
                    REDIS_SPREAD_KET,
                    &format!("{}_{}", asset.to_string(), spread.period),
                    spread.spread, now_ms
                );
                redis_reporter.record(
                    REDIS_DELAY_KET,
                    &format!("{}_{}", asset.to_string(), spread.period),
                    delay.delay, now_ms
                );
            }
            return Some(ticker);
        }
        let last_ticker = self.ticker_map.get(asset).unwrap();
        if ticker.transaction_ms > last_ticker.transaction_ms {
            self.ticker_map.insert(asset.clone(), ticker.clone());
            let spread = self.spread_map.get_mut(asset).unwrap();
            spread.update(&ticker, now_ms);

            let delay = self.delay_map.get_mut(asset).unwrap();
            delay.update(&ticker, now_ms);

            if self.redis_reporter.is_some() {
                let redis_reporter = self.redis_reporter.as_mut().unwrap();
                redis_reporter.record(
                    REDIS_SPREAD_KET,
                    &format!("{}_{}", asset.to_string(), spread.period),
                    spread.spread, now_ms
                );
                redis_reporter.record(
                    REDIS_DELAY_KET,
                    &format!("{}_{}", asset.to_string(), spread.period),
                    delay.delay, now_ms
                );
            }
            Some(ticker)
        } else {
            None
        }
    }

    fn sync_order_position(&mut self, asset: &Asset, ticker: &Ticker) -> Result<()> {
        if !self.oms_map.contains_key(asset) {
            return Ok(());
        }
        let bk_private = self.bk_privates.get(&asset.exchange).unwrap();
        if !bk_private.order_position_context.contains_key(asset) {
            return Err(anyhow!("{:?} get order position context none.", asset));
        }
        let op_ctx = bk_private.order_position_context.get(asset).unwrap();
        let order_ctx_rc = op_ctx.order_ctx;
        let order_ctx = order_ctx_rc.borrow();
        let opens = order_ctx.opened_orders.clone();
        let pending = order_ctx.pending_orders.clone();
        let canceling = order_ctx.canceling_orders.clone();
        let mid_price = ticker.mid_price();

        let position_ctx = &op_ctx.pos_ctx;
        let current_pos_value = position_ctx
            .rule
            .get_usd_size(position_ctx.current_position.get_total_volume(), mid_price);
        let virtual_pos_value = position_ctx
            .rule
            .get_usd_size(position_ctx.virtual_position.get_total_volume(), mid_price);
        let oms = self.oms_map.get_mut(asset).unwrap();
        oms.sync_position_and_orders(
            current_pos_value,
            virtual_pos_value,
            opens,
            pending,
            canceling,
        );
        Ok(())
    }

    pub fn get_asset_usd_position(&self, asset: &Asset) -> Result<f64> {
        if !self.oms_map.contains_key(asset) {
            return Err(anyhow!("get {:?} oms none.", asset));
        }
        let oms = self.oms_map.get(asset).unwrap();
        if oms.virtual_usd_position.is_none() {
            return Err(anyhow!("get {:?} usd position none.", asset));
        }
        Ok(oms.virtual_usd_position.unwrap())
    }

    pub fn do_taker(&mut self, taker: TakerContext) -> Result<()> {
        let asset = &taker.asset;
        if !self.oms_map.contains_key(asset) {
            return Err(anyhow!("get {:?} oms none.", asset));
        }
        let oms = self.oms_map.get_mut(asset).unwrap();
        let bk_private = self.bk_privates.get_mut(&asset.exchange).unwrap();
        let op_ctx = bk_private
            .order_position_context
            .get_mut(asset)
            .unwrap();
        let order_ctx_rc = &mut op_ctx.order_ctx;
        let order_ctx = order_ctx_rc.borrow_mut();
        oms.do_taker(taker, order_ctx, &mut bk_private.client)
    }

    pub fn do_maker(&mut self, maker: MakerContext) -> Result<()> {
        let asset = &maker.asset;
        if !self.oms_map.contains_key(asset) {
            return Err(anyhow!("get {:?} oms none.", asset));
        }
        let oms = self.oms_map.get_mut(asset).unwrap();
        let bk_private = self.bk_privates.get_mut(&asset.exchange).unwrap();
        let op_ctx = bk_private
            .order_position_context
            .get_mut(asset)
            .unwrap();
        let order_ctx_rc = &mut op_ctx.order_ctx;
        let order_ctx = order_ctx_rc.borrow_mut();
        oms.do_maker(maker, order_ctx, &mut bk_private.client)
    }

    pub fn batch_report_custom_data(&mut self, measurement: &str, asset: &Asset, data: HashMap<String, Value>) {
        let now_ms = now_ms();
        self.reporter.add_custom_batch_report_data(
            measurement,
            asset,
            data,
            &mut self.legacy_client,
            now_ms,
        );
    }

    pub fn report_single_custom_data(&mut self, measurement: &str, tag: HashMap<String, String>, data: HashMap<String, Value>) {
        let now_ms = now_ms();
        self.reporter.add_custom_single_report_data(
            measurement,
            tag,
            data,
            &mut self.legacy_client,
            now_ms,
        );
    }

}