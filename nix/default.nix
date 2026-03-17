{
  pkgs,
  system,
  crane,
  fenix,
}: let
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: let
    rust-toolchain-toml = builtins.fromTOML (builtins.readFile ../rust-toolchain.toml);
    fenix-toolchain =
      fenix.packages.${system}.stable.withComponents
      rust-toolchain-toml.toolchain.components;
  in
    fenix-toolchain);

  runtimeInputs = with pkgs; [gitMinimal];

  # Include test fixture files (.kdl, .snap) alongside standard Cargo sources
  # so that integration tests run in nix check derivations.
  testFixtureFilter = path: _type:
    builtins.match ".*\\.kdl$" path
    != null
    || builtins.match ".*\\.snap$" path != null;
  src = pkgs.lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      (testFixtureFilter path type) || (craneLib.filterCargoSources path type);
    name = "source";
  };

  # Common derivation arguments used for all builds
  commonArgs = {
    inherit src;

    buildInputs =
      []
      ++ pkgs.lib.optionals pkgs.stdenv.isDarwin
      [
        pkgs.pkgsStatic.libiconv.dev
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
  myCrateCoverage = craneLib.cargoLlvmCov (commonArgs
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
        removeReferencesTo
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
          --suffix PATH : ${pkgs.lib.makeBinPath runtimeInputs}
      '';

      postFixup = pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
        remove-references-to -t ${pkgs.pkgsStatic.libiconv} $out/bin/.git-disjoint-wrapped
        remove-references-to -t ${pkgs.pkgsStatic.libiconv.dev} $out/bin/.git-disjoint-wrapped
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
