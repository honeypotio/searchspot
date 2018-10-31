FROM rust:1.30

RUN mkdir /app
WORKDIR /app

COPY . /app

RUN cargo build

EXPOSE 3001
