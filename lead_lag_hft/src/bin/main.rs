use bkbase::utils::rand_id::init_rand_rng;
use bkbase::utils::time::tscns_init;
use lead_lag_hft::offset_taker_strategy::offset_taker_config::OffsetTakerConfig;
use lead_lag_hft::offset_taker_strategy::OffsetTakerStrategy;
use lead_lag_hft::strategy::Strategy;

fn main() {
    tscns_init();
    init_rand_rng();
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_file(true)
        .with_max_level(tracing::Level::INFO)
        .init();
    tracing::info!("hello");

    let mut strategy = Strategy::<OffsetTakerConfig>::new();
    let mut behavior = OffsetTakerStrategy::new();
    strategy.run(&mut behavior).unwrap();
}