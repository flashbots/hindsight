use std::collections::HashMap;

use ethers::types::{H160, H256};

pub fn weth() -> H160 {
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse()
        .unwrap()
}

pub type H256Map<T> = HashMap<H256, T>;
