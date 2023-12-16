# tempo-rs

Tempo is a temporary notes service.

## Crates

### server

The `server` crate contains the server implementation.

#### Installation

```bash
cargo install --path server
```

#### Usage

Run the server (default port is 7536):

```bash
tempo-server
```

or specify with a specific port:

```bash
tempo-server --port 8080
```

### client

The `client` crate contains a CLI client implementation.

#### Installation

```bash
cargo install --path client
```

#### Usage

```bash
tempo add "some note"
tempo list
```

Optionally, you can specify the socket address:

```bash
tempo -u "localhost:8080" add "some note"
tempo -u "localhost:8080" list
```

or set the `TEMPO_SERVER_URL` environment variable:

```bash
TEMPO_SERVER_URL="localhost:8080" tempo add "some note"
```

### common

This crate contains common code for the client and server, such as the protocol definition.
