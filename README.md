Searchspot
==========
This service is used as endpoint responsible for Honeypot's ElasticSearch data.
For this first iteration it only reads and returns the data based on given filters but the plan is to implement the population of the indexes as well.

Our hope is to make this service usable by everyone who needs a search engine with a more-or-less complex system of data filtering (including string matching, dates, booleans and full text search).

Every kind of contribution is more than well accepted!

Setup
-----
Install the latest stable release of Rust using the [official installer](https://www.rust-lang.org/downloads.html) or your package manager (i.e.: `brew install rust`).

Then clone this repository to your computer and run the executable with

```sh
$ cargo run [examples/default.toml]
````

You can produce an optimized executable just appending `--release`, but the compile time will be longer.

You can execute `$ cargo test` to run the tests and `$ cargo doc` to produce the documentation.

Heroku
------
To deploy this application on Heroku, just run `$ heroku create my-searchspot --buildpack https://github.com/Hoverbear/heroku-buildpack-rust` and then `$ heroku ps:scale web=1`.

You need also to set the following environment variables:

- `ES_HOST` (i.e.: `$user`:`$pass`@`$host`)
- `ES_INDEX` (i.e.: `incubator_production_mahoshojos`)
- `ES_PORT` (i.e.: `80`)
– `HTTP_HOST` (i.e.: `0.0.0.0`)

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

P.S.: Companies on [Honeypot](http://www.honeypot.io?utm_source=github) use this service to search the developers they need to hire!
