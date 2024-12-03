{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    crane = {
      url = "github:ipetkov/crane";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    fenix,
    pre-commit-hooks,
  }: let
    forEachSystem = nixpkgs.lib.genAttrs [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];
  in {
    checks = forEachSystem (system: let
      craneDerivations = nixpkgs.legacyPackages.${system}.callPackage ./default.nix {
        inherit crane fenix;
      };
      pre-commit-check = pre-commit-hooks.lib.${system}.run {
        src = ../.;
        hooks = {
          actionlint.enable = true;
          alejandra.enable = true;
          prettier.enable = true;
          rustfmt.enable = true;
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

    devShells = forEachSystem (system: let
      craneDerivations = nixpkgs.legacyPackages.${system}.callPackage ./default.nix {
        inherit crane fenix;
      };
    in {
      default = nixpkgs.legacyPackages.${system}.mkShell {
        # DISCUSS: can we use inherit instead?
        # DISCUSS: can we use inputsFrom instead?
        buildInputs = craneDerivations.commonArgs.buildInputs ++ craneDerivations.runtimeInputs;
        nativeBuildInputs =
          craneDerivations.commonArgs.nativeBuildInputs
          ++ [
            fenix.packages.${system}.rust-analyzer
          ];

        inherit (self.checks.${system}.pre-commit-check) shellHook;
      };
    });
  };
}
