{ modulesPath, pkgs, robotics-scripts, ... }:

let
  githubKeys = pkgs.fetchurl {
    url = "https://github.com/dragonblade316.keys";
    hash = "sha256-u+cXfHsMvXETvNDU5m4hxGMLKijX8DHliY5TaUFwKec=";
  };
in
{
  imports = [
    "${modulesPath}/installer/sd-card/sd-image-aarch64.nix"
    ./networking.nix
    ./services/evolution.nix
    ./services/zenoh.nix
  ];

  image.baseName = "evolution-rpi3";

  boot.kernelModules = [
    "spi_bcm2835"
    "spidev"
  ];

  hardware.deviceTree = {
    filter = "bcm2837-rpi-3-b.dtb";
    overlays = [
      {
        name = "spi0-2cs";
        filter = "bcm2837-rpi-3-b.dtb";
        dtsText = ''
          /dts-v1/;
          /plugin/;

          / {
            compatible = "raspberrypi,3-model-b", "brcm,bcm2837";

            fragment@0 {
              target = <&spi>;

              __overlay__ {
                status = "okay";
                pinctrl-names = "default";
                pinctrl-0 = <&spi0_gpio7>;
                #address-cells = <1>;
                #size-cells = <0>;

                spidev@0 {
                  compatible = "rohm,dh2228fv";
                  reg = <0>;
                  spi-max-frequency = <125000000>;
                };

                spidev@1 {
                  compatible = "rohm,dh2228fv";
                  reg = <1>;
                  spi-max-frequency = <125000000>;
                };
              };
            };
          };
        '';
      }
      {
        name = "pwm0-gpio12";
        filter = "bcm2837-rpi-3-b.dtb";
        dtsText = ''
          /dts-v1/;
          /plugin/;

          / {
            compatible = "raspberrypi,3-model-b", "brcm,bcm2837";

            fragment@0 {
              target = <&pwm>;

              __overlay__ {
                status = "okay";
                pinctrl-names = "default";
                pinctrl-0 = <&pwm0_gpio12>;
              };
            };
          };
        '';
      }
    ];
  };

  networking = {
    hostName = "evolution";
    firewall.allowedTCPPorts = [ 8080 ];
  };

  services.avahi = {
    enable = true;
    nssmdns4 = true;
    publish = {
      enable = true;
      addresses = true;
      workstation = true;
    };
  };

  services.openssh = {
    enable = true;
    openFirewall = true;
    settings = {
      PasswordAuthentication = true;
      PermitRootLogin = "no";
    };
  };

  users.groups = {
    gpio = { };
    spi = { };
  };

  users.users.dragonblade316 = {
    isNormalUser = true;
    openssh.authorizedKeys.keyFiles = [ githubKeys ];
    extraGroups = [
      "wheel"
      "networkmanager"
      "dialout"
      "video"
      "input"
      "gpio"
      "spi"
    ];
  };

  services.udev.extraRules = ''
    KERNEL=="gpiochip*", SUBSYSTEM=="gpio", GROUP="gpio", MODE="0660"
    KERNEL=="gpiomem", GROUP="gpio", MODE="0660"
    KERNEL=="spidev*", SUBSYSTEM=="spidev", GROUP="spi", MODE="0660"
  '';

  environment.systemPackages = with pkgs; [
    git
    rsync
    vim
    robotics-scripts.packages.${pkgs.stdenv.hostPlatform.system}.moteus
  ];

  nix.settings.experimental-features = [
    "nix-command"
    "flakes"
  ];

  programs.nix-ld.enable = true;

  system.stateVersion = "25.11";
}
