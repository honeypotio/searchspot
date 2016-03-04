Searchspot
==========
This service is used as endpoint responsible for Honeypot's ElasticSearch data.
For this first iteration it only reads and returns the data based on given filters but the plan is to implement the population of the indexes as well.

Our hope is to make this service usable by everyone who needs a search engine with a more-or-less complex system of data filtering (including string matching, dates, booleans and full text search).

Every kind of contribution is more than well accepted!

Things that are missing
-----------------------
Basically at the moment we cannot map the types and the analyzers.
Please check the details [here](https://github.com/benashford/rs-es/issues/11).

Also we need to implement a proper pagination and bulk indexing.

Setup
-----
Install the latest stable release of Rust using the [official installer](https://www.rust-lang.org/downloads.html) or your package manager (i.e.: `brew install rust`).

Then clone this repository to your computer and run the executable with

```sh
$ cargo run examples/default.toml
````

You can produce an optimized executable just appending `--release`, but the compile time will be longer.

You can execute `$ cargo test` to run the tests and `$ cargo doc` to produce the documentation.

Heroku
------
To deploy this application on Heroku, just run

```sh
$ heroku create my-searchspot --buildpack https://github.com/Hoverbear/heroku-buildpack-rust
$ heroku ps:scale web=1`
```

You need also to set the following environment variables (example in parentheses):

- `ES_HOST` (`$user`:`$pass`@`$host`)
- `ES_INDEX` (`incubator_production_mahoshojos`)
- `ES_PORT` (`80`)
- `HTTP_HOST` (`0.0.0.0`)

Performance
-----------
MacBook Pro (Early 2015) on [11c5714](https://github.com/honeypotio/searchspot/commit/11c57149d88e1dca5cccf858d986894e878cc8f0):

```
┌[giovanni@lifestream-2] [/dev/ttys001] [master ⚡]
└[~/Desktop/searchspot]> wrk -t12 -c400 -d30s "http://127.0.0.1:1234/talents?work_roles[]=DevOps"
Running 30s test @ http://127.0.0.1:1234/talents?work_roles[]=DevOps
  12 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    35.85ms    2.35ms  67.12ms   81.01%
    Req/Sec     0.90k    38.54     0.95k    86.33%
  26790 requests in 30.10s, 4.57MB read
  Socket errors: connect 0, read 588, write 3, timeout 0
Requests/sec:    890.00
Transfer/sec:    155.58KB
```


P.S.: Companies on [Honeypot](http://www.honeypot.io?utm_source=github) use this service to search the developers they need to hire!
