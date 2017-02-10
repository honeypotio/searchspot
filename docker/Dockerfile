FROM liuchong/rustup:nightly

# RUN apt-get update && \
#     apt-get install --no-install-recommends -y \
#     git \
#     nano && \
#     rm -rf /var/lib/apt/lists/*

RUN cargo install searchspot --force
# RUN git clone https://github.com/honeypotio/searchspot
# WORKDIR searchspot

EXPOSE 3001

# RUN cargo build --release
# CMD ["./target/release/searchspot"]
CMD ["searchspot"]
