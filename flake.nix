{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crate2nix, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        crateName = "rub-login";

        inherit (import "${crate2nix}/tools.nix" { inherit pkgs; })
          generatedCargoNix;

        project = import (generatedCargoNix {
          name = crateName;
          src = ./.;
        }) {
          inherit pkgs;
          defaultCrateOverrides = pkgs.defaultCrateOverrides // {
            # Crate dependency overrides go here
          };
        };

      in {
        packages.${crateName} = project.rootCrate.build;

        defaultPackage = self.packages.${system}.${crateName};

        devShell = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = [ pkgs.cargo pkgs.rust-analyzer pkgs.clippy ];
        };
      }) // {
    nixosModules.default = { lib, pkgs, config, ... }: with lib; 
    let
      cfg = config.services.rub-login;
      rub-loginBin = self.packages.${pkgs.stdenv.hostPlatform.system}.rub-login;
    in {
      options.services.rub-login = {
        enable = mkEnableOption "rub-login";
        username = mkOption {
          type = types.str;
        };
        passwordFile = mkOption {
          type = types.str;
        };
      };

      config = mkIf cfg.enable {
        systemd.services.rub-login = {
          serviceConfig = {
            Type = "oneshot";
            ExecStart = "${rub-loginBin}/bin/rub-login login ${escapeShellArg cfg.username} ${escapeShellArg cfg.passwordFile}";
            TimeoutStartSec = "10s";
          };
        };

        systemd.timers.rub-login = {
          wantedBy = [ "timers.target" ];
          after = [ "network-online.target" ];
          timerConfig = {
            OnActiveSec = [ "0" ];
            OnCalendar = [ "*:0/15" ];
            Persistent = true;
          };
        };
      };
    };
  };
}
