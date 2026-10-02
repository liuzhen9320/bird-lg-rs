{
  description = "BIRD Looking Glass frontend and proxy in Rust";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./frontend/Cargo.toml
              ./frontend/src
              ./frontend/assets
              ./proxy/Cargo.toml
              ./proxy/src
            ];
          };
          cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
          mkPackage =
            pname: description:
            pkgs.rustPlatform.buildRustPackage {
              inherit
                pname
                version
                src
                cargoDeps
                ;

              # aws-lc-sys, used by reqwest, builds native code with CMake and Perl.
              nativeBuildInputs = with pkgs; [
                cmake
                perl
                makeWrapper
              ];
              dontUseCmakeConfigure = true;
              cargoBuildFlags = [ "--package=${pname}" ];
              cargoTestFlags = [ "--package=${pname}" ];

              # Use a shell available in the Nix sandbox for the proxy's process tests.
              postPatch = lib.optionalString (pname == "bird-lgproxy-rs") ''
                substituteInPlace proxy/src/traceroute.rs \
                  --replace-fail '"/bin/sh"' '"${pkgs.runtimeShell}"'
              '';

              postInstall = lib.optionalString (pname == "bird-lgproxy-rs" && pkgs.stdenv.hostPlatform.isLinux) ''
                wrapProgram "$out/bin/bird-lgproxy-rs" \
                  --suffix PATH : ${lib.makeBinPath [ pkgs.traceroute ]}
              '';

              meta = {
                inherit description;
                license = lib.licenses.gpl3Only;
                mainProgram = pname;
                platforms = systems;
              };
            };
        in
        {
          bird-lg-rs = mkPackage "bird-lg-rs" "BIRD Looking Glass frontend";
          bird-lgproxy-rs = mkPackage "bird-lgproxy-rs" "BIRD Looking Glass proxy";
          default = self.packages.${system}.bird-lg-rs;
        }
      );

      apps = forAllSystems (
        system:
        let
          mkApp = package: {
            type = "app";
            program = nixpkgs.lib.getExe package;
            meta.description = package.meta.description;
          };
        in
        {
          bird-lg-rs = mkApp self.packages.${system}.bird-lg-rs;
          bird-lgproxy-rs = mkApp self.packages.${system}.bird-lgproxy-rs;
          default = self.apps.${system}.bird-lg-rs;
        }
      );

      checks = forAllSystems (system: {
        inherit (self.packages.${system}) bird-lg-rs bird-lgproxy-rs;
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = builtins.attrValues self.checks.${system};
            packages =
              with pkgs;
              [
                cargo
                rustc
                rustfmt
                clippy
                rust-analyzer
              ]
              ++ lib.optionals stdenv.hostPlatform.isLinux [ traceroute ];
            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
