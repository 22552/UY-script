# Install

UY-script is currently built from source and keeps the upstream `goboscript` package/binary name for compatibility.

!!! tip
    UY-script currently requires the **nightly** Rust toolchain. Install it once with:

    ```bash
    rustup toolchain install nightly
    ```

## Install from source

Requires `git` and the [Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/22552/UY-script.git
cd UY-script
cargo +nightly install --path .
```

The installed command is currently:

```bash
goboscript --help
```

To update:

```bash
cd UY-script
git pull
cargo +nightly install --path . --force
```

## Install directly with Cargo

```bash
cargo +nightly install --git https://github.com/22552/UY-script.git
```

To update:

```bash
cargo +nightly install --git https://github.com/22552/UY-script.git --force
```

## Development checkout

For compiler development, cloning the repository and running directly is usually easier:

```bash
git clone https://github.com/22552/UY-script.git
cd UY-script
cargo +nightly test
cargo +nightly run -- --help
```

UY-script adds its static type/ownership/borrow passes before the inherited goboscript Scratch backend, so running the normal test suite is useful when changing either layer.

## Install with Nix

The repository retains the upstream Nix setup. You can enter its development shell directly from this fork:

```bash
nix develop github:22552/UY-script
```

For a NixOS flake input:

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=25.11";
    uy-script.url = "github:22552/UY-script";
  };

  outputs = { self, nixpkgs, uy-script, ... }: {
    nixosConfigurations.yourHostname = nixpkgs.lib.nixosSystem {
      modules = [
        ({ pkgs, ... }: {
          environment.systemPackages = [
            uy-script.packages.${pkgs.stdenv.hostPlatform.system}.goboscript
          ];
        })
      ];
    };
  };
}
```

The package output is still named `goboscript` for compatibility.