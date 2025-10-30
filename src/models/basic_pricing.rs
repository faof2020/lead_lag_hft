use bklib::legacy::RoundMethod::{Ceil, Floor};
use bklib::legacy::types::BkTradeRule;
use crate::domains::common::Ticker;
use crate::oms::MakerContext;

#[derive(Debug, Clone)]
pub struct BasicMakerContext {
    pub theo_bid: f64,
    pub theo_ask: f64,
    pub ticker: Ticker,
    pub position_usd: f64,
    pub min_bps_diff: f64,
    pub min_tick_diff: f64,
    pub now_ms: u64,
}

pub struct BasicMaker {
    position_unit_usd: f64,
    position_limit_usd: f64,
}

#[derive(Debug, Clone)]
pub struct MakerOrderReportContext {
    pub maker: MakerContext,
}

#[derive(Debug, Clone)]
pub struct PricingReportContext {
}

impl BasicMaker {
    pub fn new(
        position_unit_usd: f64,
        position_limit: f64,
    ) -> Self {
        let position_limit_usd = if position_limit - 1.0 < 1e-8 {
            position_unit_usd * 0.1
        } else {
            position_unit_usd * position_limit
        };
        BasicMaker {
            position_unit_usd,
            position_limit_usd,
        }
    }

    pub fn get_maker_ctx(
        &self,
        pricing_ctx: BasicMakerContext,
        trade_rule: &BkTradeRule
    ) -> (Vec<MakerOrderReportContext>, PricingReportContext) {
        let mut bid_price = pricing_ctx.theo_bid
            .min(pricing_ctx.ticker.bp1 + trade_rule.price_unit)
            .min(pricing_ctx.ticker.ap1 - trade_rule.price_unit);
        let mut ask_price = pricing_ctx.theo_ask
            .max(pricing_ctx.ticker.ap1 - trade_rule.price_unit)
            .max(pricing_ctx.ticker.bp1 + trade_rule.price_unit);
        bid_price = trade_rule.get_safe_price_with_round_method(bid_price, Floor);
        ask_price = trade_rule.get_safe_price_with_round_method(ask_price, Ceil);
        let mid_price = (bid_price + ask_price) / 2.0;
        let mut size = trade_rule.get_size_from_usd(self.position_unit_usd, mid_price);
        size = trade_rule.get_safe_size_ceil(size);
        let mut min_price_diff = mid_price * pricing_ctx.min_bps_diff * 1e-4;
        min_price_diff = min_price_diff.max(trade_rule.price_unit * pricing_ctx.min_tick_diff);
        let ret = vec![
            MakerOrderReportContext {
                maker: MakerContext {
                    asset: pricing_ctx.ticker.asset.clone(),
                    price: bid_price,
                    size,
                    is_post_only: true,
                    is_first: false,
                    max_order_num: 1,
                    order_min_price_diff: min_price_diff,
                    max_usd_pos: self.position_limit_usd,
                    now_ms: pricing_ctx.now_ms,
                },
            }, MakerOrderReportContext {
                maker: MakerContext {
                    asset: pricing_ctx.ticker.asset.clone(),
                    price: ask_price,
                    size: -size,
                    is_post_only: true,
                    is_first: false,
                    max_order_num: 1,
                    order_min_price_diff: min_price_diff,
                    max_usd_pos: self.position_limit_usd,
                    now_ms: pricing_ctx.now_ms,
                },
        }];
        (ret, PricingReportContext {})
    }
}