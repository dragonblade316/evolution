{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    clang
    cmake
    ninja
    pkg-config

    gtk3

    flutter
  ];

  LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [
    fontconfig.lib
    sqlite.out
  ];

  GSETTINGS_SCHEMA_DIR="${pkgs.gtk3}/share/gsettings-schemas/gtk+3-3.24.41/glib-2.0/schemas/";
}
