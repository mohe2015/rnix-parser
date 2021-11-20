/*
cargo run --quiet --example dump-ast ./test_data/general/multiline-strings-minimal.nix
nix eval --expr ...
*/
[
    ''
    echo test
        echo test > $out
                ''
                 ''
 echo test
     echo test > $out
        ''
]