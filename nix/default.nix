{
  pkgs,
  system,
  crane,
  fenix,
}: let
  craneLib = (crane.mkLib pkgs).overrideToolchain (p: let
    fenix-channel = fenix.packages.${system}.latest;
    fenix-toolchain = fenix-channel.withComponents [
      "rustc"
      "cargo"
      "clippy"
      "rust-analysis"
      "rust-src"
      "rustfmt"
    ];
  in
    fenix-toolchain);

  runtimeInputs = with pkgs; [gitMinimal];

  # Common derivation arguments used for all builds
  commonArgs = {
    src = craneLib.cleanCargoSource ../.;

    buildInputs = with pkgs;
      [
        openssl
      ]
      ++ pkgs.lib.optionals pkgs.stdenv.isDarwin
      [
        darwin.apple_sdk.frameworks.Security
        darwin.apple_sdk.frameworks.SystemConfiguration
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
        installManPage "$(
          find target/release/build -type f -name git-disjoint.1 -print0 \
          | xargs -0 ls -t \
          | head -n 1
        )"
        installShellCompletion \
          "$(
            find target/release/build -type f -name git-disjoint.bash -print0 \
            | xargs -0 ls -t \
            | head -n 1
          )" \
          "$(
            find target/release/build -type f -name git-disjoint.fish -print0 \
            | xargs -0 ls -t \
            | head -n 1
          )" \
          --zsh "$(
            find target/release/build -type f -name _git-disjoint -print0 \
            | xargs -0 ls -t \
            | head -n 1
          )"

        wrapProgram $out/bin/git-disjoint \
          --prefix PATH ${pkgs.lib.makeBinPath runtimeInputs}
      '';
    });
in {
  inherit
    commonArgs
    myCrate
    myCrateClippy
    myCrateCoverage
    runtimeInputs
    ;
}
