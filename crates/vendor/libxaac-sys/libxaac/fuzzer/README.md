# Fuzzer for libxaac decoder and encoder

This describes steps to build xaac_dec_fuzzer and xaac_enc_fuzzer binary.

## Linux x86/x64

###  Requirements
- cmake (3.5 or above)
- make
- clang (6.0 or above)
  needs to support -fsanitize=fuzzer, -fsanitize=fuzzer-no-link

### Steps to build
Clone libxaac repository
```
$ git clone https://android.googlesource.com/platform/external/libxaac
```
Create a directory inside libxaac and change directory
```
 $ cd libxaac
 $ mkdir build
 $ cd build
```

Build fuzzer with required sanitizers (-DSANITIZE=fuzzer-no-link is mandatory to enable fuzzers)
```
 $ cmake .. -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
   -DCMAKE_BUILD_TYPE=Debug -DSANITIZE=fuzzer-no-link,address,\
   signed-integer-overflow,unsigned-integer-overflow
 $ make
```

### Steps to run
Create a directory CORPUS_DIR and copy some elementary aac files to that folder
To run the fuzzer
```
$ ./xaac_dec_fuzzer CORPUS_DIR
$ ./xaac_enc_fuzzer CORPUS_DIR
```

## Android

### Steps to build
Build the fuzzer
```
  $ SANITIZE_TARGET=address SANITIZE_HOST=address mmma -j$(nproc) \
    external/libxaac/fuzzer
```

### Steps to run
Create a directory CORPUS_DIR and copy some elementary aac files to that folder
Push this directory to device.

To run on device
```
  $ adb sync data
  $ adb shell /data/fuzz/xaac_dec_fuzzer CORPUS_DIR
```
To run on host
```
  $ $ANDROID_HOST_OUT/fuzz/xaac_dec_fuzzer CORPUS_DIR
```

## References:
 * http://llvm.org/docs/LibFuzzer.html
 * https://github.com/google/oss-fuzz
