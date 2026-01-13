{
  rev,
  lib,
  stdenv,
  pkgs,
}:
pkgs.pkgsi686Linux.stdenv.mkDerivation {
  pname = "SLSsteam";
  version = "${rev}";
  src = ../.;

  nativeBuildInputs = with pkgs; [
    pkg-config
    makeWrapper
  ];

  buildInputs = with pkgs.pkgsi686Linux; [
    openssl
    which
  ];

  buildPhase = ''
    make clean
    make
  '';

  installPhase = ''
    mkdir -p $out/
    cp bin/SLSsteam.so $out/
  '';

  meta = {
    description = "Steamclient Modification for Linux";
    homepage = "https://github.com/AceSLS/SLSsteam";
    license = lib.licenses.agpl3Only;
    platforms = lib.platforms.linux;
  };
}
