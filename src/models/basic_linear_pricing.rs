use bklib::legacy::RoundMethod::{Ceil, Floor};
use bklib::legacy::types::BkTradeRule;
use crate::domains::common::Ticker;
use crate::oms::TakerContext;

#[derive(Debug, Clone)]
pub struct BasicLinearTakerContext {
    pub theo_bid: f64,
    pub theo_ask: f64,
    pub ticker: Ticker,
    pub position_usd: f64,
    pub now_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PricingReportContext {
    pub buy_threshold: f64,
    pub buy_profit: f64,
    pub sell_threshold: f64,
    pub sell_profit: f64,
}

#[derive(Debug, Clone)]
pub struct TakerOrderReportContext {
    pub taker: TakerContext,
    pub position: f64,
    pub taker_threshold: f64,
    pub taker_profit: f64,
    pub ap1: f64,
    pub bp1: f64,
}

pub struct BasicLinearTaker {
    taker_threshold: f64,
    taker_fee: f64,
    position_unit_usd: f64,
    _position_limit: f64,
    position_limit_usd: f64,
    bias_rate: Option<f64>,
}

impl BasicLinearTaker {

    pub fn new(
        taker_threshold: f64,
        taker_fee: f64,
        position_unit_usd: f64,
        position_limit: f64,
        bias_rate: Option<f64>,
    ) -> Self {
        let position_limit_usd = position_unit_usd * position_limit;
        BasicLinearTaker {
            taker_threshold,
            taker_fee,
            position_unit_usd,
            _position_limit: position_limit,
            position_limit_usd,
            bias_rate,
        }
    }

    pub fn get_taker_ctx(
        &self,
        pricing_ctx: BasicLinearTakerContext,
        trade_rule: &BkTradeRule
    ) -> (Vec<TakerOrderReportContext>, PricingReportContext) {
        let mut ret = vec![];
        let (buy_bias, sell_bias) = if self.bias_rate.is_some() {
            let br = self.bias_rate.unwrap();
            let bias = pricing_ctx.position_usd / self.position_unit_usd;
            if bias > 0.0 {
                (bias * br, 0.0)
            } else {
                (0.0, -bias * br)
            }
        } else {
            (0.0, 0.0)
        };

        let buy_threshold = self.taker_threshold + self.taker_fee + buy_bias;
        let buy_profit = pricing_ctx.theo_bid / pricing_ctx.ticker.ap1 - 1.0;
        if buy_profit > buy_threshold {
            let mut buy_price = pricing_ctx.ticker.ap1 * (1.0 + buy_profit - buy_threshold);
            buy_price = trade_rule.get_safe_price_with_round_method(buy_price, Floor);
            let mut size = trade_rule.get_size_from_usd(self.position_unit_usd, buy_price);
            size = trade_rule.get_safe_size_ceil(size);
            let taker_ctx = TakerContext {
                asset: pricing_ctx.ticker.asset.clone(),
                price: Some(buy_price),
                size,
                is_market: false,
                max_usd_pos: self.position_limit_usd,
                now_ms: pricing_ctx.now_ms,
            };
            ret.push(TakerOrderReportContext {
                taker: taker_ctx,
                position: pricing_ctx.position_usd,
                taker_threshold: buy_threshold,
                taker_profit: buy_profit,
                ap1: pricing_ctx.ticker.ap1,
                bp1: pricing_ctx.ticker.bp1,
            });
        }

        let sell_threshold = self.taker_threshold + self.taker_fee + sell_bias;
        let sell_profit = 1.0 - pricing_ctx.theo_ask / pricing_ctx.ticker.bp1;
        if sell_profit > sell_threshold {
            let mut sell_price = pricing_ctx.ticker.bp1 * (1.0 - (sell_profit - sell_threshold));
            sell_price = trade_rule.get_safe_price_with_round_method(sell_price, Ceil);
            let mut size = trade_rule.get_size_from_usd(self.position_unit_usd, sell_price);
            size = trade_rule.get_safe_size_ceil(size);
            let taker_ctx = TakerContext {
                asset: pricing_ctx.ticker.asset.clone(),
                price: Some(sell_price),
                size: -size,
                is_market: false,
                max_usd_pos: self.position_limit_usd,
                now_ms: pricing_ctx.now_ms,
            };
            ret.push(TakerOrderReportContext {
                taker: taker_ctx,
                position: pricing_ctx.position_usd,
                taker_threshold: sell_threshold,
                taker_profit: sell_profit,
                ap1: pricing_ctx.ticker.ap1,
                bp1: pricing_ctx.ticker.bp1,
            });
        }
        (ret, PricingReportContext {
            buy_threshold,
            buy_profit,
            sell_threshold,
            sell_profit,
        })
    }

}