list(APPEND XAAC_DEC_FUZZER_SRCS "${XAAC_ROOT}/fuzzer/xaac_dec_fuzzer.cpp")

set(LIBXAACDEC_INCLUDES ${XAAC_ROOT}/decoder ${XAAC_ROOT}/test
                        ${XAAC_ROOT}/decoder/drc_src)

libxaac_add_fuzzer(xaac_dec_fuzzer libxaacdec SOURCES ${XAAC_DEC_FUZZER_SRCS}
                   INCLUDES ${LIBXAACDEC_INCLUDES})
