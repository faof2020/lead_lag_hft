use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use bkbase::models::{Asset, AssetVec, Exchange};
use bklib::legacy::ExCredential;
use bklib::legacy::handler::BkLegacyUserInfo;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use toml;
use crate::calculator::delay_ema::DelayEmaConfig;
use crate::calculator::spread_ema::SpreadEmaConfig;

#[derive(Deserialize, Debug, Clone)]
pub struct CredentialConfig {
    pub exchange: String,
    pub ak: String,
    pub sk: String,
    pub pwd: Option<String>,
    pub extra_info: Option<String>,
    pub user_id: String,
}

pub trait StrategyConfig: DeserializeOwned + Debug + Clone {
    fn get_market_assets(&self) -> AssetVec;
    fn get_trade_assets(&self) -> AssetVec;
    fn get_asset_trading(&self) -> HashMap<Asset, bool>;
}

#[derive(Deserialize, Debug, Clone)]
pub struct CommonConfig<T> {
    pub instance_id: String,
    pub market_worker_id: String,
    pub legacy_core_id: usize,
    pub trading: bool,
    pub taker_fee: f64,
    pub maker_fee: f64,
    pub redis_url: Option<String>,
    pub ex_credential_configs: Vec<CredentialConfig>,
    pub spread_ema_config: SpreadEmaConfig,
    pub delay_ema_config: DelayEmaConfig,
    pub quote_intval: u64,
    pub strategy_config: T,
}

impl<T> CommonConfig<T>
where T: StrategyConfig
{
    pub fn get_bk_userinfo(&self) -> Vec<BkLegacyUserInfo> {
        let all_asset = self.strategy_config.get_trade_assets();
        let mut asset_group = HashMap::new();
        for asset in all_asset.iter() {
            if !asset_group.contains_key(&asset.exchange) {
                let assets = vec![asset.clone()];
                asset_group.insert(asset.exchange, assets);
            } else {
                let assets = asset_group.get_mut(&asset.exchange).unwrap();
                assets.push(asset.clone());
            }
        }
        let mut credential_map = HashMap::new();
        let mut uid_map = HashMap::new();
        for credential in self.ex_credential_configs.iter() {
            let exchange = Exchange::from_str(&credential.exchange).unwrap();
            let extra_data = if credential.extra_info.is_none() {
                None
            } else {
                let ak = credential.extra_info.as_ref().unwrap();
                let ak_json = json!({"ak": ak});
                Some(ak_json)
            };
            credential_map.insert(
                exchange.clone(),
                ExCredential {
                    api_key: credential.ak.clone(),
                    secret_key: credential.sk.clone(),
                    password: credential.pwd.clone(),
                    extra_data,
                },
            );
            uid_map.insert(exchange, credential.user_id.to_string());
        }
        let mut ret = vec![];
        for (exchange, assets) in asset_group.iter() {
            if credential_map.contains_key(exchange) && uid_map.contains_key(exchange) {
                let credential = credential_map.get(exchange).unwrap();
                ret.push(BkLegacyUserInfo {
                    exchange: exchange.clone(),
                    assets: AssetVec::from_vec(assets.clone()),
                    credential: credential.clone(),
                    user_id: uid_map.get(exchange).unwrap().to_string(),
                });
            }
        }
        ret
    }

    pub fn get_uid_asset_map(&self) -> HashMap<String, AssetVec> {
        let mut ret = HashMap::new();
        let all_asset = self.strategy_config.get_trade_assets();
        let mut asset_group = HashMap::new();
        for asset in all_asset.iter() {
            if !asset_group.contains_key(&asset.exchange) {
                let assets = vec![asset.clone()];
                asset_group.insert(asset.exchange, assets);
            } else {
                let assets = asset_group.get_mut(&asset.exchange).unwrap();
                assets.push(asset.clone());
            }
        }
        for credential in self.ex_credential_configs.iter() {
            let exchange = Exchange::from_str(&credential.exchange).unwrap();
            if asset_group.contains_key(&exchange) {
                let assets = asset_group.get(&exchange).unwrap().clone();
                ret.insert(credential.user_id.to_string(), AssetVec::from_vec(assets));
            }
        }
        ret
    }

}

pub fn load_config_from_args<T>() -> CommonConfig<T>
where T: StrategyConfig
{
    let args = std::env::args().collect::<Vec<String>>();
    let file_path = args.get(1).expect("config file path not found");
    let file = std::fs::read_to_string(file_path).expect("failed to read config file");
    let config: CommonConfig<T> = toml::from_str(&file).expect("failed to parse config file");
    config
}