# src files
list(
  APPEND
  LIBXAAC_COMMON_SRCS
  "${XAAC_ROOT}/common/ixheaac_esbr_fft.c"
  "${XAAC_ROOT}/common/ixheaac_esbr_rom.c"
  "${XAAC_ROOT}/common/ixheaac_fft_ifft_32x32_rom.c")

include_directories(${XAAC_ROOT}/common)