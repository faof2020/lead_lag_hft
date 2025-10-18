use std::collections::HashMap;
use redis::{Client, Connection};
use crate::utils::redis_util::write_redis_batch;

pub struct RedisReporter {
    redis_conn: Connection,
    cache: HashMap<String, HashMap<String, f64>>,
    last_update_map: HashMap<String, u64>,
    max_update_intval: u64,
    max_update_length: usize,
}

impl RedisReporter {
    pub fn new(url: &str) -> Self {
        let client = Client::open(url).unwrap();
        RedisReporter {
            redis_conn: client.get_connection().unwrap(),
            cache: HashMap::new(),
            last_update_map: HashMap::new(),
            max_update_intval: 1000 * 60 * 10,
            max_update_length: 1000,
        }
    }

    pub fn record(&mut self, bucket: &str, key: &str, val: f64, now_ms: u64) {
        if !self.cache.contains_key(bucket) {
            self.cache.insert(bucket.to_string(), HashMap::new());
        }
        let bucket_map = self.cache.get_mut(bucket).unwrap();
        bucket_map.insert(key.to_string(), val);
        let mut need_upload = false;
        if bucket_map.len() > self.max_update_length {
            need_upload = true;
        } else if !self.last_update_map.contains_key(bucket) {
            need_upload = true;
        } else if self.last_update_map.get(bucket).unwrap() + self.max_update_intval < now_ms {
            need_upload = true;
        }
        if need_upload {
            write_redis_batch(bucket, bucket_map.clone(), &mut self.redis_conn);
            bucket_map.clear();
            self.last_update_map.insert(bucket.to_string(), now_ms);
        }
    }

}