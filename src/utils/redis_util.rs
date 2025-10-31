use std::collections::HashMap;
use bkbase::models::Asset;
use redis::{Commands, Connection, RedisResult};

pub const REDIS_OFFSET_KET: &str = "offset";
pub const REDIS_SPREAD_KET: &str = "spread";
pub const REDIS_DELAY_KET: &str = "delay";

pub fn read_offset_by_key(
    asset: &Asset,
    flag: &str,
    period: &str,
    redis: &mut Connection
) -> Option<f64> {
    let key = format!("{}_{}_{}", asset, period, flag);
    let ret: RedisResult<f64> = redis.hget(REDIS_OFFSET_KET, key);
    match ret {
        Ok(v) => Some(v),
        _ => None,
    }
}

pub fn read_redis_offset(
    asset: &Asset,
    period: &str,
    redis: &mut Connection,
) -> Option<Vec<f64>> {
    let keys = vec![
        "bid2bid", "bid2ask", "ask2bid", "ask2ask"
    ];
    let mut ret = vec![];
    for key in keys {
        match read_offset_by_key(&asset, key, period, redis) {
            Some (val) => ret.push(val),
            None => return None
        }
    }
    Some(ret)
}

pub fn read_redis_spread(
    asset: &Asset,
    period: &str,
    redis: &mut Connection,
) -> Option<f64> {
    let key = format!("{}_{}", asset, period);
    let ret: RedisResult<f64> = redis.hget(REDIS_SPREAD_KET, key);
    match ret {
        Ok(v) => Some(v),
        _ => None,
    }
}

pub fn read_redis_delay(
    asset: &Asset,
    period: &str,
    redis: &mut Connection,
) -> Option<f64> {
    let key = format!("{}_{}", asset, period);
    let ret: RedisResult<f64> = redis.hget(REDIS_DELAY_KET, key);
    match ret {
        Ok(v) => Some(v),
        _ => None,
    }
}

pub fn read_redis_key (
    key: &str,
    hmap_key: &str,
    redis: &mut Connection,
) -> Option<f64> {
    let ret: RedisResult<f64> = redis.hget(hmap_key, key);
    match ret {
        Ok(v) => Some(v),
        _ => None,
    }
}

pub fn write_redis_batch (
    bucket: &str,
    data_map: HashMap<String, f64>,
    redis: &mut Connection,
) {
    let mut data = vec![];
    for (k, v) in data_map {
        data.push((k, v));
    }
    let _: () = redis.hset_multiple(bucket, &data).unwrap();
}