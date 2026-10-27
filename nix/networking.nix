{ lib, pkgs, ... }:

let
  hotspotConnection = "evolution-hotspot";
  homeConnection = "evolution-home";

  configureHomeWifi = pkgs.writeShellApplication {
    name = "evolution-wifi";
    runtimeInputs = [
      pkgs.gnugrep
      pkgs.networkmanager
    ];
    text = ''
      if [[ $# -ne 1 ]]; then
        echo "Usage: evolution-wifi <ssid>" >&2
        exit 2
      fi

      ssid="$1"
      read -r -s -p "Wi-Fi password: " password
      echo

      if ! nmcli -t -f NAME connection show | grep -Fxq ${lib.escapeShellArg homeConnection}; then
        nmcli connection add \
          type wifi \
          ifname wlan0 \
          con-name ${lib.escapeShellArg homeConnection} \
          ssid "$ssid"
      fi

      nmcli connection modify ${lib.escapeShellArg homeConnection} \
        connection.autoconnect yes \
        connection.autoconnect-priority 100 \
        802-11-wireless.mode infrastructure \
        802-11-wireless.ssid "$ssid" \
        802-11-wireless-security.key-mgmt wpa-psk \
        802-11-wireless-security.psk "$password" \
        ipv4.method manual \
        ipv4.addresses 192.168.12.254/24 \
        ipv4.gateway 192.168.12.1 \
        ipv4.dns 1.1.1.1 \
        ipv4.ignore-auto-dns yes \
        ipv6.method disabled

      nmcli connection down ${lib.escapeShellArg hotspotConnection} 2>/dev/null || true
      nmcli --wait 20 connection up ${lib.escapeShellArg homeConnection}
    '';
  };

  selectNetwork = pkgs.writeShellApplication {
    name = "evolution-network-select";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.gnugrep
      pkgs.networkmanager
    ];
    text = ''
      if ! nmcli -t -f NAME connection show | grep -Fxq ${lib.escapeShellArg hotspotConnection}; then
        nmcli connection add \
          type wifi \
          ifname wlan0 \
          con-name ${lib.escapeShellArg hotspotConnection} \
          ssid Evolution
        nmcli connection modify ${lib.escapeShellArg hotspotConnection} \
          connection.autoconnect no \
          802-11-wireless.mode ap \
          802-11-wireless.band bg \
          802-11-wireless-security.key-mgmt wpa-psk \
          802-11-wireless-security.psk Technoblade \
          ipv4.method shared \
          ipv4.addresses 192.168.12.1/24 \
          ipv6.method disabled
      fi

      for _ in $(seq 1 10); do
        if nmcli -t -f TYPE,STATE device status | grep -Fxq 'ethernet:connected'; then
          nmcli connection down ${lib.escapeShellArg hotspotConnection} 2>/dev/null || true
          exit 0
        fi

        if ! nmcli -t -f TYPE,STATE device status | grep -Fxq 'ethernet:connecting'; then
          break
        fi

        sleep 1
      done

      if nmcli -t -f NAME connection show | grep -Fxq ${lib.escapeShellArg homeConnection}; then
        if nmcli --wait 20 connection up ${lib.escapeShellArg homeConnection}; then
          exit 0
        fi
      fi

      nmcli --wait 20 connection up ${lib.escapeShellArg hotspotConnection}
    '';
  };
in
{
  networking.networkmanager = {
    enable = true;
    ensureProfiles.profiles.evolution-ethernet = {
      connection = {
        id = "evolution-ethernet";
        type = "ethernet";
        autoconnect = true;
        autoconnect-priority = 50;
      };
      ipv4.method = "auto";
      ipv6.method = "auto";
    };
    dispatcherScripts = [
      {
        type = "basic";
        source = pkgs.writeShellScript "evolution-network-dispatcher" ''
          action="$2"
          connection="''${CONNECTION_ID:-}"

          if [[ "$action" == "up" && "$connection" == "evolution-ethernet" ]]; then
            ${pkgs.networkmanager}/bin/nmcli connection down ${lib.escapeShellArg hotspotConnection} 2>/dev/null || true
          elif [[ "$action" == "down" && "$connection" == "evolution-ethernet" ]]; then
            ${pkgs.systemd}/bin/systemctl restart evolution-network.service
          fi
        '';
      }
    ];
  };

  environment.systemPackages = [ configureHomeWifi ];

  systemd.services.evolution-network = {
    description = "Select the Evolution robot network";
    wantedBy = [ "multi-user.target" ];
    wants = [ "NetworkManager.service" ];
    after = [
      "NetworkManager.service"
      "NetworkManager-ensure-profiles.service"
    ];
    before = [ "evolution.service" ];

    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = lib.getExe selectNetwork;
      Restart = "on-failure";
      RestartSec = 5;
    };
  };
}
