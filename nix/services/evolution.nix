{ pkgs, ... }:

let
  setupPwm = pkgs.writeShellScript "setup-evolution-pwm" ''
    set -eu

    pwm_chip=/sys/class/pwm/pwmchip0
    pwm_channel="$pwm_chip/pwm0"

    ${pkgs.coreutils}/bin/chgrp gpio "$pwm_chip/export" "$pwm_chip/unexport"
    ${pkgs.coreutils}/bin/chmod 0770 "$pwm_chip/export" "$pwm_chip/unexport"

    if [ ! -d "$pwm_channel" ]; then
      echo 0 > "$pwm_chip/export"
    fi

    attempts=0
    while [ ! -d "$pwm_channel" ] && [ "$attempts" -lt 25 ]; do
      ${pkgs.coreutils}/bin/sleep 0.04
      attempts=$((attempts + 1))
    done

    ${pkgs.coreutils}/bin/chgrp gpio \
      "$pwm_channel" \
      "$pwm_channel/period" \
      "$pwm_channel/duty_cycle" \
      "$pwm_channel/polarity" \
      "$pwm_channel/enable"
    ${pkgs.coreutils}/bin/chmod 0770 \
      "$pwm_channel" \
      "$pwm_channel/period" \
      "$pwm_channel/duty_cycle" \
      "$pwm_channel/polarity" \
      "$pwm_channel/enable"
  '';
in
{
  systemd.tmpfiles.rules = [
    "d /opt/evolution 0755 dragonblade316 users -"
    "d /opt/evolution/bin 0755 dragonblade316 users -"
  ];

  security.sudo.extraRules = [
    {
      users = [ "dragonblade316" ];
      commands = [
        {
          command = "ALL";
          options = [ "NOPASSWD" ];
        }
      ];
    }
  ];

  systemd.services.evolution = {
    description = "Evolution robot application";
    wantedBy = [ "multi-user.target" ];
    wants = [
      "evolution-network.service"
      "network-online.target"
    ];
    after = [
      "evolution-network.service"
      "network-online.target"
    ];

    unitConfig.ConditionPathIsExecutable = "/opt/evolution/bin/evolution";

    serviceConfig = {
      User = "dragonblade316";
      Group = "users";
      SupplementaryGroups = [
        "dialout"
        "video"
        "input"
        "gpio"
        "spi"
      ];
      WorkingDirectory = "/var/lib/evolution";
      StateDirectory = "evolution";
      ExecStartPre = "+${setupPwm}";
      ExecStart = "/opt/evolution/bin/evolution";
      Restart = "on-failure";
      RestartSec = 1;
      Environment = [
        "NIX_LD=/run/current-system/sw/share/nix-ld/lib/ld.so"
        "NIX_LD_LIBRARY_PATH=/run/current-system/sw/share/nix-ld/lib"
        "RUST_BACKTRACE=1"
      ];
    };
  };
}
