Searchspot
==========
[![Build Status](https://travis-ci.org/honeypotio/searchspot.svg)](https://travis-ci.org/honeypotio/searchspot)
[![](https://meritbadge.herokuapp.com/searchspot)](https://crates.io/crates/searchspot)

This service is responsible for Honeypot's ElasticSearch data and is mainly powered by [rs-es](https://github.com/benashford/rs-es) and [iron](https://github.com/iron/iron).
Companies on [Honeypot](https://www.honeypot.io/pages/how_does_it_work?utm_source=searchspot) use it to search the developers they need to hire.

We hope that it will be useful to anyone who needs a search engine with a more-or-less complex system of data filtering
(including strings, dates and booleans querying and full text search).

Things that are missing
-----------------------
- Proper indentation
- Proper pagination

Dependencies
------------
* Rust
* ElasticSearch 2.x (1.6+ [here](https://github.com/honeypotio/searchspot/tree/es-1.6))

Our target is Rust Stable. However, you can use Rust Nightly too by passing `--features nightly --no-default-features` to cargo.

Setup
-----
Install the latest release of Rust using either [rustup](https://www.rustup.rs), the [official way](https://www.rust-lang.org/downloads.html)
or your package manager (i.e.: `brew install rust`)).

Then clone this repository to your computer and run the executable with

```sh
$ cargo run examples/default.toml
````

You can generate an optimized executable just appending `--release`, but the compile time will be longer.

You can execute `$ cargo test` to run the tests and `$ cargo doc` to generate the documentation.

Please make sure you have an ElasticSearch instance running.

Example
-------
You can create your own searchspot creating a new executable with cargo, whose `main.rs` will look like ours, but instead of
using `searchspot::resources::user::Talent` you'll need to replace it with a new resource made by you, according to your needs.

Basically, a resource is any struct that implements the trait `searchspot::resource::Resource`.

Authentication
--------------
When the authentication is enabled, the server accepts only requests that provide an `Authentication` header containing a valid
[TOTP](https://en.wikipedia.org/wiki/HMAC-based_One-time_Password_Algorithm) token generated using the secrets defined in searchspot's
`auth.read` or `auth.write` depending from the kind of request (either `GET` or `POST`/`DELETE`), i.e.: `{ "Authorize" => "token 492039" }`.

Heroku
------
To deploy this application on Heroku, just run

```sh
$ heroku create my-searchspot --buildpack https://github.com/Hoverbear/heroku-buildpack-rust
$ heroku ps:scale web=1`
```

You need also to set the following environment variables (example in parentheses):

- `ES_URL` (`https://user:pass@some-server.io:80`)
- `ES_INDEX` (`my_index`)
- `HTTP_HOST` (`0.0.0.0`)
- `AUTH_ENABLED` (`true`)
- `AUTH_READ` (`icsbqwdg7ukqluav`)
- `AUTH_WRITE` (`7x2ockhyff4fmm5n`)

You can get the data for `ES_URL` by adding an addon ((☞ﾟ∀ﾟ)☞) for ElasticSearch to `my-searchspot` and click on it.

`AUTH_` is optional – if omitted the feature will be turned off.

Versioning
----------
Unfortunately we didn't use the semantic versioning from the very beginning. We'll bump the minor version
when a relevant change is done or a reindex is needed, otherwise a patch will be released. No major version
is currently planned to be released.

License
-------
Copyright © 2016 [Honeypot GmbH](https://www.honeypot.io/?utm_source=searchspot).
It is free software, and may be redistributed under the terms specified in the [LICENSE](/LICENSE) file.

About Honeypot
--------------
[![Honeypot](https://www.honeypot.io/logo.png)](https://www.honeypot.io/?utm_source=searchspot)

Honeypot is a developer focused job platform.

The names and logos for Honeypot are trademarks of Honeypot GmbH.
