use bkbase::models::{Asset, DepthData};

#[derive(Debug, Clone)]
pub struct Ticker {
    pub asset: Asset,
    pub transaction_ms: u64,
    pub receive_ms: u64,
    pub ap1: f64,
    pub bp1: f64,
    pub av1: f64,
    pub bv1: f64,
}

impl Ticker {
    pub fn from_depth(depth: &DepthData) -> Option<Self> {
        if depth.asks[0].is_none() || depth.bids[0].is_none() {
            return None;
        }
        let ask = depth.asks[0].as_ref().unwrap();
        let bid = depth.bids[0].as_ref().unwrap();
        Some(Ticker {
            asset: depth.asset.clone(),
            transaction_ms: depth.transaction_time,
            receive_ms: depth.local_time_ns / 1_000_000,
            ap1: ask.price,
            bp1: bid.price,
            av1: ask.volume,
            bv1: bid.volume,
        })
    }

    pub fn get_delay(&self) -> u64 {
        self.receive_ms - self.transaction_ms
    }

    pub fn mid_price(&self) -> f64 {
        (self.ap1 + self.bp1) / 2f64
    }

    pub fn spread(&self) -> f64 {
        self.ap1 - self.bp1
    }
}