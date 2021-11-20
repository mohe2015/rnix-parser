{ stdenv }:

{
    # nix-build --expr 'with import <nixpkgs> {}; callPackage (import ./multiline-strings.nix).a {}'
    a = stdenv.mkDerivation {
        name = "test";
        dontUnpack = true;

        installPhase = ''
        echo test > $out
        '';
    };

    b = stdenv.mkDerivation {
        name = "test";
        dontUnpack = true;

        installPhase = ''
    echo test > $out
        '';
    };
}
