# Hindsight

<!-- show ./header-img.png -->
![hindsight visual algorithm](header-img.png)

_Hindsight is an arbitrage simulator written in Rust which estimates the historical value of (Uniswap) MEV from Flashbots MEV-Share events._

The simulation core uses [revm](https://github.com/bluealloy/revm) to simulate arbs locally by first getting state diffs from an Ethereum archive node that supports the `trace_callMany` API (see [requirements](#requirements) for node recommendations).

> Just a warning: ⚠️ running Hindsight on a hosted node may require a high rate limit, which can be expensive.

The arbitrage strategy implemented here is a relatively simple two-step arb: after simulating the user's trade, we simulate swapping WETH for tokens on the exchange with the best rate (with the user's trade accounted for) and then simulate selling them on whichever other supported exchange gives us the best rate. Currently, Uniswap V2/V3 and SushiSwap are supported. More exchanges may be added to improve odds of profitability.

Simulated arbitrage attempts are saved in a MongoDB database by default, for dead-simple storage that allows us to change our data format as needed with no overhead. Postgres is also supported, but does not currently save all the same fields that Mongo does.

## ⚠️ limitations ⚠️

Hindsight is still in development, and is not stable. If you find a bug, please [open an issue](https://github.com/zeroXbrock/hindsight/issues).

This project is an experiment. The profits estimated by this system are by no means definitive; they more accurately represent a **lower bound** for the total addressable MEV on MEV-Share. With more complex strategies and more exchanges supported, total profits which could be realized on MEV-Share should far exceed those which are estimated by this system.

This system implements a decidedly simple strategy to estimate a baseline amount of MEV exposed by a few well-known exchanges in the context of MEV-Share. It does not account for many factors that would affect the profitability of an arb, such as gas prices or placement in the block. This system also ignores multiple-hop arbitrage paths, which would improve profits considerably. It also ignores Balancer and Curve trades, which are supported by MEV-Share.

The system currently only supports Uniswap V2/V3 and SushiSwap. More exchanges may be added in the future, which should improve profitability.

The system currently only supports WETH as the input token, so that the arbitrage is always WETH -> TOKEN -> WETH.

The system (the `scan` command specifically) is set up to retry indefinitely when the main loop crashes. This is because every once in a while, the system encounters a critical error, usually related to a bad API response. This is not ideal, but a retry usually fixes it. However, this means that your instance might spam your node with requests if it encounters an unrecoverable error. If you're running on a hosted node, this could waste your rate limit. Make sure to check on it while it's running. 👁️

## setup

Make sure to clone the repo with `--recurse-submodules`. At least for now, we depend on a specific commit of rusty-sando, for its very-useful ForkDB.

```bash
git clone --recurse-submodules https://github.com/flashbots/hindsight
```

or if you already cloned without recursing submodules:

```bash
# in hindsight/
git submodule update --init
```

### 🚧 DB implementation incomplete 🚧

The system defaults to using mongo as the database to store arb simulation results. Postgres can be used (add `--help` to any command for details) but currently it only stores `tx_hash`, `event_block`, `event_timestamp`, and `profit`, whereas mongo stores all event and arbitrage trade data. Postgres functionality may be improved later on.

### requirements

- ethereum archive node supporting [`trace_callMany`](https://openethereum.github.io/JSONRPC-trace-module#trace_callmany) API (Reth or Erigon or Infura)
  - [Erigon](https://github.com/ledgerwatch/erigon) and [Reth](https://github.com/paradigmxyz/reth) are good self-hosted options.
  - [Infura](https://www.infura.io/solutions/archive-access) and [QuickNode](https://www.quicknode.com/core-api) offer hosted solutions (make sure you get an "archive node" plan if prompted for it).

  > The default environment (specified in [`.env.example`](.env.example)) assumes that you have an Ethereum node accessible on `ws://localhost:8545`.

### To build and run locally

_Either/Or:_

- [rust](https://www.rust-lang.org/learn/get-started) (tested with rustc 1.70.0)
- [docker](https://www.docker.com/get-started/) (tested with v24.0.3)

### populate environment variables

If you want to set your environment variables in a file, copy the template file `.env.example` to `.env` and update as needed.

```sh
cp .env.example .env
# modify in your preferred editor
vim .env
```

The values present in `.env.example` will work if you run hindsight locally, but if you're using docker, you'll have to change the values to reflect the host in the context of the container.

With the DB and Ethereum RPC accessible on the host machine:

*Docker .env config:*

```txt
RPC_URL_WS=ws://host.docker.internal:8545
MONGO_URL=mongodb://root:example@host.docker.internal:27017
POSTGRES_URL=postgres://postgres:adminPassword@host.docker.internal:5432

```

Some docker installations on linux don't support `host.docker.internal`; you may try this instead:

```txt
RPC_URL_WS=ws://172.17.0.1:8545
MONGO_URL=mongodb://root:example@172.17.0.1:27017
POSTGRES_URL=postgres://postgres:adminPassword@172.17.0.1:5432
```

#### .env vs environment variables

`.env` is optional. If you prefer, you can set environment variables directly in your shell:

```sh
export RPC_URL_WS=ws://127.0.0.1:8545
export MONGO_URL=mongodb://root:example@localhost:27017
export POSTGRES_URL=postgres://postgres:adminPassword@localhost:5432
cargo run -- scan

# alternatively, to pass the variables directly to hindsight rather than setting them in the shell
RPC_URL_WS=ws://127.0.0.1:8545 \
MONGO_URL=mongodb://root:example@localhost:27017 \
POSTGRES_URL=postgres://postgres:adminPassword@localhost:5432 \
cargo run -- scan
```

### system dependencies

```sh
# Debian/Ubuntu
sudo apt install build-essential libssl-dev pkg-config
```

### TLS for AWS DocumentDB (optional; only used for cloud DBs)

Get the CA file:

```sh
./get-ca.sh
```

Enable TLS for the db by modifying your `.env` file:

- uncomment and set `TLS_CA_FILE_MONGO`
- add `?tls=true` to your existing `MONGO_URL`

`.env`

```txt
TLS_CA_FILE_MONGO=global-bundle.pem
MONGO_URL=mongodb://root:example@localhost:27017/?tls=true
```

### run DB locally w/ docker

```sh
docker compose up -d
```

If you like, you can browse the database in your web browser here: [http://localhost:8081/](http://localhost:8081). Note that there won't be any interesting data in it until you run the [`scan`](#scan) command.

### build and run

**Locally:**

```sh
# compile it
cargo build

# run with cargo
cargo run -- --help

# or run the binary directly
./target/debug/hindsight --help
```

**With Docker:**

```sh
docker build -t hindsight .
docker run -it -e RPC_URL_WS=ws://host.docker.internal:8545 -e MONGO_URL=mongodb://host.docker.internal:27017 hindsight --help
```

> :information_source: From this point on, I'll use `hindsight` to refer to whichever method you choose to run the program. So `hindsight scan --help` would translate to `cargo run -- scan --help` or `docker run -it hindsight --help` or `./target/debug/hindsight --help`.

### (optional) test

All the tests are integration tests, so you'll have to have your environment (DB & ETH provider) set up to run them successfully.

```sh
export RPC_URL_WS=ws://127.0.0.1:8545
export MONGO_URL=mongodb://localhost:27017
cargo test
```

## `scan`

The `scan` command is the heart of Hindsight. It scans events from the MEV-Share Event History API, then fetches the full transactions of those events from the blockchain to use in simulations. The system then forks the blockchain at the block in which each transaction landed, and runs an [arbitrarily](./src/sim/core.rs#L28)-[juiced quadratic search](https://research.ijcaonline.org/volume65/number14/pxc3886165.pdf) to find the optimal amount of WETH to execute a backrun-arbitrage. The results are then saved to the database.

To scan the last week's events for arbs:

```sh
hindsight scan -t $(echo "$(date +%s) - (86400 * 7)" | bc)

# or if you don't have `bc` and can accept these ugly parenthesis
hindsight scan -t $(echo $(($(date +%s) - ((86400 * 7)))))
```

The timestamp arguments accept unix-style integer timestamps, represented in seconds.

## `export`

The `export` command is a simple way to filter and export results from the database into a JSON file.

To export arbs for events from the last week:

```sh
hindsight export -t $(echo "$(date +%s) - (86400 * 7)" | bc)

# or
hindsight export -t $(echo $(($(date +%s) - ((86400 * 7)))))
```

To filter out unprofitable results:

```sh
# only export arbs that returned a profit of at least 0.0001 WETH
hindsight export -p 0.0001
```

### exporting with docker

Hindsight exports all files into a directory `./arbData`, relative to wherever the program is executed. To get these files out of the docker container and on to your host machine, you'll need to map the volume to a local directory.

In the directory where you want to put the files (we make an `arbData` directory but you don't have to):

```sh
mkdir -p arbData
docker run -it -v $(pwd)/arbData:/app/arbData -e RPC_URL_WS=ws://host.docker.internal:8545 -e MONGO_URL=mongodb://host.docker.internal:27017 hindsight export -p 0.0001
```

## common errors

### error: "too many open files"

The `scan` command can spawn a lot of threads. You may need to increase the open file limit on your system to ensure reliable operation.

```sh
# check current open file limit
ulimit -Sn

# raise the limit if needed
ulimit -n 4000

# be careful, this can cause system-wide issues if you set it too high
ulimit -n 9001
```

Alternatively, you can run the `scan` command with less parallel operations:

```sh
# only process two txs at a time
hindsight scan -n 2
```

### Error: Kind: Server selection timeout: No available servers

... `Topology: { Type: Unknown, Servers: [ { Address: host.docker.internal:27017, Type: Unknown, Error: Kind: I/O error: failed to lookup address information: Name or service not known, labels: {} } ] }, labels: {}`

This means that your system doesn't support the `host.docker.internal` mapping. Try replacing `host.docker.internal` with `172.17.0.1`.

### Error: IO error: Connection refused (os error 111)

This means that either your ETH node or your DB is not properly connected.

Make sure your DB is running with `docker ps` (you should see `mongo` and `mongo-express` running). If they're not running, try this:

```sh
docker compose down
docker compose up
```

If your DB is running, make sure your node is running and accessible from your host. A [simple JSON-RPC request with curl](https://ethereum.org/en/developers/docs/apis/json-rpc/#net_version) is the simplest way to test this.

```sh
curl -X POST --data '{"jsonrpc":"2.0","method":"net_version","params":[],"id":42}'
```

If that doesn't work, try double-checking your URLs. Refer back to the [environment instructions](#populate-environment-variables) if you're lost.

## acknowledgements

- [rusty-sando](https://github.com/mouseless-eth/rusty-sando)
- [mev-inspect-rs](https://github.com/flashbots/mev-inspect-rs)
- [mev-inspect-py](https://github.com/flashbots/mev-inspect-py)
- [nitepunk](https://soundcloud.com/nitepunk/sets/slices)

## future improvements

See [issues](https://github.com/flashbots/hindsight/issues) for the most up-to-date status, or to propose an improvement!

- [ ] support all the fields in postgres, then make postgres the default
- [ ] replace [ForkDB dependency](./src/sim/core.rs#L22-L23) (possibly with [Arbiter](https://github.com/primitivefinance/arbiter))
- [ ] add more protocols (currently only support UniV2, UniV3, and Sushiswap)
- [ ] maybe: add more complex strategies
  - multi-hop arbs
  - multi-tx backruns (using mempool txs)
  - stat arb
  - so many more...
