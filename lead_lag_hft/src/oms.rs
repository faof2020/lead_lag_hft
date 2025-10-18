use std::cell::RefMut;
use bkbase::models::{Asset, OrderData, OrderID, OrderRequest, OrderType};
use bklib::private::order::{BkPrivateOrderContext, OrderDataExtra};
use std::collections::HashMap;
use crate::common_config::CommonConfig;
use anyhow::{anyhow, Result};
use bklib::BkPrivateClient;
use bklib::private::BkPrivateOrderCancelPriority;

#[derive(Debug, Clone)]
pub struct TakerContext {
    pub asset: Asset,
    pub price: Option<f64>,
    pub size: f64,
    pub is_market: bool,
    pub max_usd_pos: f64,
    pub now_ms: u64,
}

// #[derive(Debug, Clone)]
// pub struct MakerContext {
//     pub asset: Asset,
//     pub price: f64,
//     pub size: f64,
//     pub is_post_only: bool,
//     pub is_first: bool,
//     pub max_order_num: usize,
//     pub order_distance_threshold: f64,
//     pub max_usd_pos: f64,
//     pub now_ms: u64,
// }

pub struct Oms {
    asset: Asset,
    open_bids: HashMap<OrderID, OrderData>,
    open_asks: HashMap<OrderID, OrderData>,
    pendings: HashMap<OrderID, OrderDataExtra>,
    canceling: HashMap<OrderID, u64>,
    pub current_usd_position: Option<f64>,
    pub virtual_usd_position: Option<f64>,
    last_quote_ms: u64,
    quote_intval: u64,
    trading: bool,
}

impl Oms {
    pub fn new<T>(asset: &Asset, trading: bool, config: &CommonConfig<T>) -> Oms {
        Oms {
            asset: asset.clone(),
            open_bids: HashMap::new(),
            open_asks: HashMap::new(),
            pendings: HashMap::new(),
            canceling: HashMap::new(),
            current_usd_position: None,
            virtual_usd_position: None,
            last_quote_ms: 0,
            quote_intval: config.quote_intval,
            trading,
        }
    }

    pub fn sync_position_and_orders(
        &mut self,
        current_pos: f64,
        virtual_pos: f64,
        opens: HashMap<OrderID, OrderData>,
        pendings: HashMap<OrderID, OrderDataExtra>,
        canceling: HashMap<OrderID, u64>,
    ) {
        self.open_asks.clear();
        self.open_bids.clear();
        if opens.len() > 0 {
            for (id, order_data) in opens.iter() {
                if order_data.size > 0f64 {
                    self.open_bids.insert(id.clone(), order_data.clone());
                } else {
                    self.open_asks.insert(id.clone(), order_data.clone());
                }
            }
        }
        self.canceling = canceling;
        self.pendings = pendings;
        self.current_usd_position = Some(current_pos);
        self.virtual_usd_position = Some(virtual_pos);
    }

    pub fn position_check(&self, size: f64, max_usd_pos: f64) -> (bool, Vec<OrderID>) {
        let usd_position = self.virtual_usd_position.unwrap();
        let mut cancel_list = vec![];
        let mut should_post = true;
        // 如果仓位打满，对应方向全撤，并不再挂单
        if size > 0f64 && usd_position > max_usd_pos {
            for (oid, _) in self.open_bids.iter() {
                cancel_list.push(oid.clone());
            }
            should_post = false;
        } else if size < 0f64 && usd_position < -max_usd_pos {
            for (oid, _) in self.open_asks.iter() {
                cancel_list.push(oid.clone());
            }
            should_post = false;
        }
        (should_post, cancel_list)
    }

    fn oms_is_ready(&self) -> bool {
        if self.pendings.len() > 0 {
            return false;
        }
        if self.canceling.len() > 0 {
            return false;
        }
        if self.virtual_usd_position.is_none() {
            return false;
        }
        true
    }

    fn is_post_order_safe(&self, order_ctx: &RefMut<BkPrivateOrderContext>, now_ms: u64) -> bool {
        if self.last_quote_ms + self.quote_intval > now_ms {
            return false;
        }
        if !self.trading {
            return false;
        }
        if !order_ctx.is_safe_to_post_order() {
            return false;
        }
        true
    }

    pub fn do_taker(
        &mut self, taker: TakerContext,
        mut order_ctx: RefMut<BkPrivateOrderContext>,
        private_client: &mut BkPrivateClient,
    ) -> Result<()> {
        if !self.asset.eq(&taker.asset) {
            return Err(anyhow!("oms: {:?} not match taker: {:?}", self.asset, taker.asset));
        }
        if !taker.is_market && taker.price.is_none() {
            return Err(anyhow!("taker order price is none while is not market order: {:?}", taker));
        }
        if !self.oms_is_ready() {
            return Ok(());
        }
        let (should_post, cancel_list) = self.position_check(taker.size, taker.max_usd_pos);
        for id in cancel_list {
            let _ = order_ctx.cancel_order(id, BkPrivateOrderCancelPriority::Normal, private_client);
        }
        if !should_post {
            return Ok(());
        }
        if !self.is_post_order_safe(&order_ctx, taker.now_ms) {
            return Ok(())
        }
        let mut req = OrderRequest::new(self.asset.clone(), taker.price, taker.size);
        req.order_type = if taker.is_market {
            OrderType::MARKET
        } else {
            OrderType::IOC
        };
        order_ctx.post_order(req, private_client);
        self.last_quote_ms = taker.now_ms;
        Ok(())
    }

}