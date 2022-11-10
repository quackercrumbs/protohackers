https://protohackers.com/problem/2

Your friendly neighbourhood investment bank is having trouble analysing historical price data. They need you to build a TCP server that will let clients insert and query timestamped prices.
Overview

Clients will connect to your server using TCP. Each client tracks the price of a different asset. Clients send messages to the server that either insert or query the prices.

Each connection from a client is a separate session. Each session's data represents a different asset, so each session can only query the data supplied by itself.
