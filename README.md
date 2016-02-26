Honeysearch
===========
This service will be the endpoint responsible for Honeypot's ElasticSearch data.

Currently a really WIP. Just [biliv](https://just-believe.in).

Setup
-----
Install the latest stable release of Rust using the [official installer](https://www.rust-lang.org/downloads.html) or your package manager.

Then clone locally this repository and run the executable with

```sh
$ cargo run [examples/config.toml]
````

You can produce an optimized executable just appending `--release`, but the compile time will be longer.

Run `cargo test` to run the tests and `cargo doc` to produce the documentation.

Performance
-----------
```
┌[giovanni@lifestream] [/dev/ttys001] [master]
└[~/Desktop/honeysearch]> wrk -t12 -c400 -d30s "http://127.0.0.1:3000/talents?work_roles[]=DevOps"
Running 30s test @ http://127.0.0.1:3000/talents?work_roles[]=DevOps
  12 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    56.23ms   38.46ms 314.68ms   94.36%
    Req/Sec   152.72    143.05   575.00     78.06%
  17050 requests in 30.08s, 2.89MB read
  Socket errors: connect 0, read 361, write 0, timeout 0
Requests/sec:    566.84
Transfer/sec:     98.53KB
```
