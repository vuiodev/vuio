# src files
list(
  APPEND
  LIBXAACDEC_SRCS
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_function_selector_armv8.c"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_qmf_dec_armv8.c"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_apply_scale_factors.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_calcmaxspectralline.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_cos_sin_mod_loop1.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_cos_sin_mod_loop2.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_fft32x32_ld2_armv8.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_inv_dit_fft_8pt.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_imdct_using_fft.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_no_lap1.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_overlap_add1.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_overlap_add2.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_post_twiddle.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_post_twiddle_overlap.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_postradixcompute4.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_pre_twiddle.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_postradixcompute4.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_sbr_imdct_using_fft.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_sbr_qmf_analysis32_neon.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_sbr_qmfsyn64_winadd.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_shiftrountine_with_round.s"
  "${XAAC_ROOT}/decoder/armv8/ixheaacd_shiftrountine_with_round_eld.s")

include_directories(${XAAC_ROOT}/decoder/armv8})
