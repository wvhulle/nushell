{
  nixConfig = {
    extra-substituters = [ "https://wvhulle.cachix.org" ];
    extra-trusted-public-keys = [ "wvhulle.cachix.org-1:heXx8DZMiRsKUx6l1TxNoF+Nmtmz66QEdsonQzc1ir0=" ];
  };

  description = "Nushell that shows LSP diagnostics inline as you type";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };

      # Toolchain pinned by ./rust-toolchain.toml (single source of truth, matches
      # upstream CI). rust-src and rust-analyzer are added for IDE support.
      rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
        extensions = [
          "rust-src"
          "rust-analyzer"
        ];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      src = pkgs.lib.cleanSource ./.;

      # Track the upstream nushell version instead of hardcoding it. The root
      # package inherits its version from the workspace.
      cargoToml = pkgs.lib.importTOML ./Cargo.toml;
      nushellVersion =
        if (cargoToml.package.version.workspace or false) then
          cargoToml.workspace.package.version
        else
          cargoToml.package.version;

      commonArgs = {
        inherit src;
        pname = "nu-inline-diagnostics";
        version = nushellVersion;

        nativeBuildInputs = with pkgs; [
          pkg-config
          python3
        ];

        buildInputs = with pkgs; [
          zstd
          libx11
          openssl
        ];

        # Required for openssl-sys
        OPENSSL_NO_VENDOR = 1;

        # Build with mcp feature
        cargoExtraArgs = "--features mcp";
      };

      # Build dependencies separately - this gets cached
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;
    in
    rec {
      apps.${system}.default = {
        type = "app";
        program = pkgs.lib.getExe packages.${system}.default;
      };
      packages.${system} = {
        default = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;

            passthru.shellPath = "/bin/nu";

            meta = {
              description = "Nushell that shows LSP diagnostics inline as you type";
              homepage = "https://github.com/wvhulle/nushell";
              mainProgram = "nu";
            };
          }
        );

        nu_plugin_query = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "nu_plugin_query";
            cargoExtraArgs = "--package nu_plugin_query";
            doCheck = false;

            meta = {
              description = "Nushell query plugin";
              mainProgram = "nu_plugin_query";
            };
          }
        );
      };

      # Dev shell for working on nushell
      devShells.${system}.default = craneLib.devShell {
        packages = with pkgs; [
          pkg-config
          python3
          zstd
          libx11
          openssl
        ];
      };
    };
}
