{
  description = "Yet Another Skkserv 2";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    { self, nixpkgs }:
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
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        in
        {
          default = self.packages.${system}.yaskkserv2;
          yaskkserv2 = pkgs.rustPlatform.buildRustPackage {
            pname = manifest.package.name;
            version = manifest.package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
            cargoBuildFlags = [
              "--bin"
              "yaskkserv2"
              "--bin"
              "yaskkserv2_make_dictionary"
            ];

            # The upstream tests download dictionaries and require network access.
            doCheck = false;

            meta = {
              description = manifest.package.description;
              homepage = "https://github.com/wachikun/yaskkserv2";
              license = with pkgs.lib.licenses; [
                mit
                asl20
              ];
              mainProgram = "yaskkserv2";
              platforms = systems;
            };
          };
        }
      );
    };
}
