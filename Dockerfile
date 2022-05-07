FROM rustlang/rust:nightly-bullseye@sha256:68b8cd85adfb2711063b033a9d58d1f29863a20f1851dc9bb9e91f7184b3f78a AS build

WORKDIR /opt/bitsquatting
COPY Cargo.* ./
RUN mkdir -p src/ \
  && echo "fn main() {}" > src/main.rs \
  && echo "fn main() {}" > build.rs
RUN cargo fetch
RUN cargo build --release

COPY src/ src/
# gotta touch the files to trick Cargo into compiling again
RUN find src/ -name "*.rs" -exec touch {} \; \
  && cargo build --release

FROM gcr.io/distroless/cc AS binary

WORKDIR /opt/bitsquatting
COPY public/ ./public/
COPY templates/ ./templates/
COPY --from=build /opt/bitsquatting/target/release/bitsquatting ./
CMD [ "/opt/bitsquatting/bitsquatting" ]
