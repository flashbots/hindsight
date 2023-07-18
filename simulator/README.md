# hindsight

## setup

### spin up DB

```sh
docker compose up -d
```

### build and run

```sh
cargo build
./target/debug/hindsight --help
```

## error: "too many open files"

This thing spawns lots of threads. You may need to increase the open file limit on your system to ensure reliable operation.

```sh
# check current open file limit
ulimit -Sn

# run this only if the value returned is lower than 8000
ulimit -n 8000
```
