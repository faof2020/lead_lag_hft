use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use bkbase::models::{Asset, AssetVec, Exchange};
use bklib::legacy::{spawn_legacy_thread, BkLegacyClient};
use bklib::legacy::handler::{BkLegacyDefaultHander, BkLegacyUserInfo};
use anyhow::{anyhow, Result};
use crate::reporter::BatchReportHandler;

pub fn init_legacy(
    instance_id: &str,
    user_infos: Vec<BkLegacyUserInfo>,
    assets: AssetVec,
    core_idx: Option<usize>,
) -> Result<(BkLegacyClient, Arc<AtomicBool>)> {
    let rpc_id: u64 = rand::random();
    let legacy_client = BkLegacyClient::new(rpc_id);
    let mut handler = BkLegacyDefaultHander::new(instance_id, user_infos, assets);
    handler.set_raw_handler(Box::new(BatchReportHandler {}));
    let (start_signal, exit_signal) = spawn_legacy_thread(Box::new(handler), rpc_id, 100, core_idx);

    // 等待 legacy 模块初始化完成
    let legacy_ready = start_signal.wait();
    if !legacy_ready {
        return Err(anyhow!("legacy module init failed"));
    }
    Ok((legacy_client, exit_signal))
}

pub fn get_default_exchange_asset(exchange: &Exchange) -> AssetVec {
    let asset = if exchange.eq(&Exchange::BINANCE) {
        Asset::from_str("BINANCE_SWAP_BTC-USDT").unwrap()
    } else if exchange.eq(&Exchange::COINEXV2){
        Asset::from_str("COINEXV2_SWAP_BTC-USDT").unwrap()
    } else {
        panic!("unsupported exchange: {:?}", exchange);
    };
    AssetVec::from_vec(vec![asset])
}