{ stdenv }:

/*
  cargo run --quiet --example dump-ast ./test_data/general/multiline-strings.nix
  nix-build --expr 'with import <nixpkgs> {}; callPackage ./test_data/general/multiline-strings.nix {}'
*/
{
  a = stdenv.mkDerivation {
    name = "test";
    dontUnpack = true;

    installPhase = ''
      for i in /usr /sw /opt /pkg; do # improve purity
      done
    '';
  };

  b = stdenv.mkDerivation {
    name = "test";
    dontUnpack = true;

    installPhase = ''
      for i in /usr /sw /opt /pkg; do # improve purity
      done
    '';
  };
}
