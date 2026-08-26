cmake_minimum_required(VERSION 3.5.1)
project(libxaac C ASM)

if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64|ARM64)$")
  set(CMAKE_SYSTEM_PROCESSOR "aarch64")
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64|AMD64)$")
  set(CMAKE_SYSTEM_PROCESSOR "x86_64")
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(i.86|x86)$")
  set(CMAKE_SYSTEM_PROCESSOR "i686")
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(arm.*|aarch32)$")
  set(CMAKE_SYSTEM_PROCESSOR "aarch32")
endif()

option(BUILD64 "Build for 64 bit" OFF)
option(BUILD_TESTS "Build tests and fuzzers" OFF)
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
include("${XAAC_ROOT}/encoder/libxaacenc.cmake")

if(BUILD_TESTS)
  include("${XAAC_ROOT}/test/decoder/xaacdec.cmake")
  include("${XAAC_ROOT}/fuzzer/xaac_dec_fuzzer.cmake")
  include("${XAAC_ROOT}/test/encoder/xaacenc.cmake")
  include("${XAAC_ROOT}/fuzzer/xaac_enc_fuzzer.cmake")
endif()

