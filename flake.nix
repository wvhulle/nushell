{
  description = "Nushell - A new type of shell (with LSP diagnostics)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";

    # Local reedline with LSP diagnostics feature
    reedline = {
      url = "git+https://github.com/wvhulle/reedline?ref=inline-hints";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.crane.follows = "crane";
    };
  };

  outputs =
    {
      nixpkgs,
      crane,
      reedline,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      craneLib = crane.mkLib pkgs;

      # Get reedline source for combining
      reedlineSrc = reedline;

      # Create combined source with reedline as subdirectory
      combinedSrc = pkgs.runCommand "nushell-combined-src" { } ''
        mkdir -p $out
        cp -r ${./.}/* $out/ || true
        cp -r ${./.}/.[!.]* $out/ 2>/dev/null || true
        chmod -R u+w $out

        # Copy reedline as subdirectory
        cp -r ${reedlineSrc} $out/reedline
        chmod -R u+w $out/reedline

        # Update Cargo.toml to use local reedline path
        substituteInPlace $out/Cargo.toml \
          --replace-fail 'reedline = { path = "../reedline" }' 'reedline = { path = "./reedline" }'
      '';

      # Filter for .nu and .md files (used by include_str!)
      extraFilesFilter =
        path: _type: (builtins.match ".*\\.nu$" path != null) || (builtins.match ".*\\.md$" path != null);

      src = pkgs.lib.cleanSourceWith {
        src = combinedSrc;
        filter = path: type: (extraFilesFilter path type) || (craneLib.filterCargoSources path type);
      };

      commonArgs = {
        inherit src;
        pname = "nushell";
        version = "0.109.2";

        nativeBuildInputs = with pkgs; [
          pkg-config
          python3
        ];

        buildInputs = with pkgs; [
          zstd
          xorg.libX11
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
    {
      packages.${system} = {
        default = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;

passthru.shellPath = "/bin/nu";

            meta = {
              description = "A new type of shell (with LSP diagnostics)";
              homepage = "https://www.nushell.sh/";
              mainProgram = "nu";
            };
          }
        );
      };

      # Dev shell for working on nushell
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          rustc
          cargo
          pkg-config
          python3
          zstd
          xorg.libX11
          openssl
        ];
      };
    };
}
