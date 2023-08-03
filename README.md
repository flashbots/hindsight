# Hindsight

Hindsight is an arbitrage simulator written in Rust which analyzes the historical value of MEV from Flashbots MEV-Share events.

revm is used to simulate arbs with the help of an Ethereum archive node that supports the `trace_callMany` API (see [requirements](#requirements) for node recommendations).

> Just a warning: ⚠️ running Hindsight on a hosted node may require a high rate limit, which can be expensive.

The arbitrage strategy implemented here is a relatively simple two-step arb: first, we simulate the user's trade, then swap WETH for tokens on the exchange with the best rate (with the user's trade accounted for) and sell them on whichever supported exchange gives us the best rate. Currently, Uniswap V2/V3 and SushiSwap are supported. More may be added to improve odds of profitability.

Simulated arbitrage attempts are saved in a MongoDB database, for dead-simple storage that allows us to change our data format as needed with no overhead.

## setup

### requirements

- [docker](https://www.docker.com/get-started/) (tested with v24.0.3)
- ethereum archive node supporting [`trace_callMany`](https://openethereum.github.io/JSONRPC-trace-module#trace_callmany) API (Reth or Erigon or Infura)
  - [Erigon](https://github.com/ledgerwatch/erigon) and [Reth](https://github.com/paradigmxyz/reth) are good self-hosted options.
  - [Infura](https://www.infura.io/solutions/archive-access) and [QuickNode](https://www.quicknode.com/core-api) offer hosted solutions (make sure you get an "archive node" plan if prompted for it).

  > The default environment (specified in [`.env.example`](.env.example)) assumes that you have an Ethereum node accessible on `ws://localhost:8545`.

**To build and run locally:**

- [rust](https://www.rust-lang.org/learn/get-started) (tested with rustc 1.70.0)

### spin up DB

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
docker run -it -e RPC_URL_WS=ws://host.docker.internal:8545 -e DB_URL=mongodb://host.docker.internal:27017 hindsight --help
```

> :information_source: From this point on, I'll use `hindsight` to refer to whichever method you choose to run the program. So `hindsight scan --help` would translate to `cargo run -- scan --help` or `docker run -it hindsight --help` or `./target/debug/hindsight --help`.

### (optional) test

All the tests are integration tests, so you'll have to have your environment (DB & ETH provider) set up to run them successfully.

```sh
export RPC_URL_WS=ws://127.0.0.1:8545
export DB_URL=mongodb://localhost:27017
cargo test
```

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
DB_URL=mongodb://host.docker.internal:27017
```

Some linux machines don't like names; you may try this instead:

```txt
RPC_URL_WS=ws://172.17.0.1:8545
DB_URL=mongodb://172.17.0.1:27017
```

#### .env vs environment variables

`.env` is optional. If you prefer, you can set environment variables directly in your shell:

```sh
export RPC_URL_WS=ws://127.0.0.1:8545
export DB_URL=mongodb://localhost:27017
cargo run -- scan

# alternatively, to pass the variables directly to hindsight rather than setting them in the shell
RPC_URL_WS=ws://127.0.0.1:8545 \
DB_URL=mongodb://localhost:27017 \
cargo run -- scan
```

## `scan`

The `scan` command is the heart of Hindsight. It scans events from the MEV-Share Event History API, then fetches the full transactions of those events from the blockchain to use in simulations. The system then forks the blockchain at the block in which each transaction landed, and runs a recursive quadratic search to find the optimal amount of WETH to execute a backrun-arbitrage. The results are then saved to the database in the `arbs` collection ("collection" is MongoDB's term for a table).

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

# or if you don't have bc
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
docker run -it -v $(pwd)/arbData:/app/arbData -e RPC_URL_WS=ws://host.docker.internal:8545 -e DB_URL=mongodb://host.docker.internal:27017 hindsight export -p 0.0001
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
