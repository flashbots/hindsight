use ethers::types::{H160, H256};
use lazy_static::lazy_static;
use std::collections::HashMap;

pub type H256Map<T> = HashMap<H256, T>;

lazy_static! {
    pub static ref WETH: H160 = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse()
        .unwrap();
}
