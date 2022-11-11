```
// Only run one thread, any test involving server have a race condition for the host:port used in the app.
// Since test threads defaults to number of CPU cores, tests can run in parallel.
// Ideally, we either spin up the server once, or we dynamically choose ports

cargo test -j 1 -- --test-threads 1

```

https://protohackers.com/problem/2

Your friendly neighbourhood investment bank is having trouble analysing historical price data. They need you to build a TCP server that will let clients insert and query timestamped prices.
Overview

Clients will connect to your server using TCP. Each client tracks the price of a different asset. Clients send messages to the server that either insert or query the prices.

Each connection from a client is a separate session. Each session's data represents a different asset, so each session can only query the data supplied by itself.
