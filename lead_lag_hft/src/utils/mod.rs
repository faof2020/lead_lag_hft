pub mod redis_util;
pub mod bk_util;

pub fn get_period_ms(intval: &str) -> u64 {
    if intval.ends_with("S") {
        intval.replace("S", "").parse::<u64>().unwrap() * 1000
    } else if intval.ends_with("M") {
        intval.replace("M", "").parse::<u64>().unwrap() * 1000 * 60
    } else if intval.ends_with("H") {
        intval.replace("H", "").parse::<u64>().unwrap() * 1000 * 60 * 60
    } else if intval.ends_with("D") {
        intval.replace("D", "").parse::<u64>().unwrap() * 1000 * 60 * 60 * 24
    } else {
        panic!("Invalid intval")
    }
}