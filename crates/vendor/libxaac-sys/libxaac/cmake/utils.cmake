include(CheckCXXCompilerFlag)
set(CMAKE_C_STANDARD 99)
# Adds compiler options for all targets
function(libxaac_add_compile_options)
  if(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    add_compile_options(-std=gnu99 -march=armv8-a)
  elseif(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch32")
    add_compile_options(
      -O3
      -Wall
      -std=c99
      -mcpu=cortex-a8
      -march=armv7-a
      -mfloat-abi=softfp
      -mfpu=neon)
  elseif(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "x86_64")
    add_compile_options(-D_X86_ -O3 -Wall -Wsequence-point -Wunused-function -fwrapv)
  elseif(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "i686")
    add_compile_options(-D_X86_ -O3 -Wall -Wsequence-point -Wunused-function -fwrapv -m32)
  endif()

  set(CMAKE_REQUIRED_FLAGS -fsanitize=fuzzer-no-link)
  check_cxx_compiler_flag(-fsanitize=fuzzer-no-link
                          COMPILER_HAS_SANITIZE_FUZZER)
  unset(CMAKE_REQUIRED_FLAGS)

  if(DEFINED SANITIZE)
    set(CMAKE_REQUIRED_FLAGS -fsanitize=${SANITIZE})
    check_cxx_compiler_flag(-fsanitize=${SANITIZE} COMPILER_HAS_SANITIZER)
    unset(CMAKE_REQUIRED_FLAGS)

    if(NOT COMPILER_HAS_SANITIZER)
      message(
        FATAL_ERROR "ERROR: Compiler doesn't support -fsanitize=${SANITIZE}")
      return()
    endif()
    add_compile_options(-fno-omit-frame-pointer -fsanitize=${SANITIZE})
  endif()

endfunction()

# Adds defintions for all targets
function(libxaac_add_definitions)
  if(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    add_definitions(-DARMV8)
  elseif(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch32")
    add_definitions(-DARMV7)
  elseif(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "i686")
    add_definitions(-DX86 -D_X86_)
  elseif(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "x86_64")
    add_definitions(-DX86_64 -D_X86_64_)
  endif()
  add_definitions(-DLOUDNESS_LEVELING_SUPPORT)
endfunction()

# Adds libraries needed for executables
function(libxaac_set_link_libraries)
  if(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch32")
    link_libraries(Threads::Threads --static)
  endif()
endfunction()

# cmake-format: off
# Adds a target for an executable
#
# Arguments:
# NAME: Name of the executatble
# LIB: Library that executable depends on
# SOURCES: Source files
#
# Optional Arguments:
# INCLUDES: Include paths
# LIBS: Additional libraries
# FUZZER: flag to specify if the target is a fuzzer binary
# cmake-format: on

function(libxaac_add_executable NAME LIB)
  set(multi_value_args SOURCES INCLUDES LIBS)
  set(optional_args FUZZER)
  cmake_parse_arguments(ARG "${optional_args}" "${single_value_args}"
                        "${multi_value_args}" ${ARGN})

  # Check if compiler supports -fsanitize=fuzzer. If not, skip building fuzzer
  # binary
  if(ARG_FUZZER)
    if(NOT COMPILER_HAS_SANITIZE_FUZZER)
      message("Compiler doesn't support -fsanitize=fuzzer. Skipping ${NAME}")
      return()
    endif()
  endif()

  add_executable(${NAME} ${ARG_SOURCES})
  target_include_directories(${NAME} PRIVATE ${ARG_INCLUDES})
  add_dependencies(${NAME} ${LIB} ${ARG_LIBS})

  if(NOT MSVC)
    target_link_libraries(${NAME} ${LIB} ${ARG_LIBS} m)
  else()
    target_link_libraries(${NAME} ${LIB} ${ARG_LIBS})
  endif()
  
  if(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "i686")
	set_target_properties(${NAME}  PROPERTIES LINK_FLAGS "-m32")
  endif()
  
  if(ARG_FUZZER)
    target_compile_options(${NAME}
                           PRIVATE $<$<COMPILE_LANGUAGE:CXX>:-std=c++17>)
    if(DEFINED ENV{LIB_FUZZING_ENGINE})
      set_target_properties(${NAME} PROPERTIES LINK_FLAGS
                                               $ENV{LIB_FUZZING_ENGINE})
    elseif(DEFINED SANITIZE)
      set_target_properties(${NAME} PROPERTIES LINK_FLAGS
                                               -fsanitize=fuzzer,${SANITIZE})
    else()
      set_target_properties(${NAME} PROPERTIES LINK_FLAGS -fsanitize=fuzzer)
    endif()
  else()
    if(DEFINED SANITIZE)
      set_target_properties(${NAME} PROPERTIES LINK_FLAGS
                                               -fsanitize=${SANITIZE})
    endif()
  endif()
endfunction()

# cmake-format: off
# Adds a target for a fuzzer binary
# Calls libxaac_add_executable with all arguments with FUZZER set to 1
# Arguments:
# Refer to libxaac_add_executable's arguments
# cmake-format: on

function(libxaac_add_fuzzer NAME LIB)
  libxaac_add_executable(${NAME} ${LIB} FUZZER 1 ${ARGV})
endfunction()
