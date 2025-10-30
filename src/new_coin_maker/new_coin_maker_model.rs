use std::str::FromStr;
use bkbase::models::{Asset, TradeData};
use crate::calculator::tema::TemaMs;
use crate::new_coin_maker::new_coin_maker_config::TradeAssetConfig;

pub struct NewCoinMakerModel {
    pub asset: Asset,
    pub value_tema: TemaMs,
    pub volume_tema: TemaMs,
    pub value_diff_tema: TemaMs,
    pub volume_diff_tema: TemaMs,
    sigma_multi: f64,
}

impl NewCoinMakerModel {
    pub fn new(config: &TradeAssetConfig) -> Self {
        NewCoinMakerModel {
            asset: Asset::from_str(&config.asset).unwrap(),
            value_tema: TemaMs::new(config.tau_p),
            volume_tema: TemaMs::new(config.tau_p),
            value_diff_tema: TemaMs::new(config.tau_o),
            volume_diff_tema: TemaMs::new(config.tau_o),
            sigma_multi: config.sigma_multi,
        }
    }

    pub fn update(&mut self, trade: &TradeData) {
        let value = trade.price * trade.volume.abs();
        let volume = trade.volume.abs();
        self.value_tema.update(value, trade.transaction_time);
        self.volume_tema.update(volume, trade.transaction_time);
        let price_tema = self.value_tema.val / self.volume_tema.val;
        let diff = price_tema * f64::ln(trade.price / price_tema);
        let value_diff = diff.abs() * trade.volume.abs();
        self.value_diff_tema.update(value_diff, trade.transaction_time);
        self.volume_diff_tema.update(volume, trade.transaction_time);
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
        let sigma = self.get_tema_sigma();
        let theo_bid = price_tema - sigma * self.sigma_multi;
        let theo_ask = price_tema + sigma * self.sigma_multi;
        (theo_ask, theo_bid)
    }
}