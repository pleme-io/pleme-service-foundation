# pleme-service-foundation

Service foundation library for Pleme platform - bootstrap, health checks, graceful shutdown

## Installation

```toml
[dependencies]
pleme-service-foundation = "0.1"
```

## Usage

```rust
use pleme_service_foundation::{ServiceBuilder, ServiceConfig};

ServiceBuilder::new(ServiceConfig::from_env()?)
    .with_health_check()
    .with_graceful_shutdown()
    .serve(app)
    .await?;
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `errors` | pleme-error integration |
| `full` | All features enabled |

Enable features in your `Cargo.toml`:

```toml
pleme-service-foundation = { version = "0.1", features = ["full"] }
```

## Development

This project uses [Nix](https://nixos.org/) for reproducible builds:

```bash
nix develop            # Dev shell with Rust toolchain
nix run .#check-all    # cargo fmt + clippy + test
nix run .#publish      # Publish to crates.io (--dry-run supported)
nix run .#regenerate   # Regenerate Cargo.nix
```

## License

MIT - see [LICENSE](LICENSE) for details.
