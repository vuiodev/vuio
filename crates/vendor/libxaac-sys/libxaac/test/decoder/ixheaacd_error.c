/******************************************************************************
 *                                                                            *
 * Copyright (C) 2018 The Android Open Source Project
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
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_error_handler.h"

/*****************************************************************************/
/* Global memory constants                                                   */
/*****************************************************************************/
/*****************************************************************************/
/* Ittiam enhaacplus_dec ErrorCode Definitions                               */
/*****************************************************************************/
/*****************************************************************************/
/* Class 0: API Errors
 */
/*****************************************************************************/
/* Non Fatal Errors */
pWORD8 ixheaacd_ppb_api_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "No Error",
    (pWORD8) "API Command not supported",
    (pWORD8) "API Command type not supported"};
/* Fatal Errors */
pWORD8 ixheaacd_ppb_api_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid Memory Table Index",
    (pWORD8) "Invalid Library ID String Index",
    (pWORD8) "NULL Pointer: Memory Allocation Error",
    (pWORD8) "Invalid Config Param",
    (pWORD8) "Invalid Execute type",
    (pWORD8) "Invalid Command",
    (pWORD8) "Memory Allocation Error: Alignment requirement not met"};
/*****************************************************************************/
/* Class 1: Configuration Errors
 */
/*****************************************************************************/
/* Non Fatal Errors */
pWORD8 ixheaacd_ppb_config_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid Output PCM WORD Size. Setting to default, 16 ",
    (pWORD8) "Invalid Down-mix flag option. Setting to default, 0 ",
    (pWORD8) "Invalid eSBR PS flag option. Setting to default, 0 ",
    (pWORD8) "Invalid downmix to stereo flag option. Setting to default, 0 ",
    (pWORD8) "Invalid interleave to stereo flag option. Setting to default, 1 ",
    (pWORD8) "Invalid downsample flag option. Setting to default, 0 ",
    (pWORD8) "Invalid peak limiter flag option. Setting to default, 1 ",
    (pWORD8) "Invalid MP4 flag option. Setting to default, 0 ",
    (pWORD8) "Invalid maximum number of channels. limiting to between 2 and 8",
    (pWORD8) "Invalid instance for coupling channel. Setting to default 1",
    (pWORD8) "Invalid error concealment flag option. Setting to default 0",
    (pWORD8) "Invalid Disable Sync flag option. Setting to default, 0 ",
    (pWORD8) "Invalid Auto SBR upsample option. Setting to default, 1 ",
    (pWORD8) "Invalid DRC heavy compression flag option. Setting to default 0",
    (pWORD8) "Invalid DRC cut value",
    (pWORD8) "Invalid DRC boost value",
    (pWORD8) "Invalid DRC target",
    (pWORD8) "Invalid Frame size",
    (pWORD8) "Invalid LD testing flag option. Setting to default 0",
    (pWORD8) "Invalid effect type",
    (pWORD8) "Invalid target loudness value",
    (pWORD8) "Invalid HQ eSBR flag option. Setting to default 0",
    (pWORD8) "Invalid frame length flag option. Setting to default 0",
    (pWORD8) "Invalid eSBR flag option. Setting to default 1"
#ifdef LOUDNESS_LEVELING_SUPPORT
    ,
    (pWORD8) "Invalid Loudness leveling flag option. Setting to default 1"
#endif
};
/* Fatal Errors */
pWORD8 ixheaacd_ppb_config_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Invalid Sample rate specified for RAW decoding"};
/*****************************************************************************/
/* Class 2: Initialization Errors
 */
/*****************************************************************************/
/* Non Fatal Errors */
pWORD8 ixheaacd_ppb_init_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Header not found at the beginning of input data continuing syncing",
    (pWORD8) "Invalid number of QMF bands", (pWORD8) "Decoder initialization failed",
    (pWORD8) "Input bytes insufficient for decoding", (pWORD8) "Error in AAC decoding"
#ifdef LOUDNESS_LEVELING_SUPPORT
    ,
    (pWORD8) "DRC instruction count exceeded"
#endif
};
/* Fatal Errors */
pWORD8 ixheaacd_ppb_init_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "AAC Decoder initialization failed",
    (pWORD8) "End of input reached during initialization",
    (pWORD8) "No. of channels in stream greater than max channels defined",
    (pWORD8) "Audio object type is not supported",
    (pWORD8) "Decoder initialization failed",
    (pWORD8) "Channel coupling not supported"};
/*****************************************************************************/
/* Class 3: Execution Errors
 */
/*****************************************************************************/
/* Non Fatal Errors */
pWORD8 ixheaacd_ppb_exe_non_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "ADTS syncronization is lost. Re-syncing",
    (pWORD8) "Though SBR was present in previous frame, not present in current"
             "frame (SBR turned off)",
    (pWORD8) "SBR was not present in previous frame, but it is present in"
             "current frame (SBR turned on)",
    (pWORD8) "ADTS Header CRC failed.Re-syncing",
    (pWORD8) "Input bytes insufficient for decoding",
    (pWORD8) "Element instance tag mismatch, because of new channel mode",
    (pWORD8) "max huffman decoded value exceeded",
    (pWORD8) "Error in AAC decoding",
    (pWORD8) "Scale factor exceeds the transmitted boundary",
    (pWORD8) "Gain control not supported",
    (pWORD8) "Filter Order of TNS data is greater than maximum order",
    (pWORD8) "LTP data found, not supported",
    (pWORD8) "The base sampling frequency has changed in ADTS header",
    (pWORD8) "Pulse Data exceeds the permitted boundary",
    (pWORD8) "Invalid code ixheaacd_book number in ia_huffman_data_type decoding",
    (pWORD8) "Channel index not within allowed range",
    (pWORD8) "Smoothing mode not within allowed range",
    (pWORD8) "Smoothing time not within allowed range",
    (pWORD8) "Extension type in the bitstream not within allowed range",
    (pWORD8) "QMF update type in the bitstream not within allowed range",
    (pWORD8) "Window type in the bitstream not within allowed range",
    (pWORD8) "Evaluated sine parameter not within allowed range"};
/* Fatal Errors */
pWORD8 ixheaacd_ppb_exe_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Channel coupling not supported",
    (pWORD8) "TNS data range is errorneous",
    (pWORD8) "Invalid LOAS header",
    (pWORD8) "Invalid DRC data",
    (pWORD8) "MPS reshaping input not valid",
    (pWORD8) "Tree config present in bit stream not valid",
    (pWORD8) "Number of timeslots not valid",
    (pWORD8) "MPS dequantization parameter not valid",
    (pWORD8) "MPS quantization mode not valid",
    (pWORD8) "MPS input channels not valid",
    (pWORD8) "Bitstream data in arbitrary downmix spatial frame not valid",
    (pWORD8) "Window sequence value not valid",
    (pWORD8) "Temporal shape config in the bitstream not valid",
    (pWORD8) "3D audio HRTF set present in the bitstream not valid",
    (pWORD8) "TTT mode read from the bitstream not valid",
    (pWORD8) "Number of OTT boxes in the bitstream not valid",
    (pWORD8) "Number of parameter sets present in the bitstream not valid",
    (pWORD8) "Mapping of index data failed during decoding",
    (pWORD8) "Number of parameter bands present in the bitstream not valid"};

/*****************************************************************************/
/* error info structure                                                      */
/*****************************************************************************/
/* The Module's Error Info Structure */
ia_error_info_struct ixheaacd_error_info = {
    /* The Module Name  */
    (pWORD8) "Ittiam xheaac_dec",
    {/* The Class Names */
     (pWORD8) "API", (pWORD8) "Configuration", (pWORD8) "Initialization",
     (pWORD8) "Execution", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "xHeaac"},
    {/* The Message Pointers    */
     {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
      NULL, NULL, NULL, NULL},
     {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
      NULL, NULL, NULL, NULL}}};

/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_error_handler_init                     */
/*                                                                           */
/*  Description   : Initialize the error struct with string pointers         */
/*                                                                           */
/*  Inputs        : none                                                     */
/*                                                                           */
/*  Globals       : ia_error_info_struct ixheaacd_error_info        */
/*                  pWORD8 ixheaacd_ppb_api_non_fatal               */
/*                  pWORD8 ixheaacd_ppb_api_fatal                   */
/*                  pWORD8 ixheaacd_ppb_config_non_fatal            */
/*                  pWORD8 ixheaacd_ppb_config_fatal                */
/*                  pWORD8 ixheaacd_ppb_init_non_fatal              */
/*                  pWORD8 ixheaacd_ppb_init_fatal                  */
/*                  pWORD8 ixheaacd_ppb_exe_non_fatal               */
/*                  pWORD8 ixheaacd_ppb_exe_fatal                   */
/*                                                                           */
/*  Processing    : Init the struct with error string pointers               */
/*                                                                           */
/*  Outputs       : none                                                     */
/*                                                                           */
/*  Returns       : none                                                     */
/*                                                                           */
/*  Issues        : none                                                     */
/*                                                                           */
/*  Revision history :                                                       */
/*                                                                           */
/*        DD MM YYYY       Author                Changes                     */
/*        29 07 2005       Ittiam                Created                     */
/*                                                                           */
/*****************************************************************************/

VOID ixheaacd_error_handler_init() {
  /* The Message Pointers   */
  ixheaacd_error_info.ppppb_error_msg_pointers[0][0] =
      ixheaacd_ppb_api_non_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[1][0] = ixheaacd_ppb_api_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[0][1] =
      ixheaacd_ppb_config_non_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[1][1] =
      ixheaacd_ppb_config_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[0][2] =
      ixheaacd_ppb_init_non_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[1][2] = ixheaacd_ppb_init_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[0][3] =
      ixheaacd_ppb_exe_non_fatal;
  ixheaacd_error_info.ppppb_error_msg_pointers[1][3] = ixheaacd_ppb_exe_fatal;
}

/*****************************************************************************/
/* ia_testbench ErrorCode Definitions                                        */
/*****************************************************************************/
/*****************************************************************************/
/* Class 0: Memory & File Manager Errors
 */
/*****************************************************************************/
/* Non Fatal Errors */
/* Fatal Errors */
pWORD8 ixheaacd_ppb_ia_testbench_mem_file_man_fatal[IA_MAX_ERROR_SUB_CODE] = {
    (pWORD8) "Memory Allocation Error", (pWORD8) "File Open Failed"};

/*****************************************************************************/
/* error info structure                                                      */
/*****************************************************************************/
/* The Module's Error Info Structure */
ia_error_info_struct ixheaacd_ia_testbench_error_info = {
    /* The Module Name  */
    (pWORD8) "ia_testbench",
    {/* The Class Names */
     (pWORD8) "Memory & File Manager", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "", (pWORD8) "",
     (pWORD8) "", (pWORD8) ""},
    {/* The Message Pointers    */
     {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
      NULL, NULL, NULL, NULL},
     {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
      NULL, NULL, NULL, NULL}}};

/*****************************************************************************/
/*                                                                           */
/*  Function name : ia_testbench_error_handler_init                          */
/*                                                                           */
/*  Description   : Initialize the error struct with string pointers         */
/*                                                                           */
/*  Inputs        : none                                                     */
/*                                                                           */
/*  Globals       : ia_error_info_struct ixheaacd_ia_testbench_error_info */
/*                  pWORD8 ixheaacd_ppb_ia_testbench_mem_file_man_fatal */
/*                                                                           */
/*  Processing    : Init the struct with error string pointers               */
/*                                                                           */
/*  Outputs       : none                                                     */
/*                                                                           */
/*  Returns       : none                                                     */
/*                                                                           */
/*  Issues        : none                                                     */
/*                                                                           */
/*  Revision history :                                                       */
/*                                                                           */
/*        DD MM YYYY       Author                Changes                     */
/*        29 07 2005       Ittiam                Created                     */
/*                                                                           */
/*****************************************************************************/

VOID ia_testbench_error_handler_init() {
  /* The Message Pointers   */
  ixheaacd_ia_testbench_error_info.ppppb_error_msg_pointers[1][0] =
      ixheaacd_ppb_ia_testbench_mem_file_man_fatal;
}

/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_error_handler */
/*                                                                           */
/*  Description   : Called Prints the status error code from the err_info    */
/*                                                                           */
/*  Inputs        : ia_error_info_struct *p_mod_err_info (Error info struct) */
/*                  WORD8 *pb_context (Context of error)                     */
/*                  IA_ERRORCODE code (Error code)                           */
/*                                                                           */
/*  Globals       : none                                                     */
/*                                                                           */
/*  Processing    : whenever any module calls the errorhandler,  it  informs */
/*                  it about the module for which it is called and a context */
/*                  in which it was  called  in addition to  the  error_code */
/*                  the message is displayed  based  on the  module's  error */
/*                  message  array  that maps to  the error_code the context */
/*                  gives specific info in which the error occured  e.g. for */
/*                  testbench   module,  memory  allocator   can   call  the */
/*                  errorhandler   for  memory  inavailability  in   various */
/*                  contexts like input_buf or output_buf e.g.  for  mp3_enc */
/*                  module, there can be various instances running.  context */
/*                  can be used to  identify  the  particular  instance  the */
/*                  error handler is being called for                        */
/*                                                                           */
/*  Outputs       : None                                                     */
/*                                                                           */
/*  Returns       : IA_ERRORCODE error_value  (Error value)                  */
/*                                                                           */
/*  Issues        : none                                                     */
/*                                                                           */
/*  Revision history :                                                       */
/*                                                                           */
/*        DD MM YYYY       Author                Changes                     */
/*        29 07 2005       Tejaswi/Vishal        Created                     */
/*                                                                           */
/*****************************************************************************/

IA_ERRORCODE ixheaacd_error_handler(ia_error_info_struct *p_mod_err_info,
                                    WORD8 *pb_context, IA_ERRORCODE code) {
  if (code == IA_NO_ERROR) {
    return IA_NO_ERROR;
  }
  {
    WORD is_fatal, err_class, err_sub_code;

    if (code == IA_FATAL_ERROR)
      is_fatal = 1;
    else
      is_fatal = (((UWORD)code & 0x8000) >> 15);

    err_class = (((UWORD)code & 0x7800) >> 11);
    err_sub_code = (((UWORD)code & 0x07FF));

    if (!is_fatal) {
      printf("non ");
    }
    printf("fatal error: ");

    if (p_mod_err_info->pb_module_name != NULL) {
      printf("%s: ", p_mod_err_info->pb_module_name);
    }
    if (p_mod_err_info->ppb_class_names[err_class] != NULL) {
      printf("%s: ", p_mod_err_info->ppb_class_names[err_class]);
    }
    if (pb_context != NULL) {
      printf("%s: ", pb_context);
    }
    if (err_sub_code >= IA_MAX_ERROR_SUB_CODE ||
        p_mod_err_info->ppppb_error_msg_pointers[is_fatal][err_class]
                                                [err_sub_code] == NULL) {
      printf("error unlisted");
    } else {
      printf("%s\n",
             p_mod_err_info
                 ->ppppb_error_msg_pointers[is_fatal][err_class][err_sub_code]);
    }
  }
  return IA_NO_ERROR;
}