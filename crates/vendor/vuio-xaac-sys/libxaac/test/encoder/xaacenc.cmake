list(APPEND XAACENC_SRCS
     "${XAAC_ROOT}/test/encoder/impd_drc_user_config.c"
     "${XAAC_ROOT}/test/encoder/ixheaace_error.c"
     "${XAAC_ROOT}/test/encoder/ixheaace_testbench.c")

set(LIBXAACENC_INCLUDES ${XAAC_ROOT}/encoder
                        ${XAAC_ROOT}/test/encoder
                        ${XAAC_ROOT}/encoder/drc_src
                        ${XAAC_ROOT}/common)

include_directories(${LIBXAACENC_INCLUDES})

libxaac_add_executable(xaacenc libxaacenc SOURCES ${XAACENC_SRCS} INCLUDES 
                       ${LIBXAACENC_INCLUDES})

if (MSVC) 
    set_target_properties(
        xaacenc 
        PROPERTIES 
        COMPILE_FLAGS
        "-D_CRT_SECURE_NO_WARNINGS -D_X86_ -DLOUDNESS_LEVELING_SUPPORT") 
else()
    set_target_properties(
        xaacenc 
        PROPERTIES 
        COMPILE_FLAGS
        "-D_X86_ -c -O3 -Wall -Wsequence-point -Wunused-function -DLOUDNESS_LEVELING_SUPPORT"
        ) 
endif()
