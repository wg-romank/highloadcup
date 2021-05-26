Get stub server

```bash
git clone git@github.com:mr2dark/highloadcup.git
```

Build stub server

```bash
docker build -t hlc21_stub_server .
```

Run stub server

```bash
docker run --rm -it -e SERVER_RUN_TIME_IN_SECONDS=60 -p 0.0.0.0:8000:8000 hlc21_stub_server
```

Build & run

```bash
cargo build --release
ADDRESS=localhost ./hlcup/target/release/hlcup
```
