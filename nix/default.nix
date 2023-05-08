{
  pkgs,
  system,
  crane,
  fenix,
}: let
  fenix-channel = fenix.packages.${system}.latest;
  fenix-toolchain = fenix-channel.withComponents [
    "rustc"
    "cargo"
    "clippy"
    "rust-analysis"
    "rust-src"
    "rustfmt"
  ];

  craneLib = crane.lib.${system}.overrideToolchain fenix-toolchain;

  runtimeInputs = with pkgs; [gitMinimal];

  # Common derivation arguments used for all builds
  commonArgs = {
    src = craneLib.cleanCargoSource ../.;

    # Add extra inputs here or any other derivation settings
    # doCheck = true;
    buildInputs = with pkgs;
      [
        openssl
        fenix-channel.rustc
        fenix-channel.clippy
      ]
      ++ pkgs.lib.optionals pkgs.stdenv.isDarwin
      [
        darwin.apple_sdk.frameworks.Security
        libiconv
      ];

    nativeBuildInputs = with pkgs; [
      cmake
    ];
  };

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # Run clippy (and deny all warnings) on the crate source,
  # resuing the dependency artifacts (e.g. from build scripts or
  # proc-macros) from above.
  #
  # Note that this is done as a separate derivation so it
  # does not impact building just the crate by itself.
  myCrateClippy = craneLib.cargoClippy (commonArgs
    // {
      # Again we apply some extra arguments only to this derivation
      # and not every where else. In this case we add some clippy flags
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "-- --deny warnings";
    });

  # Next, we want to run the tests and collect code-coverage, _but only if
  # the clippy checks pass_ so we do not waste any extra cycles.
  myCrateCoverage = craneLib.cargoNextest (commonArgs
    // {
      cargoArtifacts = myCrateClippy;
    });

  # Build the actual crate itself, reusing the dependency
  # artifacts from above.
  myCrate = craneLib.buildPackage (commonArgs
    // {
      inherit cargoArtifacts runtimeInputs;

      nativeBuildInputs = with pkgs; [
        findutils
        installShellFiles
        makeWrapper
      ];

      postInstall = ''
        installManPage "$(find target/release/build -type f -name git-disjoint.1)"
        installShellCompletion \
          "$(find target/release/build -type f -name git-disjoint.bash)" \
          "$(find target/release/build -type f -name git-disjoint.fish)" \
          --zsh "$(find target/release/build -type f -name _git-disjoint)"

        wrapProgram $out/bin/git-disjoint \
          --prefix PATH ${pkgs.lib.makeBinPath runtimeInputs}
      '';
    });
in {
  inherit
    commonArgs
    fenix-toolchain
    myCrate
    myCrateClippy
    myCrateCoverage
    runtimeInputs
    ;
}
