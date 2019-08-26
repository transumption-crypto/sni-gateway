{ pkgs ? import ./pkgs.nix {} }:

with pkgs;

let
  gitignore = fetchFromGitHub {
    owner = "hercules-ci";
    repo = "gitignore";
    rev = "6e7569637d699facfdcde91ab5c94bac06d39edc";
    sha256 = "1lz09rmr2yza8bv46ff49226jls6q1rl2x0p11q1940rw4k4bwa9";
  };

  # https://github.com/mozilla/nixpkgs-mozilla/pull/200
  nixpkgs-mozilla = fetchTarball {
    url = "https://github.com/mozilla/nixpkgs-mozilla/archive/24d112e4895f081700ab910889818c5e189f4d69.tar.gz";
    sha256 = "0kvwbnwxbqhc3c3hn121c897m89d9wy02s8xcnrvqk9c96fj83qw";
  };

  inherit (callPackage gitignore {}) gitignoreSource;
  inherit (callPackage "${nixpkgs-mozilla}/package-set.nix" {}) rustChannelOf;

  rustChannel = rustChannelOf {
    channel = "nightly";
    date = "2019-08-21";
    sha256 = "0idc58ikv5lz7f8pvpv0zxrfcpbs1im24h5jh1crh5yfxc5rimg5";
  };

  rustPlatform = makeRustPlatform {
    cargo = rustChannel.rust;
    rustc = rustChannel.rust;
  };
in

rustPlatform.buildRustPackage {
  name = "transgateway";
  src = gitignoreSource ./.;

  cargoSha256 = "0n7nwmgwaw0ram2yhz6zd2lp83jfnkr5k9mif3bj60hhra6imla9";
}
