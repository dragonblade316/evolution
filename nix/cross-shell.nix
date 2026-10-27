{ pkgs }:

let
  target = "aarch64-unknown-linux-gnu";
  crossPkgs = pkgs.pkgsCross.aarch64-multiplatform;
  rust = pkgs.rust-bin.stable.latest.default.override {
    targets = [ target ];
  };
in
pkgs.mkShell {
  nativeBuildInputs = [
    rust
    pkgs.cmake
    pkgs.clang
    pkgs.openssh
    pkgs.patchelf
    pkgs.pkg-config
    pkgs.rsync
    crossPkgs.rustPlatform.bindgenHook
    crossPkgs.stdenv.cc
  ];

  buildInputs = [
    crossPkgs.udev
  ];

  CARGO_BUILD_TARGET = target;
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER =
    "${crossPkgs.stdenv.cc}/bin/${crossPkgs.stdenv.cc.targetPrefix}cc";
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS =
    "-C link-arg=-Wl,--dynamic-linker=/lib/ld-linux-aarch64.so.1";
  APRILTAG_SYS_METHOD = "raw,static";
  PKG_CONFIG_ALLOW_CROSS = "1";
}
