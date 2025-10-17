use crate::calculator::offset_cache::OffsetCache;
use crate::domains::common::Ticker;
use anyhow::{anyhow, Result};

// return (ask, bid)
pub fn get_theo_maker_price(lead: &Ticker, period: &str, offset_cache: &OffsetCache) -> Result<(f64, f64)> {
    let ema = offset_cache.get_offset(&lead.asset, period);
    if ema.is_none() {
        return Err(anyhow!("{:?} offset is none", lead.asset));
    }
    let ema = ema.unwrap();
    Ok(((ema.a2a + 1.0) * lead.ap1, (ema.a2b + 1.0) * lead.bp1))
}

pub fn get_theo_taker_price(lead: &Ticker, period: &str, offset_cache: &OffsetCache) -> Result<(f64, f64)> {
    let ema = offset_cache.get_offset(&lead.asset, period);
    if ema.is_none() {
        return Err(anyhow!("{:?} offset is none", lead.asset.pair.0));
    }
    let ema = ema.unwrap();
    if !ema.init {
        return Err(anyhow!("{:?} offset is not init: {:?}", lead.asset.pair.0, ema));
    }
    Ok(((ema.a2b + 1.0) * lead.bp1, (ema.b2a + 1.0) * lead.ap1))
}