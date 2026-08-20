{ lib, rustPlatform, pkg-config, openssl, rust }:

rustPlatform.buildRustPackage {
  # Keep the package/output name for compatibility with the upstream CLI.
  pname = "goboscript";
  version = "3.3.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [ pkg-config rust ];
  buildInputs = [ openssl ];

  meta = {
    description = "UY-script: a goboscript fork with static types, ownership, and borrow checking";
    homepage = "https://github.com/22552/UY-script";
    license = lib.licenses.mit;
  };
}
