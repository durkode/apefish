# NixOS module for the self-hosted GitHub Actions runner that runs
# .github/workflows/sprt.yml. Copy it next to configuration.nix and import it:
#
#   imports = [ ./apefish-runner.nix ];
#
# then create the token file (see sprt/README.md, "CI") and `nixos-rebuild switch`.
#
# The runner is a separate system user with no access to /home, so workflow
# code can't reach your own files, keys or the Lichess bot's configuration.
{ pkgs, ... }:

let
  home = "/var/lib/apefish-ci";
in
{
  users.users.apefish-ci = {
    isSystemUser = true;
    group = "apefish-ci";
    inherit home;
    createHome = true;
  };
  users.groups.apefish-ci = { };

  services.github-runners.apefish-sprt = {
    enable = true;
    url = "https://github.com/durkode/apefish";
    # One-time registration token from Settings -> Actions -> Runners -> New
    # self-hosted runner: one line, no trailing newline, owned by root, mode 600.
    # It expires after an hour, but registration only needs it once; the runner
    # then uses its own job-only credentials. Leave the file unchanged: editing
    # it, or any option that forces re-registration (url, extraLabels, workDir),
    # needs a fresh token. Deliberately not a PAT: registering via a PAT needs
    # repository Administration write, which could delete the repo.
    tokenFile = "/etc/apefish-ci/github-runner.token";
    tokenType = "registration";
    extraLabels = [ "apefish-sprt" ];
    replace = true;
    user = "apefish-ci";

    extraPackages = with pkgs; [
      cargo
      rustc
      gcc # linker for cargo
      gh
      fastchess
      curl
      python3 # unzips the opening book
      gnused
      gnugrep
      findutils
    ];

    # The service's HOME is wiped on every restart, so keep the cargo registry
    # and the SPRT caches (fastchess, book, built engines) somewhere persistent.
    extraEnvironment = {
      CARGO_HOME = "${home}/cargo";
      SPRT_CACHE_DIR = "${home}/sprt-cache";
    };

    # The service runs with ProtectSystem=strict; allow writes to the cache.
    serviceOverrides = {
      ReadWritePaths = [ home ];
    };
  };
}
