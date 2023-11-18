// SPDX-License-Identifier: MIT
pragma solidity >=0.6.6;

/*
from [rusty-sando](https://github.com/mouseless-eth/rusty-sando/blob/8bed4dbc27e8dac5c1f38cff595bdc082f1f892b/contract/src/lib/SafeMath.sol)
*/

// a library for performing overflow-safe math, courtesy of DappHub (https://github.com/dapphub/ds-math)

library SafeMath {
    function add(uint x, uint y) internal pure returns (uint z) {
        require((z = x + y) >= x, "ds-math-add-overflow");
    }

    function sub(uint x, uint y) internal pure returns (uint z) {
        require((z = x - y) <= x, "ds-math-sub-underflow");
    }

    function mul(uint x, uint y) internal pure returns (uint z) {
        require(y == 0 || (z = x * y) / y == x, "ds-math-mul-overflow");
    }
}
