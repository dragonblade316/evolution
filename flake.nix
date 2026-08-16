{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs?ref=release-25.11";
    robotics-scripts = {
      url = "github:dragonblade316/robotics-scripts";
      # inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    robotics-scripts,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {
        inherit system overlays;
        config.allowUnfree = true;
      };
      onshape-to-robot = robotics-scripts.packages.${system}.onshape-to-robot;
      moteus_gui = robotics-scripts.packages.${system}.moteus-gui;
      moteus = robotics-scripts.packages.${system}.moteus;
      rust = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
    in
      with pkgs; rec {
        devShell = mkShell rec {
        buildInputs = [
            # rust-bin.stable.latest.default
            cargo
            rust-analyzer

            perf
            protobuf
            zenoh
            clang
            rustPlatform.bindgenHook
            rerun
            opencv4
            apriltag

            just

            wireviz

            pkg-config
            mujoco

            #needed for a temp package
            libx11
            wayland

            libxkbcommon
            libGL
            # needed by dioxus-desktop (xdotool) for the evods driver station UI
            xdotool
            zsh

            # WINIT_UNIX_BACKEND=wayland
            wayland

            # WINIT_UNIX_BACKEND=x11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            xorg.libX11
            onshape-to-robot
            moteus_gui
            moteus

            #for bevy
            alsa-lib
            #why is libudev in systemd?
            systemd

            #things for flutter
            flutter
            gtk3
            cmake
            ninja

            libsoup_3
            webkitgtk_4_1

            #things for shotcal
            (pkgs.python3.withPackages (ps: with ps; [
              numpy
              scipy
              plotly
            ]))
          ];

          shellHook = ''
            zsh
          '';



          env.MUJOCO_PATH = "${mujoco}";
          env.MUJOCO_PLUGIN_PATH = "${mujoco}/lib";
          env.MUJOCO_DYNAMIC_LINK_DIR = "${mujoco}/lib";

          libpath = lib.concat buildInputs [fontconfig.lib sqlite.out];

          LD_LIBRARY_PATH = "${lib.makeLibraryPath libpath}";
          GSETTINGS_SCHEMA_DIR="${pkgs.gtk3}/share/gsettings-schemas/gtk+3-3.24.41/glib-2.0/schemas/";

        };

        packages.default = pkgs.rustPlatform.buildRustPackage rec {
          pname = "evosim"; # Replace with your package name
          version = "0.1.0";           # Replace with your version

          # The source of your project (the current directory)
          src = ./evosim;
          # Specify the location of your Cargo.lock file
          cargoLock.lockFile = ./Cargo.lock;
        };

      });
}
