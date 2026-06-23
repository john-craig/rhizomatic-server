{
  description = "Nix flake for the rhizomatic-server Rust project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
        in
        rec {
          default = rustPlatform.buildRustPackage {
            pname = "rhizomatic-server";
            version = "0.1.0";

            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              makeWrapper
              pkg-config
            ];

            buildInputs = with pkgs; [
              sqlite
            ];

            postInstall = ''
              mkdir -p "$out/share/$pname"
              cp -r static "$out/share/$pname/static"
              wrapProgram "$out/bin/rhizomatic-server" \
                --set RHIZOMATIC_STATIC_DIR "$out/share/$pname/static"
            '';

            meta = with pkgs.lib; {
              description = "Rust service for storing and querying rhizomatic themagraphs";
              mainProgram = "rhizomatic-server";
              platforms = platforms.unix;
            };
          };
        });

      apps = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          package = self.packages.${system}.default;
        in
        {
          default = {
            type = "app";
            program = pkgs.lib.getExe package;
          };

          rhizomatic-server = {
            type = "app";
            program = pkgs.lib.getExe package;
          };

          rhizomatic-server-local = {
            type = "app";
            program = pkgs.lib.getExe package;
          };

          rhizomatic-mcp-server = {
            type = "app";
            program = pkgs.lib.getExe' package "rhizomatic-mcp-server";
          };
        });

      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo-watch
              nixfmt-rfc-style
              pkg-config
              rust-analyzer
              rustToolchain
              sqlite
            ];
          };
        });
    };
}
