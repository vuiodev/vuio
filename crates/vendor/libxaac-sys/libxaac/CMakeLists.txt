cmake_minimum_required(VERSION 3.5.1)
project(libxaac C CXX)
enable_language(ASM)

option(BUILD64 "Build for 64 bit" OFF)
set(XAAC_ROOT "${CMAKE_CURRENT_SOURCE_DIR}")
set(XAAC_CONFIG_DIR "${CMAKE_CURRENT_BINARY_DIR}")
find_package(Threads REQUIRED)

set(CMAKE_STATIC_LIBRARY_PREFIX "")

include("${XAAC_ROOT}/cmake/utils.cmake")

libxaac_add_compile_options()
libxaac_add_definitions()

if(NOT COMPILER_HAS_SANITIZER)
  libxaac_set_link_libraries()
endif()

include("${XAAC_ROOT}/common/common.cmake")
include("${XAAC_ROOT}/decoder/libxaacdec.cmake")
include("${XAAC_ROOT}/test/decoder/xaacdec.cmake")
include("${XAAC_ROOT}/fuzzer/xaac_dec_fuzzer.cmake")

include("${XAAC_ROOT}/encoder/libxaacenc.cmake")
include("${XAAC_ROOT}/test/encoder/xaacenc.cmake")
include("${XAAC_ROOT}/fuzzer/xaac_enc_fuzzer.cmake")

