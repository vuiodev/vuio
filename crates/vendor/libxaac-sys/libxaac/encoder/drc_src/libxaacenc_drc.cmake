# src files
list(
  APPEND
  LIBXAACENC_SRCS
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_api.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_enc.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_gain_calculator.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_gain_enc.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_mux.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_tables.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_uni_drc_eq.c"
  "${XAAC_ROOT}/encoder/drc_src/impd_drc_uni_drc_filter_bank.c")
