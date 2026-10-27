{ ... }:

{
  services.zenohd = {
    enable = true;
    settings = {
      mode = "router";
      listen.endpoints = [ "tcp/[::]:7447" ];
    };
  };

  networking.firewall.allowedTCPPorts = [ 7447 ];
}
