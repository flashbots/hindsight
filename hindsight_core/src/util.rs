use ethers::types::H160;

pub fn weth() -> H160 {
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse()
        .unwrap()
}
