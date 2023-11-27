use ethers::types::{H160, H256, U256};
use lazy_static::lazy_static;
use revm::primitives::U256 as rU256;
use std::collections::HashMap;

pub type H256Map<T> = HashMap<H256, T>;

lazy_static! {
    pub static ref WETH: H160 = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse()
        .unwrap();
}

pub fn u256_to_ru256(num: U256) -> rU256 {
    let mut bytes: [u8; 32] = [0; 32];
    num.to_big_endian(&mut bytes);
    rU256::from_be_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::{Address, BigEndianHash, H256, U256};
    use revm::primitives::{Address as rAddress, U256 as rU256};
    use std::str::FromStr;

    #[test]
    fn test_u256_to_ru256() {
        let ethers_u256 = ethers::types::U256::from_str("0x1000").unwrap();
        let ru256 = u256_to_ru256(ethers_u256);
        assert_eq!(ru256.to_string(), "4096");
    }

    /*
    The following tests are just idioms for converting between revm and ethers types.
    */

    #[test]
    fn test_h256_to_ru256() {
        let h_be =
            H256::from_str("0x0000000000000000000000000000000000000000000000000000000000001000")
                .unwrap();
        let u = rU256::from_be_slice(&h_be.0);
        assert_eq!(u.to_string(), "4096");

        let h_le =
            H256::from_str("0x0010000000000000000000000000000000000000000000000000000000000000")
                .unwrap();
        let u = rU256::from_le_slice(&h_le.0);
        assert_eq!(u.to_string(), "4096");
    }

    #[test]
    fn test_slices_and_bytes() {
        let u = U256::from_str("0x1000").unwrap();
        let mut x = [0; 32];
        u.to_big_endian(&mut x);

        let ru256 = rU256::from_be_slice(&x);
        assert_eq!(ru256.to_string(), "4096");

        let ru256 = rU256::from_be_bytes(x);
        assert_eq!(ru256.to_string(), "4096");
    }

    #[test]
    fn test_ru256_to_h256() {
        let idx: rU256 = rU256::from_str("0x1000").unwrap();
        assert_eq!(
            H256::from_uint(&U256::from_big_endian(&idx.to_be_bytes::<32>())).to_low_u64_be(),
            4096
        )
    }

    #[test]
    fn test_address_to_raddress() {
        let address = rAddress::from_str("0x1113111311131113111311131113111311131113").unwrap();
        let address_ethers: Address = ethers::types::H160((address.0).0);
        assert_eq!(
            address_ethers,
            Address::from_str("0x1113111311131113111311131113111311131113").unwrap()
        );
    }
}
