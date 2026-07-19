{ lib, rustPlatform, fetchFromGitHub, pkg-config, openssl, systemd }:

rustPlatform.buildRustPackage rec {
  pname = "vuio";
  version = "0.0.35";

  src = fetchFromGitHub {
    owner = "vuiodev";
    repo = "vuio";
    rev = "v${version}";
    sha256 = "0000000000000000000000000000000000000000000000000000"; # Replace with actual SHA256 of source release
  };

  cargoSha256 = "0000000000000000000000000000000000000000000000000000"; # Replace with actual cargo vendor SHA256

  nativeBuildInputs = [ pkg-config ];

  buildInputs = [ openssl systemd ];

  meta = with lib; {
    description = "A cross-platform DLNA/UPnP media server with advanced audio features and real-time monitoring";
    homepage = "https://github.com/vuiodev/vuio";
    license = licenses.mit;
    maintainers = [ ];
    platforms = platforms.linux;
  };
}
