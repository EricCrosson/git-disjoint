{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane = {
      url = "github:ipetkov/crane";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    fenix,
    git-hooks,
  }: let
    forEachSystem = nixpkgs.lib.genAttrs [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];

    # Workaround for nixpkgs#351574 — cargo-llvm-cov tests fail on macOS
    cargo-llvm-cov-overlay = _final: prev: {
      cargo-llvm-cov = prev.cargo-llvm-cov.overrideAttrs (_old: {
        doCheck = false;
      });
    };
  in {
    checks = forEachSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [cargo-llvm-cov-overlay];
      };
      craneDerivations = pkgs.callPackage ./default.nix {inherit crane fenix;};
      pre-commit-check = git-hooks.lib.${system}.run {
        src = ../.;
        hooks = {
          actionlint.enable = true;
          alejandra = {
            enable = true;
            settings.verbosity = "quiet";
          };
          clippy = {
            enable = true;
            package = craneDerivations.fenix-toolchain;
            settings.denyWarnings = true;
          };
          deadnix.enable = true;
          prettier.enable = true;
          rustfmt = {
            enable = true;
            package = craneDerivations.fenix-toolchain;
          };
          statix.enable = true;
          taplo.enable = true;
        };
      };
    in {
      inherit
        (craneDerivations)
        myCrate
        myCrateClippy
        myCrateCoverage
        ;
      inherit pre-commit-check;
    });

    packages = forEachSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [cargo-llvm-cov-overlay];
      };
      craneDerivations = pkgs.callPackage ./default.nix {inherit crane fenix;};
    in {
      lcov = craneDerivations.myCrateCoverage;
    });

    apps = forEachSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [cargo-llvm-cov-overlay];
      };
      rust-toolchain-toml = builtins.fromTOML (builtins.readFile ../rust-toolchain.toml);
      fenix-toolchain =
        fenix.packages.${system}.stable.withComponents
        rust-toolchain-toml.toolchain.components;
    in {
      coverage = {
        type = "app";
        program = toString (pkgs.writeShellScript "coverage" ''
          export PATH="${pkgs.lib.makeBinPath [fenix-toolchain pkgs.cargo-llvm-cov]}:$PATH"
          cargo llvm-cov --open "$@"
        '');
      };
    });

    devShells = forEachSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [cargo-llvm-cov-overlay];
      };
      craneDerivations = pkgs.callPackage ./default.nix {inherit crane fenix;};
    in {
      default = pkgs.mkShell {
        # DISCUSS: can we use inherit instead?
        # DISCUSS: can we use inputsFrom instead?
        buildInputs = craneDerivations.commonArgs.buildInputs ++ craneDerivations.runtimeInputs;
        nativeBuildInputs =
          craneDerivations.commonArgs.nativeBuildInputs
          ++ (let
            rust-toolchain-toml = builtins.fromTOML (builtins.readFile ../rust-toolchain.toml);
          in [
            (fenix.packages.${system}.stable.withComponents
              rust-toolchain-toml.toolchain.components)
            pkgs.cargo-insta
            pkgs.cargo-llvm-cov
            pkgs.vhs
          ]);

        inherit (self.checks.${system}.pre-commit-check) shellHook;
      };
    });
  };
}
