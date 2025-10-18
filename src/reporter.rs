use std::collections::HashMap;
use std::mem;
use async_trait::async_trait;
use bkbase::models::{Asset, CURRENCY_USDT};
use bklib::excenter::prelude::{ExCenter, EX_DATA_REPORTER};
use bklib::legacy::BkLegacyClient;
use bklib::legacy::handler::BkLegacyRawHandler;
use bklib::legacy::proto::{BkLegacyRequest, BkLegacyRequestReportCustomData, BkLegacyResponse};
use serde_json::Value;
use anyhow::Result;

pub const BATCH_REPORT_REQ_TYPE_ID: u64 = 10;

pub struct BatchReportHandler {}

#[derive(Debug)]
pub struct BkLegacyRequestBatchReportCustomData {
    items: Vec<BkLegacyRequestReportCustomData>,
}

#[async_trait]
impl BkLegacyRawHandler for BatchReportHandler {
    async fn on_request(
        &mut self,
        request: BkLegacyRequest,
        _: &ExCenter,
    ) -> Result<Option<BkLegacyResponse>> {
        match request {
            BkLegacyRequest::Raw(req_type, raw_ptr) => {
                if req_type == BATCH_REPORT_REQ_TYPE_ID {
                    let box_data = unsafe {
                        Box::from_raw(raw_ptr.unwrap() as *mut BkLegacyRequestBatchReportCustomData)
                    };
                    for data in box_data.items {
                        EX_DATA_REPORTER.record_custom_data(
                            &data.measurement,
                            data.field_data,
                            data.tag_data,
                            &data.instance_id,
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }
}

pub struct Reporter {
    instance_id: String,
    global_report_ms: u64,
    global_report_intval: u64,
    custom_batch_report_ms_map: HashMap<String, u64>,
    custom_batch_report_intval: u64,
    custom_single_report_ms: u64,
    custom_single_report_intval: u64,
    custom_batch_data_cache: HashMap<String, HashMap<Asset, HashMap<String, Value>>>,
    custom_single_data_cache: Vec<BkLegacyRequestReportCustomData>,
}

impl Reporter {

    pub fn new(instance_id: &str) -> Self {
        Reporter {
            instance_id: instance_id.to_string(),
            global_report_ms: 0,
            global_report_intval: 3000,
            custom_batch_report_ms_map: HashMap::new(),
            custom_batch_report_intval: 1000,
            custom_single_report_ms: 0,
            custom_single_report_intval: 1000,
            custom_batch_data_cache: HashMap::new(),
            custom_single_data_cache: Vec::new(),
        }
    }

    pub fn report_global(&mut self, legacy: &mut BkLegacyClient, now_ms: u64) {
        if self.global_report_ms + self.global_report_intval <= now_ms {
            let box_data = Box::new(CURRENCY_USDT);
            legacy.send_message(BkLegacyRequest::ReportGlobalSummary(box_data));
            self.global_report_ms = now_ms;
        }
    }

    pub fn add_custom_batch_report_data(
        &mut self,
        measurement: &str,
        asset: &Asset,
        data: HashMap<String, Value>,
        legacy: &mut BkLegacyClient,
        now_ms: u64)
    {
        if !self.custom_batch_data_cache.contains_key(measurement) {
            self.custom_batch_data_cache.insert(measurement.to_string(), HashMap::new());
        }
        let measurement_map = &mut self.custom_batch_data_cache.get_mut(measurement).unwrap();
        if !measurement_map.contains_key(asset) {
            measurement_map.insert(asset.clone(), data);
        } else {
            let asset_map = measurement_map.get_mut(asset).unwrap();
            for (k, v) in data {
                asset_map.insert(k, v);
            }
        }
        self.batch_report_custom_data(measurement, legacy, now_ms);
    }

    pub fn add_custom_single_report_data(
        &mut self,
        measurement: &str,
        tag: HashMap<String, String>,
        data: HashMap<String, Value>,
        legacy: &mut BkLegacyClient,
        now_ms: u64)
    {
        self.custom_single_data_cache.push(BkLegacyRequestReportCustomData {
            instance_id: self.instance_id.to_string(),
            measurement: measurement.to_string(),
            field_data: data,
            tag_data: tag,
        });
        self.single_report_custom_data(legacy, now_ms);
    }

    pub fn single_report_custom_data(&mut self, legacy: &mut BkLegacyClient, now_ms: u64) {
        if self.custom_single_report_ms + self.custom_single_report_intval <= now_ms {
            let box_data = Box::new(
                BkLegacyRequestBatchReportCustomData { items: mem::take(&mut self.custom_single_data_cache) }
            );
            legacy.send_message(BkLegacyRequest::Raw(
                BATCH_REPORT_REQ_TYPE_ID,
                Some(Box::into_raw(box_data) as u64),
            ));
            self.custom_single_report_ms = now_ms;
        }
    }

    pub fn batch_report_custom_data(&mut self, measurement: &str, legacy: &mut BkLegacyClient, now_ms: u64) {
        if !self.custom_batch_report_ms_map.contains_key(measurement) {
            self.custom_batch_report_ms_map.insert(measurement.to_string(), 0);
        }
        let last_report_ms = self.custom_batch_report_ms_map.get(measurement).unwrap();
        if last_report_ms + self.custom_batch_report_intval <= now_ms {
            if !self.custom_batch_data_cache.contains_key(measurement) {
                return;
            }
            let asset_map = self.custom_batch_data_cache.get(measurement).unwrap();
            let mut data = vec![];
            for (asset, asset_data) in asset_map {
                data.push(BkLegacyRequestReportCustomData {
                    instance_id: self.instance_id.to_string(),
                    measurement: measurement.to_string(),
                    field_data: asset_data.clone(),
                    tag_data: HashMap::from([("asset".to_string(), asset.to_string())]),
                });
            }
            let box_data = Box::new(
                BkLegacyRequestBatchReportCustomData { items: data }
            );
            legacy.send_message(BkLegacyRequest::Raw(
                BATCH_REPORT_REQ_TYPE_ID,
                Some(Box::into_raw(box_data) as u64),
            ));
            self.custom_batch_data_cache.insert(measurement.to_string(), HashMap::new());
            self.custom_batch_report_ms_map.insert(measurement.to_string(), now_ms);
        }
    }
}