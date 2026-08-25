/******************************************************************************
 *                                                                            *
 * Copyright (C) 2023 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at:
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 *****************************************************************************
 * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
 */

#include <stdio.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_handler.h"

/*****************************************************************************/
/* Global memory constants                                                   */
/*****************************************************************************/
/*****************************************************************************/
/* Ittiam enhaacplus_enc ErrorCode Definitions                             */
/*****************************************************************************/
/*****************************************************************************/
/* Class 0: API Errors
*****************************************************************************/
/* Non Fatal Errors */

/* Fatal Errors */
pWORD8 ppb_ia_enhaacplus_enc_api_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Memory allocation failed", (pWORD8) "AOT unsupported"};
/*****************************************************************************/
/* Class 1: Configuration Errors
*****************************************************************************/
/* Non Fatal Errors */
pWORD8 ppb_ia_enhaacplus_enc_config_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid configuration", (pWORD8) "Insufficient bit-reservoir size"};

pWORD8 ppb_ia_enhaacplus_enc_mps_config_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid configuration", (pWORD8) "Invalid Parameter"};

pWORD8 ppb_ia_enhaacplus_enc_drc_config_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Missing configuration"};

/* Fatal Errors */
pWORD8 ppb_ia_enhaacplus_enc_config_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid sampling frequency of the stream",
    (pWORD8) "Invalid number of channels in the stream",
    (pWORD8) "Invalid stereo preprocessing flag, use 0 or 1",
    (pWORD8) "Invalid quality level",
    (pWORD8) "Invalid PCM wordsize",
    (pWORD8) "Parametric stereo not allowed with AAC classic profiles",
    (pWORD8) "Invalid TNS flag, use 0 or 1",
    (pWORD8) "Invalid channels mask",
    (pWORD8) "Invalid PCE (Program Configuration Element) flag, use 0 or 1",
    (pWORD8) "Invalid use full band width flag, use 0 or 1",
};

pWORD8 ppb_ia_enhaacplus_enc_usac_config_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid sampling frequency", (pWORD8) "Invalid resampler ratio",
    (pWORD8) "Invalid sampling frequency for usac baseline profile"};

pWORD8 ppb_ia_enhaacplus_enc_drc_config_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid configuration",
    (pWORD8) "Unsupported configuration",
    (pWORD8) "Parameter out of range",
    (pWORD8) "Compand failure",
};

/*****************************************************************************/
/* Class 2: Initialization Errors
*****************************************************************************/
/* Non Fatal Errors */

pWORD8 ppb_ia_enhaacplus_enc_mps_init_non_fatal[IA_MAX_ERROR_SUB_CODE] = {NULL};

pWORD8 ppb_ia_enhaacplus_enc_drc_init_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid DRC gain points",  (pWORD8) "Invalid start subband index" };

/* Fatal Errors */

pWORD8 ppb_ia_enhaacplus_enc_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Resampler initialization failed",
    (pWORD8) "AAC initialization failed",
    (pWORD8) "AAC Plus initialization failed",
    (pWORD8) "Bitrate not supported for the given sampling frequency",
    (pWORD8) "Invalid TNS parameter",
    (pWORD8) "Scale factor band not supported",
    (pWORD8) "Invalid core sample rate",
    (pWORD8) "Invalid element type",
    (pWORD8) "Number of channels not supported",
    (pWORD8) "Invalid number of channels in element",
    (pWORD8) "Scale factor band initalization failed",
    (pWORD8) "TNS configuration initalization failed" };

pWORD8 ppb_ia_enhaacplus_enc_mps_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "MPS Initialization failed"};

pWORD8 ppb_ia_enhaacplus_enc_usac_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Resampler initialization failed", (pWORD8) "Insufficient bit-reservoir size",
    (pWORD8) "Invalid core sample rate", (pWORD8) "Invalid element type",
    (pWORD8) "Bitbuffer initialization failed", (pWORD8) "Invalid codec mode"};

pWORD8 ppb_ia_enhaacplus_enc_drc_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid channel index"};

pWORD8 ppb_ia_enhaacplus_enc_sbr_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid number of channels",     (pWORD8) "Invalid sample rate mode",
    (pWORD8) "Invalid frequency coefficients", (pWORD8) "Invalid number of bands",
    (pWORD8) "Invalid buffer length",          (pWORD8) "SBR noise band not supported"};

pWORD8 ppb_ia_enhaacplus_enc_ps_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "PS Initialization failed" };

/*****************************************************************************/
/* Class 2: Execution Errors
*****************************************************************************/
/* Non Fatal Errors */
pWORD8 ppb_ia_enhaacplus_enc_aac_exe_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Quantization zero spectrum detected",
    (pWORD8) "Insufficient bit reservoir for non zero spectrum"};

pWORD8 ppb_ia_enhaacplus_enc_mps_exe_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Encoding Failed", (pWORD8) "Invalid MPS data bands"};

pWORD8 ppb_ia_enhaacplus_enc_usac_exe_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Quantization zero spectrum detected",
    (pWORD8) "Insufficient bit reservoir for non zero spectrum"};

pWORD8 ppb_ia_enhaacplus_enc_esbr_exe_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid bandwidth index encountered",
    (pWORD8) "Invalid number of patches",
    (pWORD8) "Invalid vocoder buffer",
    (pWORD8) "Invalid PVC mode",
    (pWORD8) "Invalid FFT size",
    (pWORD8) "Invalid start band",
    (pWORD8) "Invalid value encountered"};

/* Fatal Errors */
pWORD8 ppb_ia_enhaacplus_enc_exe_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid SBR time slots",
    (pWORD8) "Invalid SBR input channels",
    (pWORD8) "Invalid PS hybrid resolution",
    (pWORD8) "Unsupported Audio Object Type",
    (pWORD8) "Invalid block type",
    (pWORD8) "Invalid SBR frame type",
    (pWORD8) "Invalid SBR number of envelope",
    (pWORD8) "Invalid SBR bit stream",
    (pWORD8) "Invalid SBR code book",
    (pWORD8) "Invalid scale factor gain",
    (pWORD8) "Invalid bit reservoir level",
    (pWORD8) "Invalid bit consumption",
    (pWORD8) "Invalid side information bits",
    (pWORD8) "Invalid huffman bits",
    (pWORD8) "Invalid scale factor bits",
    (pWORD8) "Invalid amplitude resolution",
    (pWORD8) "Invalid output bytes",
    (pWORD8) "Invalid TNS filter order",
    (pWORD8) "Invalid SBR sample rate"};

pWORD8 ppb_ia_enhaacplus_enc_mps_exe_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Null data handle",
    (pWORD8) "Invalid Huffman data type",
    (pWORD8) "Invalid number of MPS parameter sets",
    (pWORD8) "Guided envelope shaping is not supported",
    (pWORD8) "3D stereo mode is not supported",
    (pWORD8) "Residual coding is not supported",
    (pWORD8) "Arbitrary Downmix residual coding is not supported",
    (pWORD8) "Arbitrary trees are not supported",
    (pWORD8) "Invalid quant coarse",
    (pWORD8) "Invalid res type",
    (pWORD8) "Invalid number of PCB levels",
    (pWORD8) "Error in complex FFT processing"};

pWORD8 ppb_ia_enhaacplus_enc_usac_exe_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid FAC length",
    (pWORD8) "Invalid number of SBK",
    (pWORD8) "Invalid number of channels",
    (pWORD8) "Invalid bit reservoir level",
    (pWORD8) "Invalid mapping",
    (pWORD8) "Invalid window type",
    (pWORD8) "Invalid window length",
    (pWORD8) "Invalid window shape",
};

/*****************************************************************************/
/* error info structure                                                      */
/*****************************************************************************/
/* The Module's Error Info Structure */
ia_error_info_struct ia_enhaacplus_enc_error_info = {
    /* The Module Name	*/
    (pWORD8) "libxaac encoder",
    {/* The Class Names	*/
     (pWORD8) "API", (pWORD8) "Configuration", (pWORD8) "Initialization", (pWORD8) "Execution",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) ""},
    {/* The Message Pointers	*/
     {{NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL}},
     {{NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL}}}};

VOID ia_enhaacplus_enc_error_handler_init() {
  /* The Message Pointers	*/
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][0][0] =
      ppb_ia_enhaacplus_enc_api_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][1][0] =
      ppb_ia_enhaacplus_enc_config_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][1][1] =
      ppb_ia_enhaacplus_enc_mps_config_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][1][3] =
      ppb_ia_enhaacplus_enc_drc_config_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][1][0] =
      ppb_ia_enhaacplus_enc_config_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][1][2] =
      ppb_ia_enhaacplus_enc_usac_config_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][1][3] =
      ppb_ia_enhaacplus_enc_drc_config_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][2][1] =
      ppb_ia_enhaacplus_enc_mps_init_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][2][0] =
      ppb_ia_enhaacplus_enc_init_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][2][1] =
      ppb_ia_enhaacplus_enc_mps_init_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][2][2] =
      ppb_ia_enhaacplus_enc_usac_init_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][2][3] =
      ppb_ia_enhaacplus_enc_drc_init_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][2][4] =
      ppb_ia_enhaacplus_enc_sbr_init_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][2][5] =
    ppb_ia_enhaacplus_enc_ps_init_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][3][1] =
      ppb_ia_enhaacplus_enc_mps_exe_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][3][4] =
      ppb_ia_enhaacplus_enc_esbr_exe_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][3][0] =
      ppb_ia_enhaacplus_enc_exe_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][3][1] =
      ppb_ia_enhaacplus_enc_mps_exe_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[1][3][2] =
      ppb_ia_enhaacplus_enc_usac_exe_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][3][0] =
    ppb_ia_enhaacplus_enc_aac_exe_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][3][2] =
    ppb_ia_enhaacplus_enc_usac_exe_non_fatal;
  ia_enhaacplus_enc_error_info.ppppb_error_msg_pointers[0][2][3] =
    ppb_ia_enhaacplus_enc_drc_init_non_fatal;
}

IA_ERRORCODE ia_error_handler(ia_error_info_struct *p_mod_err_info, WORD8 *pb_context,
                              IA_ERRORCODE code) {
  if (code == IA_NO_ERROR) {
    return IA_NO_ERROR;
  }
  {
    WORD32 is_fatal = (((UWORD32)code & 0x8000) >> 15);
    WORD32 err_class = (((UWORD32)code & 0x7800) >> 11);
    WORD mod_name = (((UWORD)code & 0x0700) >> 8);
    WORD32 err_sub_code = (((UWORD32)code & 0x00FF));

    printf("\n");
    if (!is_fatal) {
      printf("non ");
    }
    printf("fatal error: ");

    if (p_mod_err_info->pb_module_name != NULL) {
      printf("%s: ", p_mod_err_info->pb_module_name);
    }

    if (p_mod_err_info->pb_module_name != NULL &&
        strcmp((const char *)p_mod_err_info->pb_module_name, "ia_testbench")) {
      switch (mod_name) {
        case 0:
          printf("AAC Profile ");
          break;
        case 1:
          printf("MPEG-Surround ");
          break;
        case 2:
          printf("USAC ");
          break;
        case 3:
          printf("DRC ");
          break;
        case 4:
          printf("SBR/eSBR ");
          break;
        case 5:
          printf("Parametric Stereo ");
          break;
        default:
          break;
      }
      printf(":");
    }

    if (p_mod_err_info->ppb_class_names[err_class] != NULL) {
      printf("%s: ", p_mod_err_info->ppb_class_names[err_class]);
    }
    if (pb_context != NULL) {
      printf("%s: ", pb_context);
    }
    // printf("%s\n",
    // p_mod_err_info->ppppb_error_msg_pointers[is_fatal][err_class][err_sub_code]);
    if (err_sub_code >= IA_MAX_ERROR_SUB_CODE ||
        p_mod_err_info->ppppb_error_msg_pointers[is_fatal][err_class][mod_name][err_sub_code] ==
            NULL) {
      printf("error unlisted\n");
    } else {
      printf(
          "%s\n",
          p_mod_err_info->ppppb_error_msg_pointers[is_fatal][err_class][mod_name][err_sub_code]);
    }
  }
  return IA_NO_ERROR;
}

/*****************************************************************************/
/* ia_testbench ErrorCode Definitions                                        */
/*****************************************************************************/
/*****************************************************************************/
/* Class 0: Memory & File Manager Errors
*****************************************************************************/
/* Non Fatal Errors */
/* Fatal Errors */
pWORD8 ppb_ia_testbench_mem_file_man_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Memory Allocation Error", (pWORD8) "File Open Failed"};

/*****************************************************************************/
/* error info structure                                                      */
/*****************************************************************************/
/* The Module's Error Info Structure */
ia_error_info_struct ia_testbench_error_info = {
    /* The Module Name	*/
    (pWORD8) "ia_testbench",
    {/* The Class Names	*/
     (pWORD8) "Memory & File Manager", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) ""},
    {/* The Message Pointers	*/
     {{NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL}},
     {{NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL},
      {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
       NULL}}}};

VOID ia_testbench_error_handler_init() {
  /* The Message Pointers	*/
  ia_testbench_error_info.ppppb_error_msg_pointers[1][4][0] = ppb_ia_testbench_mem_file_man_fatal;
}
