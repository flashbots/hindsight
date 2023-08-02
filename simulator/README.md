# Hindsight

Hindsight is a simulation-based arbitrage simulator written in Rust which analyzes the historical value of MEV from Flashbots MEV-Share events.

revm is used to simulate arbs, with the help of an archive node that supports the `trace_callMany` API (such as [Erigon](https://github.com/ledgerwatch/erigon) or [Reth](https://github.com/paradigmxyz/reth)).

The arbitrage strategy implemented here is a relatively simple two-hop arb: we swap WETH for tokens on the exchange with the best rate (with the user's trade accounted for) and sell them on whichever supported exchange gives us the best rate. Currently, Uniswap V2/V3 and SushiSwap are supported. More may be added to improve odds of profitability.

Simulated arbitrage attempts are saved in a local MongoDB database, for dead-simple storage that allows us to change our data format as needed with no overhead.

## setup

### requirements

- [docker](https://www.docker.com/get-started/) (tested with v24.0.3)
- [rust](https://www.rust-lang.org/learn/get-started)
- ethereum archive node supporting [`trace_callMany`](https://openethereum.github.io/JSONRPC-trace-module#trace_callmany) API (Reth or Erigon or Infura)

This thing spawns lots of threads. You may need to increase the file limit for your session.

```sh
# check current open file limit
ulimit -Sn

# run this only if the value returned is lower than 4000
ulimit -n 4000
```

### spin up DB

```sh
docker compose up -d
```

If you like, you can browse the database in your web browser here: [http://localhost:8081/](http://localhost:8081). Note that there won't be any interesting data in it until you run the [`scan`](#scan) command.

### build and run

```sh
# compile it
cargo build

# run with cargo
cargo run -- --help

# or run the binary directly
./target/debug/hindsight --help
```

## scan

The `scan` command is the heart of Hindsight. It scans events from the MEV-Share Event History API, then fetches the full transactions of those events from the blockchain to use in simulations. The system then forks the blockchain at the block in which each transaction landed, and runs a recursive quadratic search to find the optimal amount of WETH to execute a backrun-arbitrage. The results are then saved to the database in the `arbs` collection ("collection" is MongoDB's term for a table).

To scan the last week's events for arbs:

```sh
cargo run -- scan -t $(echo "$(date +%s) - (86400 * 7)" | bc)

# or if you don't have `bc` and can accept these ugly parenthesis
cargo run -- scan -t $(echo $(($(date +%s) - ((86400 * 7)))))
```

The timestamp arguments accept unix-style integer timestamps, represented in seconds.

## export

The `export` command is a simple way to filter and export results from the database into a JSON file.

To export arbs for events from the last week:

```sh
cargo run -- export -t $(echo "$(date +%s) - (86400 * 7)" | bc)

# or if you don't have bc
cargo run -- export -t $(echo $(($(date +%s) - ((86400 * 7)))))
```

To filter out unprofitable results:

```sh
# only export arbs that returned a profit of at least 0.0001 WETH
cargo run -- export -p 0.0001
```

## common errors

### error: "too many open files"

This thing spawns lots of threads. You may need to increase the open file limit on your system to ensure reliable operation.

```sh
# check current open file limit
ulimit -Sn

# raise the limit if needed
ulimit -n 4000
```

## acknowledgements

- [rusty-sando](https://github.com/mouseless-eth/rusty-sando)
- [mev-inspect-rs](https://github.com/flashbots/mev-inspect-rs)
- [mev-inspect-py](https://github.com/flashbots/mev-inspect-py)
