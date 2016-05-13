Searchspot
==========
This service is used as endpoint responsible for Honeypot's ElasticSearch data and it's powered by [benashford](https://github.com/benashford)'s rs-es.

We hope that it will be useful to anyone who needs a search engine with a more-or-less complex system of data filtering (including string matching, dates, booleans and full text search).

Things that are missing
-----------------------
- Bulk indexing
- Proper pagination

Dependencies
------------
* ElasticSearch 1.6 - 2.2.2

Setup
-----
Install the latest stable release of Rust using the [official installer](https://www.rust-lang.org/downloads.html) or your package manager (i.e.: `brew install rust`).

Then clone this repository to your computer and run the executable with

```sh
$ cargo run examples/default.toml
````

You can generate an optimized executable just appending `--release`, but the compile time will be longer.

You can execute `$ cargo test` to run the tests and `$ cargo doc` to generate the documentation.

Please make sure you have an ElasticSearch instance running.

Example
-------
You can create your own searchspot creating a new executable with cargo, whose `main.rs` will look like our [src/main.rs](https://github.com/honeypotio/searchspot/blob/master/src/main.rs), but instead of using `searchspot::resources::user::Talent`, you'll need to replace it with a new resource made by you, according to your needs.

Basically, a resource is any struct that implements the trait `searchspot::resource::Resource`.

Authentication
--------------
When the authentication is enabled, the server accepts only requests that provide an `Authentication` header containing a valid [TOTP](https://en.wikipedia.org/wiki/HMAC-based_One-time_Password_Algorithm) token generated using the secrets defined in searchspot's `auth.read` or `auth.write` depending from the kind of request (either `GET` or `POST`/`DELETE`). I.e.: `{ "Authorize" => "token 492039" }`.

Heroku
------
To deploy this application on Heroku, just run

```sh
$ heroku create my-searchspot --buildpack https://github.com/Hoverbear/heroku-buildpack-rust
$ heroku ps:scale web=1`
```

You need also to set the following environment variables (example in parentheses):

- `ES_HOST` (`$user`:`$pass`@`$host`)
- `ES_INDEX` (`my_index`)
- `ES_PORT` (`80`)
- `HTTP_HOST` (`0.0.0.0`)
- `AUTH_ENABLED` (`true`)
- `AUTH_READ` (`icsbqwdg7ukqluav`)
- `AUTH_WRITE` (`7x2ockhyff4fmm5n`)

You can get the data for `ES_HOST` by adding an addon ((☞ﾟ∀ﾟ)☞) for ElasticSearch to `my-searchspot` and click on it.

`AUTH_` is optional – if omitted the feature will be turned off.

P.S.: Companies on [Honeypot](https://www.honeypot.io/pages/how_does_it_work?utm_source=gh) use this service to search the developers they need to hire!
