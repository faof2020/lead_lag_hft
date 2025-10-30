use bkbase::utils::rand_id::init_rand_rng;
use bkbase::utils::time::tscns_init;
use lead_lag_hft::new_coin_maker::new_coin_maker_config::NewCoinMakerConfig;
use lead_lag_hft::new_coin_maker::NewCoinMakerStrategy;
use lead_lag_hft::strategy::Strategy;

fn main() {
    tscns_init();
    init_rand_rng();
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_file(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut strategy = Strategy::<NewCoinMakerConfig>::new();
    let mut behavior = NewCoinMakerStrategy::new();
    strategy.run(&mut behavior).unwrap();
}