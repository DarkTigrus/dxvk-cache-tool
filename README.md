# dxvk-cache-tool

Standalone dxvk-cache merger

Usage
-----
```
dxvk-cache-tool [OPTION]... <FILEs>...

OPTIONS:    
        -o, --output FILE   Set output file name
        -h, --help          Display help and exit
        -V, --version       Output version information and exit
```

```
$ dxvk-cache-tool re2_proton.dxvk-cache re2_lutris.dxvk-cache
Merging files ["re2_proton.dxvk-cache", "re2_lutris.dxvk-cache"]
Detected state cache version v8
Merging re2_proton.dxvk-cache (1/2)... 744 new entries
Merging re2_lutris.dxvk-cache (2/2)... 224 new entries
Writing 968 entries to file output.dxvk-cache 
Finished
```

Building
-----
```
cargo build --release
target/release/dxvk-cache-tool
```