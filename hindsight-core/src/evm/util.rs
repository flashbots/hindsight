use super::fork_db::ForkDB;
use crate::interfaces::BlockInfo;
use revm::{
    primitives::{Address as rAddress, U256 as rU256},
    EVM,
};
use std::str::FromStr;

pub fn set_block_state(evm: &mut EVM<ForkDB>, block_info: &BlockInfo) {
    evm.env.block.number = rU256::from(block_info.number.as_u64());
    evm.env.block.timestamp = rU256::from(block_info.timestamp.as_u64());
    let mut basefee_slice = [0u8; 32];
    block_info
        .base_fee_per_gas
        .to_big_endian(&mut basefee_slice);
    evm.env.block.basefee = rU256::from_be_bytes(basefee_slice);
    // use something other than default
    evm.env.block.coinbase =
        rAddress::from_str("0xC0ffeeFee15BAD00000000000000000000000000").unwrap();
}
