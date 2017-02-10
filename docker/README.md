# Searchspot on Docker

## From Dockerfile
First of all, [install Docker](https://docs.docker.com/engine/installation/).

Then, `$ mv env.list.example env.list` and modify `env.list` (if you already configured the
TOML configuration files, you can just copy the data from them).

Build the image by running `$ docker build -t "searchspot:latest"` and start it with
`$ docker run --env-file env.list -p 3001:3001 searchspot:latest`.

Searchspot should now be listening to `http://localhost`.
