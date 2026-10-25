# Build
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p ccda2fhir-server

# Run: small image, non-root, no shell
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/ccda2fhir /usr/local/bin/ccda2fhir
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD ["/usr/local/bin/ccda2fhir", "healthcheck", "--addr", "127.0.0.1:8080"]
ENTRYPOINT ["/usr/local/bin/ccda2fhir"]
CMD ["serve"]
