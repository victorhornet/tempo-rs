# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.74.1
FROM rust:${RUST_VERSION}-slim-bullseye AS build
WORKDIR /app
# Build the application.
# Leverage a cache mount to /usr/local/cargo/registry/
# for downloaded dependencies and a cache mount to /app/target/ for 
# compiled dependencies which will speed up subsequent builds.
# Leverage a bind mount to the src directory to avoid having to copy the
# source code into the container. Once built, copy the executable to an
# output directory before the cache mounted /app/target is unmounted.
RUN --mount=type=bind,source=client,target=client \
    --mount=type=bind,source=common,target=common \
    --mount=type=bind,source=server,target=server \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    <<EOF
set -e
cargo build --locked --release
cp ./target/release/tempo-server /bin/server
cp ./target/release/tempo /bin/client
EOF


FROM debian:bullseye-slim AS with-user
# Create a non-privileged user that the app will run under.
# See https://docs.docker.com/go/dockerfile-user-best-practices/
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser


FROM with-user AS server
COPY --from=build /bin/server /bin/
EXPOSE 8080
CMD ["/bin/server", "-p", "8080"]

FROM with-user AS client
COPY --from=build /bin/client /bin/tempo
ENTRYPOINT [ "/bin/tempo" ]
