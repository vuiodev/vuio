list(APPEND XAAC_ENC_FUZZER_SRCS "${XAAC_ROOT}/fuzzer/xaac_enc_fuzzer.cpp")

set(LIBXAACENC_INCLUDES ${XAAC_ROOT}/encoder ${XAAC_ROOT}/test/encoder ${XAAC_ROOT}/common)

libxaac_add_fuzzer(xaac_enc_fuzzer libxaacenc SOURCES ${XAAC_ENC_FUZZER_SRCS}
                   INCLUDES ${LIBXAACENC_INCLUDES})
