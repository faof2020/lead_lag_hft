use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::vec;
use bkbase::models::{Asset, AssetVec, Exchange, TradeData};
use bklib::legacy::{spawn_legacy_thread, BkLegacyClient};
use bklib::legacy::handler::{BkLegacyDefaultHander, BkLegacyUserInfo};
use anyhow::{anyhow, Result};
use bklib::market::get_bkmarket_ref;
use crate::reporter::BatchReportHandler;

pub fn init_legacy(
    instance_id: &str,
    user_infos: Vec<BkLegacyUserInfo>,
    assets: AssetVec,
    core_idx: Option<usize>,
) -> Result<(BkLegacyClient, Arc<AtomicBool>)> {
    let rpc_id: u64 = rand::random();
    let legacy_client = BkLegacyClient::new(rpc_id);
    let mut use_assets = assets.clone();
    for info in user_infos.iter() {
        let info_assets = &info.assets;
        for asset in info_assets.iter() {
            if !use_assets.contains(asset) {
                use_assets.push(asset.clone());
            }
        }
    }
    let mut handler = BkLegacyDefaultHander::new(instance_id, user_infos, use_assets);
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
    let assets = if exchange.eq(&Exchange::BINANCE) {
        vec![
            Asset::from_str("BINANCE_SWAP_BTC-USDT").unwrap(),
            Asset::from_str("BINANCE_SWAP_BTC-USDC").unwrap(),
        ]
    } else if exchange.eq(&Exchange::COINEXV2){
        vec![Asset::from_str("COINEXV2_SWAP_BTC-USDT").unwrap()]
    } else {
        panic!("unsupported exchange: {:?}", exchange);
    };
    AssetVec::from_vec(assets)
}

pub fn bk_get_trades(asset: &Asset, start_id: u64) -> (Vec<TradeData>, u64) {
    let bk_market = get_bkmarket_ref();
    let asset_snap = bk_market.asset_map.get(asset);
    if asset_snap.is_none() {
        tracing::warn!("market asset not found: {:?}", asset);
        return (vec![], start_id);
    }
    let mut ret = vec![];
    let mut last_id = start_id;
    for trade_data in asset_snap.unwrap().trade_list.iter() {
        if trade_data.id.is_some() {
            let id = trade_data.id.unwrap();
            if id > start_id {
                ret.insert(0, trade_data.clone());
                if id > last_id {
                    last_id = id;
                }
            } else {
                break;
            }
        } else {
            tracing::warn!("{:?} trade data id is none.", asset);
            continue;
        }
    }
    (ret, last_id)
}