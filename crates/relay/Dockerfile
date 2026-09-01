FROM rust:1.82-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release --package ssh-migrator-relay

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/relay /usr/local/bin/relay
EXPOSE 8443
ENV PORT=8443
CMD ["relay"]
