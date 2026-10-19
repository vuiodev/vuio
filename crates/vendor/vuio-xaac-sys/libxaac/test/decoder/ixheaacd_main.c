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
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

#define AOSP_CHANGE

#include "ixheaacd_fileifc.h"
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_error_handler.h"
#include "ixheaacd_apicmd_standards.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_aac_config.h"
#include "ixheaacd_metadata_read.h"
#include "impd_drc_config_params.h"

IA_ERRORCODE ixheaacd_dec_api(pVOID p_ia_module_obj, WORD32 i_cmd, WORD32 i_idx,
                              pVOID pv_value);

IA_ERRORCODE ia_drc_dec_api(pVOID p_ia_module_obj, WORD32 i_cmd, WORD32 i_idx,
                            pVOID pv_value);

VOID ixheaacd_error_handler_init();
VOID ia_testbench_error_handler_init();
VOID metadata_mp4_stsz_size_free(metadata_info *meta_info);

extern ia_error_info_struct ixheaacd_ia_testbench_error_info;
extern ia_error_info_struct ixheaacd_error_info;

/*****************************************************************************/
/* Process select hash defines                                               */
/*****************************************************************************/
#define WAV_HEADER
#define DISPLAY_MESSAGE

/*****************************************************************************/
/* Constant hash defines                                                     */
/*****************************************************************************/
#define MAX_STACK_PROC 10
#define MAX_MEM_ALLOCS 100
#define IA_MAX_CMD_LINE_LENGTH 300
#define IA_MAX_ARGS 20
#define IA_SCREEN_WIDTH 80

/*****************************************************************************/
/* Error codes for the testbench                                             */
/*****************************************************************************/
#define IA_TESTBENCH_MFMAN_FATAL_MEM_ALLOC_FAILED 0xFFFF8000
#define IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED 0xFFFF8001

#define IA_HE_AAC_DEC_TABLE_RELOCATABLE_ENABLE 0

#ifdef ARM_PROFILE_HW
#include <sys/time.h>
#define CLK_FREQ_BOARD_MHZ 716 // a9 omap4430 board
//#define CLK_FREQ_BOARD_MHZ 1555 //Nexus6P
//#define CLK_FREQ_BOARD_MHZ 2035 //Tegra Board
//#define CLK_FREQ_BOARD_MHZ 550 //a8  board
//#define CLK_FREQ_BOARD_MHZ 297 //dm365 board
//#define CLK_FREQ_BOARD_MHZ 1209.6 //a5 board
#endif
#ifdef ARM_PROFILE_HW
long long itGetMs(void) {
  struct timeval t;
  long long currTime;

  if (gettimeofday(&t, NULL) == -1) {
    printf("Error in gettimeofday. It has returned -1. \n");
  }
  currTime = ((t.tv_sec * 1000 * 1000) + (t.tv_usec));
  return currTime;
}
#endif
/*****************************************************************************/
/* Global variables                                                          */
/*****************************************************************************/
pVOID g_pv_arr_alloc_memory[MAX_MEM_ALLOCS];
WORD g_w_malloc_count;
FILE *g_pf_out;
FileWrapperPtr g_pf_inp; /* file pointer to bitstream file (mp4) */

WORD32 mpeg_d_drc_on = 0;

metadata_info meta_info;  // metadata pointer;
WORD32 ixheaacd_i_bytes_to_read;
WORD32 prev_i_bytes_to_read;
WORD32 flush_frame = 0;
FILE *g_pf_meta;

WORD32 raw_testing = 0;
WORD32 eld_testing = 0;
WORD32 ec_enable = 0;
WORD32 esbr_testing = 1;

#define _IA_PRINT_ERROR(p_mod_err_info, context, e)           \
  if ((e) != IA_NO_ERROR) {                                   \
    ixheaacd_error_handler((p_mod_err_info), (context), (e)); \
  }

#ifndef WIN32
int ia_fwrite(pVOID buffer[], int size, int nwords, FILE *fp) {
  int i, j;
  pWORD8 pb_buf = (pWORD8)buffer;

  for (i = 0; i < nwords; i++) {
    for (j = 0; j < size; j++) {
      putc(pb_buf[i * size + j], fp);
    }
  }
  return 1;
}

#endif

#ifdef WAV_HEADER
#ifndef ARM_PROFILE_BOARD
/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_write16_bits_lh */
/*                                                                           */
/*  Description   : write 16 bits low high (always little endian)            */
/*                                                                           */
/*  Inputs        : none                                                     */
/*                                                                           */
/*  Globals       : pVOID g_pv_arr_alloc_memory[MAX_MEM_ALLOCS];             */
/*                  WORD  g_w_malloc_count;                                  */
/*                  FILE *g_pf_inp, *g_pf_out;                               */
/*                                                                           */
/*  Processing    : write 16 bits low high (always little endian)            */
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

static VOID ixheaacd_write16_bits_lh(FILE *fp, WORD32 i) {
  putc(i & 0xff, fp);
  putc((i >> 8) & 0xff, fp);
}

/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_write32_bits_lh */
/*                                                                           */
/*  Description   : write 32 bits low high (always little endian)            */
/*                                                                           */
/*  Inputs        : none                                                     */
/*                                                                           */
/*  Globals       : FILE* fp (file to write)                                 */
/*                  WORD32 i (value to write)                                */
/*                                                                           */
/*  Processing    : write 32 bits low high (always little endian)            */
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

static VOID ixheaacd_write32_bits_lh(FILE *fp, WORD32 i) {
  ixheaacd_write16_bits_lh(fp, (WORD32)(i & 0xffffL));
  ixheaacd_write16_bits_lh(fp, (WORD32)((i >> 16) & 0xffffL));
}

/*****************************************************************************/
/*                                                                           */
/*  Function name : write_wav_header                                         */
/*                                                                           */
/*  Description   : Write wav header to a wav file                           */
/*                                                                           */
/*  Inputs        : none                                                     */
/*                                                                           */
/*  Globals       : FILE* fp (file to write)                                 */
/*                  WORD32 pcmbytes (total bytes in wav file)                */
/*                  WORD32 freq (sampling freq)                              */
/*                  WORD32 channels (output channels)                        */
/*                  WORD32 bits (bits per sample)                            */
/*                                                                           */
/*  Processing    : Write wav header                                         */
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

WORD32 write_wav_header(FILE *fp, WORD32 pcmbytes, WORD32 freq, WORD32 channels,
                        WORD32 bits, WORD32 i_channel_mask) {
  if (channels > 2) {
    WORD32 bytes = (bits + 7) / 8;
    fwrite("RIFF", 1, 4, fp); /* label */
    ixheaacd_write32_bits_lh(
        fp, pcmbytes + 44 - 8);   /* length in bytes without header */
    fwrite("WAVEfmt ", 2, 4, fp); /* 2 labels */
    /* tag for WAVE_FORMAT_EXTENSIBLE */
    if (channels > 2) {
      ixheaacd_write16_bits_lh(fp, 0x28);
      ixheaacd_write16_bits_lh(fp, 0x00);
      ixheaacd_write16_bits_lh(fp, 0xfffe);
    } else {
      ixheaacd_write32_bits_lh(
          fp, 2 + 2 + 4 + 4 + 2 + 2);  /* length of PCM format decl area */
      ixheaacd_write16_bits_lh(fp, 1); /* is pcm? */
    }

    ixheaacd_write16_bits_lh(fp, channels);
    ixheaacd_write32_bits_lh(fp, freq);
    ixheaacd_write32_bits_lh(fp, freq * channels * bytes); /* bps */
    ixheaacd_write16_bits_lh(fp, channels * bytes);
    ixheaacd_write16_bits_lh(fp, bits);

    /* tag for WAVE_FORMAT_EXTENSIBLE */
    if (channels > 2) {
      ixheaacd_write16_bits_lh(fp, 0x16);
      ixheaacd_write16_bits_lh(fp, 0x10);           /*Samples.wReserved*/
      ixheaacd_write32_bits_lh(fp, i_channel_mask); /* dwChannelMask */

      ixheaacd_write32_bits_lh(fp, 0x0001); /* SubFormat.Data1 */
      ixheaacd_write32_bits_lh(
          fp, 0x00100000); /* SubFormat.Data2 and SubFormat.Data3 */

      ixheaacd_write16_bits_lh(fp, 0x0080);
      ixheaacd_write16_bits_lh(fp, 0xAA00);

      ixheaacd_write16_bits_lh(fp, 0x3800);
      ixheaacd_write16_bits_lh(fp, 0x719b);
    }

    fwrite("data", 1, 4, fp);
    ixheaacd_write32_bits_lh(fp, pcmbytes);

    return (ferror(fp) ? -1 : 0);

  } else {
    WORD32 bytes = (bits + 7) / 8;
    fwrite("RIFF", 1, 4, fp); /* label */
    ixheaacd_write32_bits_lh(
        fp, pcmbytes + 44 - 8);   /* length in bytes without header */
    fwrite("WAVEfmt ", 2, 4, fp); /* 2 labels */
    ixheaacd_write32_bits_lh(
        fp, 2 + 2 + 4 + 4 + 2 + 2);  /* length of PCM format decl area */
    ixheaacd_write16_bits_lh(fp, 1); /* is pcm? */
    ixheaacd_write16_bits_lh(fp, channels);
    ixheaacd_write32_bits_lh(fp, freq);
    ixheaacd_write32_bits_lh(fp, freq * channels * bytes); /* bps */
    ixheaacd_write16_bits_lh(fp, channels * bytes);
    ixheaacd_write16_bits_lh(fp, bits);
    fwrite("data", 1, 4, fp);
    ixheaacd_write32_bits_lh(fp, pcmbytes);

    return (ferror(fp) ? -1 : 0);
  }
}
#endif /* WAV_HEADER */
#endif /*ARM_PROFILE_BOARD*/

#ifdef DISPLAY_MESSAGE

/*****************************************************************************/
/*                                                                           */
/*  Function name : ia_display_id_message                                    */
/*                                                                           */
/*  Description   : Display the ID message of the process                    */
/*                                                                           */
/*  Inputs        : WORD8 lib_name[] (library name)                          */
/*                  WORD8 lib_version[] (library version)                    */
/*                  WORD8 api_version[] (API version)                        */
/*                                                                           */
/*  Globals       : none                                                     */
/*                                                                           */
/*  Processing    : Display all the information about the process            */
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
/*        29 07 2005       Tejaswi/Vishal        Created                     */
/*                                                                           */
/*****************************************************************************/

VOID ia_display_id_message(WORD8 lib_name[], WORD8 lib_version[]) {
  WORD8 str[4][IA_SCREEN_WIDTH] = {"ITTIAM SYSTEMS PVT LTD, BANGALORE\n",
                                   "http:\\\\www.ittiam.com\n", "", ""};
  WORD8 spaces[IA_SCREEN_WIDTH / 2 + 1];
  WORD32 i, spclen;

  strcpy((pCHAR8)str[2], (pCHAR8)lib_name);
  strcat((pCHAR8)str[2], (pCHAR8)lib_version);
  strcat((pCHAR8)str[2], "\n");
  strcat((pCHAR8)str[4 - 1], "\n");

  for (i = 0; i < IA_SCREEN_WIDTH / 2 + 1; i++) {
    spaces[i] = ' ';
  }

  for (i = 0; i < 4; i++) {
    spclen = IA_SCREEN_WIDTH / 2 - (WORD32)strlen((pCHAR8)str[i]) / 2;
    spaces[spclen] = '\0';
    printf("%s", (pCHAR8)spaces);
    spaces[spclen] = ' ';
    printf("%s", (pCHAR8)str[i]);
  }
}
#endif /* DISPLAY_MESSAGE */

/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_set_config_param                       */
/*                                                                           */
/*  Description   : Set config parameters                                    */
/*                                                                           */
/*  Inputs        : pVOID p_ia_process_api_obj (process API obj)             */
/*                  WORD32 argc (Arguments count)                            */
/*                  pWORD8 argv[] (Argument strings)                         */
/*                                                                           */
/*  Globals       : none                                                     */
/*                                                                           */
/*  Processing    : Set config params inside API                             */
/*                                                                           */
/*  Outputs       : none                                                     */
/*                                                                           */
/*  Returns       : IA_ERRORCODE error_value  (Error value)                  */
/*                                                                           */
/*  Issues        : none                                                     */
/*                                                                           */
/*  Revision history :                                                       */
/*                                                                           */
/*        DD MM YYYY       Author                Changes                     */
/*        29 07 2005       Ittiam                Created                     */
/*                                                                           */
/*****************************************************************************/

IA_ERRORCODE ixheaacd_set_config_param(WORD32 argc, pWORD8 argv[],
                                       pVOID p_ia_process_api_obj) {
  LOOPIDX i;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  /* the process API function */
  IA_ERRORCODE(*p_ia_process_api)
  (pVOID p_ia_process_api_obj, WORD32 i_cmd, WORD32 i_idx, pVOID pv_value) =
      ixheaacd_dec_api;
  ia_error_info_struct *p_proc_err_info = &ixheaacd_error_info;

  for (i = 0; i < argc; i++) {
    /* To indicate if its a MP4 file or not. */
    if (!strncmp((pCHAR8)argv[i], "-mp4:", 5)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 5);
      UWORD32 ui_mp4_flag = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_MP4FLAG, &ui_mp4_flag);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* PCM WORD Size (For single input file) */
    if (!strncmp((pCHAR8)argv[i], "-pcmsz:", 7)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 7);
      UWORD32 ui_pcm_wd_sz = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_PCM_WDSZ, &ui_pcm_wd_sz);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* Down-mix stereo to mono. */
    if (!strncmp((pCHAR8)argv[i], "-dmix:", 6)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 6);
      UWORD32 ui_down_mix = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DOWNMIX, &ui_down_mix);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-esbr_hq:", 9)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 9);
      UWORD32 ui_esbr_hq = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_HQ_ESBR, &ui_esbr_hq);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-esbr_ps:", 9)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 9);
      UWORD32 ui_esbr_ps = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_PS_ENABLE, &ui_esbr_ps);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-esbr:", 6)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 6);
      UWORD32 ui_esbr = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_ESBR, &ui_esbr);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      esbr_testing = ui_esbr;
    }
#ifdef RESAMPLE_SUPPORT
    /* Resample the output to 8 kHz. */
    if (!strncmp((pCHAR8)argv[i], "-f08:", 5)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 5);
      UWORD32 ui_08khz_out = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_PARAM_OUT08KHZ, &ui_08khz_out);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* Resample the output to 16 kHz. */
    if (!strncmp((pCHAR8)argv[i], "-f16:", 5)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 5);
      UWORD32 ui_16khz_out = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_PARAM_OUT16KHZ, &ui_16khz_out);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
#endif
    /* Interleave mono output to stereo */
    if (!strncmp((pCHAR8)argv[i], "-tostereo:", 10)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 10);
      UWORD32 ui_to_stereo = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_TOSTEREO, &ui_to_stereo);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* Downsampled synthesis to be used */
    if (!strncmp((pCHAR8)argv[i], "-dsample:", 9)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 9);
      UWORD32 ui_dsample = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DSAMPLE, &ui_dsample);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

#ifdef HEAACV2_AS_AACLC
    /* To indicate if its a MP4 file or not. */
    if (!strncmp((pCHAR8)argv[i], "-aac_lc_only:", 13)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 13);
      UWORD32 ui_aac_lc_only = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_PARAM_AAC_ONLY, &ui_aac_lc_only);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
#endif

#ifdef LATM_LOAS
    /* To indicate if its a LOAS file or not. */
    if (!strncmp((pCHAR8)argv[i], "-isLOAS:", 8)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 8);
      UWORD32 ui_loas_flag = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_PARAM_ISLOAS, &ui_loas_flag);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

#endif

#ifdef DRC_ENABLE
    if (!strncmp((pCHAR8)argv[i], "-drc_cut_fac:", 13)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 13);
      FLOAT32 drc_cut = (FLOAT32)atof(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_CUT, &drc_cut);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-drc_boost_fac:", 15)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 15);
      FLOAT32 drc_boost = (FLOAT32)atof(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_BOOST, &drc_boost);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-drc_target_level:", 18)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 18);
      UWORD32 ui_drc_target = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LEVEL, &ui_drc_target);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-drc_heavy_comp:", 16)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 16);
      UWORD32 ui_drc_heavy_comp = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_HEAVY_COMP, &ui_drc_heavy_comp);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

#endif
    /* For MPEG-D DRC effect type */
    if (!strncmp((pCHAR8)argv[i], "-effect:", 8)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 8);
      WORD32 ui_effect = atoi(pb_arg_val);
      err_code =
          (*p_ia_process_api)(p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
              IA_XHEAAC_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE, &ui_effect);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      mpeg_d_drc_on = 1;
    }
    /* For MPEG-D DRC target loudness */
    if (!strncmp((pCHAR8)argv[i], "-target_loudness:", 17)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 17);
      WORD32 ui_target_loudness = atoi(pb_arg_val);
      if ((ui_target_loudness > 0) || (ui_target_loudness < -63)) {
          ui_target_loudness = 0;
      }
      ui_target_loudness = -(ui_target_loudness << 2);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS, &ui_target_loudness);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      mpeg_d_drc_on = 1;
    }
    /* To indicate if its a MP4 file or not. */
    if (!strncmp((pCHAR8)argv[i], "-nosync:", 8)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 8);
      UWORD32 ui_disable_sync = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_DISABLE_SYNC, &ui_disable_sync);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* To indicate SBR upsampling. */
    if (!strncmp((pCHAR8)argv[i], "-sbrup:", 7)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 7);
      UWORD32 ui_auto_sbr_upsample = atoi(pb_arg_val);
      err_code =
          (*p_ia_process_api)(p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                              IA_XHEAAC_DEC_CONFIG_PARAM_AUTO_SBR_UPSAMPLE,
                              &ui_auto_sbr_upsample);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* To indicate frame length for a RAW bit-stream. */
    if (!strncmp((pCHAR8)argv[i], "-flflag:", 8)) {
        pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 8);
        UWORD32 ui_fl_flag = atoi(pb_arg_val);
        err_code =
            (*p_ia_process_api)(p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                                IA_XHEAAC_DEC_CONFIG_PARAM_FRAMELENGTH_FLAG,
                                &ui_fl_flag);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /* To indicate sample rate for a RAW bit-stream. */
    if (!strncmp((pCHAR8)argv[i], "-fs:", 4)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 4);
      UWORD32 ui_samp_freq = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_SAMP_FREQ, &ui_samp_freq);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* To indicate the number of maximum channels */
    if (!strncmp((pCHAR8)argv[i], "-maxchannel:", 12)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 12);
      UWORD32 ui_max_channel = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_MAX_CHANNEL, &ui_max_channel);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /* To indicate the number of coupling channels to be used for coupling */
    if (!strncmp((pCHAR8)argv[i], "-coupchannel:", 13)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 13);
      UWORD32 ui_coupling_channel = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_COUP_CHANNEL, &ui_coupling_channel);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /* Down-mix N.1 to stereo */
    if (!strncmp((pCHAR8)argv[i], "-downmix:", 9)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 9);
      UWORD32 ui_downmix = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DOWNMIX_STEREO, &ui_downmix);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /* For LD files, to indicate  */
    if (!strncmp((pCHAR8)argv[i], "-fs480:", 7)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 7);
      UWORD32 ui_fs480 = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_FRAMESIZE, &ui_fs480);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    if (!strncmp((pCHAR8)argv[i], "-ld_testing:", 12)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 12);
      UWORD32 ld_testing = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(
          p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_LD_TESTING, &ld_testing);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    if (!strncmp((pCHAR8)argv[i], "-peak_limiter_off:", 18))
    {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 18);
      UWORD32 peak_limiter_flag = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_PEAK_LIMITER, &peak_limiter_flag);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* For Error concealment */
    if (!strncmp((pCHAR8)argv[i], "-err_conceal:", 13))
    {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 13);
      UWORD32 ui_err_conceal = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                                     IA_XHEAAC_DEC_CONFIG_ERROR_CONCEALMENT, &ui_err_conceal);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      ec_enable = ui_err_conceal;
    }
#ifdef LOUDNESS_LEVELING_SUPPORT
    /* For loudness leveling*/
    if (!strncmp((pCHAR8)argv[i], "-loudness_leveling:", 19)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 19);
      UWORD32 loudness_leveling_flag = atoi(pb_arg_val);
      err_code = (*p_ia_process_api)(p_ia_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                                     IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING,
                                     &loudness_leveling_flag);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      mpeg_d_drc_on = 1;
    }
#endif
  }

  return IA_NO_ERROR;
}

/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_get_config_param                       */
/*                                                                           */
/*  Description   : Get config parameters                                    */
/*                                                                           */
/*  Inputs        : pVOID p_ia_process_api_obj (process API obj)             */
/*                  pWORD32 pi_samp_freq (Ptr for samp freq param)           */
/*                  pWORD32 pi_num_chan (Ptr for num chan param)             */
/*                  pWORD32 pi_pcm_wd_sz (Ptr for PCM WORD size param)       */
/*                                                                           */
/*  Globals       : none                                                     */
/*                                                                           */
/*  Processing    : Get config params from API                               */
/*                                                                           */
/*  Outputs       : none                                                     */
/*                                                                           */
/*  Returns       : IA_ERRORCODE error_value  (Error value)                  */
/*                                                                           */
/*  Issues        : none                                                     */
/*                                                                           */
/*  Revision history :                                                       */
/*                                                                           */
/*        DD MM YYYY       Author                Changes                     */
/*        29 07 2005       Ittiam                Created                     */
/*                                                                           */
/*****************************************************************************/

IA_ERRORCODE ixheaacd_get_config_param(pVOID p_ia_process_api_obj,
                                       pWORD32 pi_samp_freq,
                                       pWORD32 pi_num_chan,
                                       pWORD32 pi_pcm_wd_sz,
                                       pWORD32 pi_channel_mask,
                                       pWORD32 pi_sbr_mode,
                                       pWORD32 pi_aot) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  /* the process API function */
  IA_ERRORCODE(*p_ia_process_api)
  (pVOID p_ia_process_api_obj, WORD32 i_cmd, WORD32 i_idx, pVOID pv_value) =
      ixheaacd_dec_api;
  ia_error_info_struct *p_proc_err_info = &ixheaacd_error_info;

  /* Sampling frequency */
  {
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_SAMP_FREQ, pi_samp_freq);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
  }
  /* Total Number of Channels */
  {
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_NUM_CHANNELS, pi_num_chan);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
  }
  /* PCM word size */
  {
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_PCM_WDSZ, pi_pcm_wd_sz);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
  }
  /* channel mask to tell the arrangement of channels in bit stream */
  {
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_CHANNEL_MASK, pi_channel_mask);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
  }

  /* Channel mode to tell MONO/STEREO/DUAL-MONO/NONE_OF_THESE */
  {
    UWORD32 ui_channel_mode;
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_CHANNEL_MODE, &ui_channel_mode);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    if (ui_channel_mode == 0)
      printf("Channel Mode: MONO_OR_PS\n");
    else if (ui_channel_mode == 1)
      printf("Channel Mode: STEREO\n");
    else if (ui_channel_mode == 2)
      printf("Channel Mode: DUAL-MONO\n");
    else
      printf("Channel Mode: NONE_OF_THESE or MULTICHANNEL\n");
  }

  /* Channel mode to tell SBR PRESENT/NOT_PRESENT */
  {
    UWORD32 ui_sbr_mode;
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_SBR_MODE, &ui_sbr_mode);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    if (ui_sbr_mode == 0)
      printf("SBR Mode: NOT_PRESENT\n");
    else if (ui_sbr_mode == 1)
      printf("SBR Mode: UPSAMPLING FACTOR 2 or 8/3\n");
    else if (ui_sbr_mode == 2)
      printf("SBR Mode: ILLEGAL\n");
    else if (ui_sbr_mode == 3)
      printf("ESBR Mode: UPSAMPLING FACTOR 4\n");
    else
      printf("ui_sbr_mode not vaild\n");
    *pi_sbr_mode = ui_sbr_mode;
  }
  {
    UWORD32 ui_aot;
    err_code = (*p_ia_process_api)(
        p_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_AOT, &ui_aot);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    *pi_aot = ui_aot;
  }
  return IA_NO_ERROR;
}

int counter_bl;
/*****************************************************************************/
/*                                                                           */
/*  Function name : ixheaacd_main_process                           */
/*                                                                           */
/*  Description   : Stacked processing with function pointer selection       */
/*                                                                           */
/*  Inputs        : WORD32 argc (Arguments count)                            */
/*                  pWORD8 argv[] (Argument strings)                         */
/*                                                                           */
/*  Globals       : pVOID g_pv_arr_alloc_memory[MAX_MEM_ALLOCS];             */
/*                  WORD  g_w_malloc_count;                                  */
/*                  FILE *g_pf_inp, *g_pf_out;                               */
/*                                                                           */
/*  Processing    : Stacked processing of multiple components                */
/*                  Loop1: Set params + Mem alloc                            */
/*                  Loop2: Set params + Init process + Get params            */
/*                  Loop3: Execute                                           */
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

int ixheaacd_main_process(WORD32 argc, pWORD8 argv[]) {
  LOOPIDX i;
  WORD frame_counter = 0;
#ifdef DISPLAY_MESSAGE
  /* Library Info and Identification strings */
  WORD8 pb_process_name[IA_SCREEN_WIDTH] = "";
  WORD8 pb_lib_version[IA_SCREEN_WIDTH] = "";
#endif

  /* Error code */
  IA_ERRORCODE err_code = IA_NO_ERROR;
  IA_ERRORCODE err_code_reinit = IA_NO_ERROR;

  /* API obj */
  pVOID pv_ia_process_api_obj;

  pVOID pv_ia_drc_process_api_obj;
  UWORD32 pui_api_size;

  /* First part                                        */
  /* Error Handler Init                                */
  /* Get Library Name, Library Version and API Version */
  /* Initialize API structure + Default config set     */
  /* Set config params from user                       */
  /* Initialize memory tables                          */
  /* Get memory information and allocate memory        */

  UWORD8 drc_ip_buf[4096 * 4 * 8];
  UWORD8 drc_op_buf[4096 * 4 * 8];

  /* Memory variables */
  UWORD32 n_mems, ui_rem;
  UWORD32 ui_proc_mem_tabs_size;
  /* API size */
  UWORD32 pui_ap_isize;
  /* Process initing done query variable */
  UWORD32 ui_init_done, ui_exec_done;
  pWORD8 pb_inp_buf = 0, pb_out_buf = 0;

  /* Number of Prerolls variable */
  WORD32 num_preroll = 0;

  // pWORD16 litt2big;

  UWORD32 ui_inp_size = 0;
  WORD32 i_bytes_consumed, i_bytes_read;
  WORD32 i_buff_size;
  WORD32 prev_sampling_rate = 0;
  WORD32 skip_samples = 0;
  WORD32 total_samples = 0;
  WORD32 write_flag = 1;
  WORD32 bytes_to_write = 0;
  WORD32 ixheaacd_drc_offset = 0;
  WORD32 current_samples = 0;
  WORD32 samples_written = 0;
  WORD32 init_iteration = 1;

  WORD32 fatal_error_chk;

#ifdef ARM_PROFILE_HW
  int frame_count_b = 0;
  long long cycles_b = 0;
  long long start1_b, stop1_b;
  double Curr_b, Ave_b = 0, Sum_b = 0;
  double Peak_b = 0;
  WORD32 Peak_frame_b = 0;
#endif
  WORD32 i_out_bytes, i_total_bytes = 0;
  WORD32 i_samp_freq, i_num_chan, i_pcm_wd_sz, i_channel_mask;

  WORD32 i_sbr_mode;
  WORD32 i_effect_type = 0;
  WORD32 i_target_loudness = 0;
  WORD32 i_loud_norm = 0;
  WORD32 drc_flag = 0;
  WORD32 mpegd_drc_present = 0;
  WORD32 uo_num_chan;
  FLOAT32 f_boost = 0;
  FLOAT32 f_cut = 0;
  UWORD8 ui_drc_mode_cut = 0;
  UWORD8 ui_drc_mode_boost = 0;
#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD32 i_loudness_leveling_flag = 1;
#endif
  /* The process API function */
  IA_ERRORCODE(*p_ia_process_api)
  (pVOID p_ia_process_api_obj, WORD32 i_cmd, WORD32 i_idx, pVOID pv_value);

  /* The set config from argc argv */
  IA_ERRORCODE(*p_set_config_param)
  (WORD32 argc, pWORD8 argv[], pVOID p_ia_process_api_obj);

  /* The get config from API */
  IA_ERRORCODE(*p_get_config_param)
  (pVOID p_ia_process_api_obj, pWORD32 pi_samp_freq, pWORD32 pi_num_chan,
   pWORD32 pi_pcm_wd_sz, pWORD32 pi_channel_mask, pWORD32 pi_sbr_mode, pWORD32 pi_aot);
  WORD32 ui_aot = 0;

  /* The error init function */
  VOID (*p_error_init)();

  /* The process error info structure */
  ia_error_info_struct *p_proc_err_info;

  /* Process struct initing */
  p_ia_process_api = ixheaacd_dec_api;
  p_set_config_param = ixheaacd_set_config_param;
  p_get_config_param = ixheaacd_get_config_param;
  p_error_init = ixheaacd_error_handler_init;
  p_proc_err_info = &ixheaacd_error_info;
  /* Process struct initing end */

  /* ******************************************************************/
  /* Initialize the error handler                                     */
  /* ******************************************************************/
  (*p_error_init)();

/* ******************************************************************/
/* Get the library name, library version and API version            */
/* ******************************************************************/

#ifdef DISPLAY_MESSAGE
  /* Get the library name string */
  err_code = (*p_ia_process_api)(NULL, IA_API_CMD_GET_LIB_ID_STRINGS,
                                 IA_CMD_TYPE_LIB_NAME, pb_process_name);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /* Get the library version string */
  err_code = (*p_ia_process_api)(NULL, IA_API_CMD_GET_LIB_ID_STRINGS,
                                 IA_CMD_TYPE_LIB_VERSION, pb_lib_version);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /* Display the ittiam identification message */
  ia_display_id_message(pb_process_name, pb_lib_version);
#endif

  /* ******************************************************************/
  /* Initialize API structure and set config params to default        */
  /* ******************************************************************/

  /* Get the API size */
  err_code =
      (*p_ia_process_api)(NULL, IA_API_CMD_GET_API_SIZE, 0, &pui_ap_isize);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /* Allocate memory for API */
  g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(pui_ap_isize + 4);

  if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
    err_code = IA_TESTBENCH_MFMAN_FATAL_MEM_ALLOC_FAILED;
    _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                     (pWORD8) "API struct alloc", err_code);
  }

  /* API object requires 4 bytes (WORD32) alignment */
  ui_rem = ((SIZE_T)g_pv_arr_alloc_memory[g_w_malloc_count] & 3);
  /* Set API object with the memory allocated */
  pv_ia_process_api_obj =
      (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count] + 4 - ui_rem);

  g_w_malloc_count++;

  /* Set the config params to default values */
  err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_INIT,
                                 IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS, NULL);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /*ITTIAM: Get API size for DRC*/

  /* Get the API size */
  err_code = ia_drc_dec_api(NULL, IA_API_CMD_GET_API_SIZE, 0, &pui_api_size);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /* Allocate memory for API */
  g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(pui_api_size + 4);
  if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
    _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                     (pWORD8) "API struct alloc", err_code);
  }

  /* API object requires 4 bytes (WORD32) alignment */
  ui_rem = ((SIZE_T)g_pv_arr_alloc_memory[g_w_malloc_count] & 3);
  /* Set API object with the memory allocated */
  pv_ia_drc_process_api_obj =
      (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count] + 4 - ui_rem);

  g_w_malloc_count++;

  err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                            IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS, NULL);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /* ******************************************************************/
  /* Set config parameters got from the user present in argc argv     */
  /* ******************************************************************/

  err_code = (*p_set_config_param)(argc, argv, pv_ia_process_api_obj);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

/* ******************************************************************/
/* Table Relocatibility                                             */
/* ******************************************************************/
#if IA_HE_AAC_DEC_TABLE_RELOCATABLE_ENABLE

  /* Get number of tables required */
  err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_N_TABLES,
                                 0, &n_mems);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  for (i = 0; i < (WORD32)n_mems; i++) {
    int ui_size, ui_alignment;
    pVOID pv_alloc_ptr = NULL, pv_curr_ptr = NULL;
    LOOPIDX k;

    /* Get table size */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_TABLE_INFO_SIZE, i, &ui_size);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Get table alignment */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_TABLE_INFO_ALIGNMENT, i,
                                   &ui_alignment);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(ui_size + ui_alignment);

    if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
      _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                       (pWORD8) "Mem for table relocation alloc",
                       IA_TESTBENCH_MFMAN_FATAL_MEM_ALLOC_FAILED);
    }

    ui_rem = ((SIZE_T)g_pv_arr_alloc_memory[g_w_malloc_count] % ui_alignment);
    pv_alloc_ptr = (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count] +
                           ui_alignment - ui_rem);

    g_w_malloc_count++;

    /* Get the current table pointer */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_TABLE_PTR, i, &pv_curr_ptr);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    for (k = 0; k < ui_size; k++) {
      ((pWORD8)pv_alloc_ptr)[k] = ((pWORD8)pv_curr_ptr)[k];
      /* Disabled for multiple files running
((pWORD8)pv_curr_ptr)[k] = (WORD8)0xab; */
    }

    /* Set the relocated table pointer */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_SET_TABLE_PTR, i, pv_alloc_ptr);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
  }
#endif
  /* ******************************************************************/
  /* Initialize Memory info tables                                    */
  /* ******************************************************************/

  /* Get memory info tables size */
  err_code =
      (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_MEMTABS_SIZE, 0,
                          &ui_proc_mem_tabs_size);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(ui_proc_mem_tabs_size + 4);

  if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
    err_code = IA_TESTBENCH_MFMAN_FATAL_MEM_ALLOC_FAILED;
    _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                     (pWORD8) "Mem tables alloc", err_code);
  }

  /* API object requires 4 bytes (WORD32) alignment */
  ui_rem = ((SIZE_T)g_pv_arr_alloc_memory[g_w_malloc_count] & 3);

  /* Set pointer for process memory tables    */
  err_code = (*p_ia_process_api)(
      pv_ia_process_api_obj, IA_API_CMD_SET_MEMTABS_PTR, 0,
      (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count] + 4 - ui_rem));

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  g_w_malloc_count++;

  /* initialize the API, post config, fill memory tables    */
  err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_INIT,
                                 IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS, NULL);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  /* ******************************************************************/
  /* Allocate Memory with info from library                           */
  /* ******************************************************************/

  /* Get number of memory tables required */
  err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                 IA_API_CMD_GET_N_MEMTABS, 0, &n_mems);

  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  for (i = 0; i < (WORD32)n_mems; i++) {
    int ui_size, ui_alignment, ui_type;
    pVOID pv_alloc_ptr;

    /* Get memory size */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_MEM_INFO_SIZE, i, &ui_size);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Get memory alignment */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_MEM_INFO_ALIGNMENT, i,
                                   &ui_alignment);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Get memory type */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_MEM_INFO_TYPE, i, &ui_type);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    err_code =
        (*p_get_config_param)(pv_ia_process_api_obj, &i_samp_freq, &i_num_chan,
                              &i_pcm_wd_sz, &i_channel_mask, &i_sbr_mode, &ui_aot);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(ui_size + ui_alignment);

    if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
      err_code = IA_TESTBENCH_MFMAN_FATAL_MEM_ALLOC_FAILED;
      _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                       (pWORD8) "Mem tables alloc", err_code);
    }

    ui_rem = ((SIZE_T)g_pv_arr_alloc_memory[g_w_malloc_count] % ui_alignment);
    pv_alloc_ptr = (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count] +
                           ui_alignment - ui_rem);

    g_w_malloc_count++;

    /* Set the buffer pointer */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_SET_MEM_PTR, i, pv_alloc_ptr);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    if (ui_type == IA_MEMTYPE_INPUT) {
      pb_inp_buf = pv_alloc_ptr;
      ui_inp_size = ui_size;
    }
    if (ui_type == IA_MEMTYPE_OUTPUT) {
      pb_out_buf = pv_alloc_ptr;
    }
  }

  /* End first part */

  /* Second part        */
  /* Initialize process */
  /* Get config params  */

  /* ******************************************************************/
  /* Initialize process in a loop (to handle junk data at beginning)  */
  /* ******************************************************************/

  i_bytes_consumed = ui_inp_size;
  i_buff_size = ui_inp_size;

  do {
    i_bytes_read = 0;

    if ((ui_inp_size - (i_buff_size - i_bytes_consumed)) > 0) {
      for (i = 0; i < (i_buff_size - i_bytes_consumed); i++) {
        pb_inp_buf[i] = pb_inp_buf[i + i_bytes_consumed];
      }

      FileWrapper_Read(g_pf_inp, (unsigned char *)(pb_inp_buf + i_buff_size -
                                                   i_bytes_consumed),
                       (ui_inp_size - (i_buff_size - i_bytes_consumed)),
                       (pUWORD32)&i_bytes_read);

      i_buff_size = i_buff_size - (i_bytes_consumed - i_bytes_read);

      /* Tell input is over, if algorithm returns with insufficient input and
         there is no
             more input left in the bitstream*/
      if ((i_buff_size <= 0) ||
          ((err_code_reinit == 0x00001804) && i_bytes_read == 0))

      {
        i_buff_size = 0;
        /* Tell that the input is over in this buffer */
        err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                       IA_API_CMD_INPUT_OVER, 0, NULL);

        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      }
    }

    if ((i_buff_size <= 0) ||
        ((err_code_reinit == 0x00001804) && i_bytes_read == 0)) {
      i_buff_size = 0;
      /* Tell that the input is over in this buffer */
      err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                     IA_API_CMD_INPUT_OVER, 0, NULL);

      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
#ifdef WAV_HEADER
#ifndef ARM_PROFILE_BOARD
      /* ******************************************************************/
      /* Get config params from API                                       */
      /* ******************************************************************/

      err_code =
          (*p_get_config_param)(pv_ia_process_api_obj, &i_samp_freq,
                                &i_num_chan, &i_pcm_wd_sz, &i_channel_mask,
                                &i_sbr_mode, &ui_aot);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      // This is done in those cases, where file decodes ends at init time
      // Since init is incomplete, sampling freq might be zero and will result
      // in
      // writing invalid wave header

      if (i_samp_freq == 0) i_samp_freq = prev_sampling_rate;

      if (!fseek(g_pf_out, 0, SEEK_SET))
        write_wav_header(g_pf_out, i_total_bytes, i_samp_freq, i_num_chan,
                         i_pcm_wd_sz, i_channel_mask);
#endif
#endif
      return 1;
    }

    if (init_iteration == 1) {
      if (raw_testing)
        ixheaacd_i_bytes_to_read = get_metadata_dec_info_init(meta_info);
      else
        ixheaacd_i_bytes_to_read = i_buff_size;

      /* Set number of bytes to be processed */
      err_code =
          (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES,
                              0, &ixheaacd_i_bytes_to_read);
      init_iteration++;

    } else {
      if (raw_testing) {
        ixheaacd_i_bytes_to_read =
            get_metadata_dec_exec(meta_info, frame_counter);

        if (ixheaacd_i_bytes_to_read > (WORD32)ui_inp_size)
          return IA_FATAL_ERROR;

        if (ixheaacd_i_bytes_to_read <= 0) {
          err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                         IA_API_CMD_INPUT_OVER, 0, NULL);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          return IA_NO_ERROR;
        }

        /* Set number of bytes to be processed */
        err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                       IA_API_CMD_SET_INPUT_BYTES, 0,
                                       &ixheaacd_i_bytes_to_read);
        init_iteration++;
      } else {
        /* Set number of bytes to be processed */
        err_code = (*p_ia_process_api)(
            pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES, 0, &i_buff_size);
      }
    }

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Initialize the process */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_INIT,
                                   IA_CMD_TYPE_INIT_PROCESS, NULL);
    err_code_reinit = err_code;

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Checking for end of initialization */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_INIT,
                                   IA_CMD_TYPE_INIT_DONE_QUERY, &ui_init_done);

    if (init_iteration > 2 && ui_init_done == 0) {
      frame_counter++;
    }

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* How much buffer is used in input buffers */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                   IA_API_CMD_GET_CURIDX_INPUT_BUF, 0,
                                   &i_bytes_consumed);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  } while (!ui_init_done);
  if (ec_enable == 1) {
    mpeg_d_drc_on = 0;
  }
  if (mpeg_d_drc_on == 1) {
    err_code = (*p_ia_process_api)(
        pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_XHEAAC_DEC_CONFIG_PARAM_SBR_MODE, &i_sbr_mode);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    if (i_sbr_mode != 0) {
      WORD32 frame_length;
      if (i_sbr_mode == 1) {
        frame_length = 2048;
      } else if (i_sbr_mode == 3) {
        frame_length = 4096;
      } else {
        frame_length = 1024;
      }

      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE, &frame_length);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    err_code =
        (*p_get_config_param)(pv_ia_process_api_obj, &i_samp_freq, &i_num_chan,
                              &i_pcm_wd_sz, &i_channel_mask, &i_sbr_mode, &ui_aot);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Sampling Frequency */
    {
      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ, &i_samp_freq);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    /* Total Number of Channels */
    {
      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS, &i_num_chan);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /* PCM word size  */
    {
      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ, &i_pcm_wd_sz);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /*Set Effect Type*/

    {
      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE, &i_effect_type);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_DRC_EFFECT_TYPE, &i_effect_type);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /*Set target loudness */

    {
      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS,
          &i_target_loudness);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code = ia_drc_dec_api(
          pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
          IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS, &i_target_loudness);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

#ifdef LOUDNESS_LEVELING_SUPPORT
    /*Set loudness leveling */

    {
      err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                                     IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING,
                                     &i_loudness_leveling_flag);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_DRC_LOUDNESS_LEVELING, &i_loudness_leveling_flag);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
#endif

    /*Set cut */

    {
      err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                                     IA_XHEAAC_DEC_CONFIG_PARAM_DRC_MODE_CUT, &ui_drc_mode_cut);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      if (ui_drc_mode_cut) {
        err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                                       IA_XHEAAC_DEC_CONFIG_PARAM_DRC_CUT, &f_cut);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                                  IA_DRC_DEC_CONFIG_DRC_CUT, &f_cut);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      }
    }

    /*Set boost */

    {
      err_code =
          (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                              IA_XHEAAC_DEC_CONFIG_PARAM_DRC_MODE_BOOST, &ui_drc_mode_boost);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      if (ui_drc_mode_boost) {
        err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                                       IA_XHEAAC_DEC_CONFIG_PARAM_DRC_BOOST, &f_boost);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                                  IA_DRC_DEC_CONFIG_DRC_BOOST, &f_boost);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      }
    }

    /*Set loud_norm_flag*/
    {
      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUD_NORM, &i_loud_norm);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                         IA_DRC_DEC_CONFIG_DRC_LOUD_NORM, &i_loud_norm);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }

    /* Get memory info tables size */
    err_code =
        ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_GET_MEMTABS_SIZE,
                       0, &ui_proc_mem_tabs_size);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(ui_proc_mem_tabs_size);

    if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
      err_code = IA_TESTBENCH_MFMAN_FATAL_MEM_ALLOC_FAILED;
      _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                       (pWORD8) "Mem tables alloc", err_code);
    }

    /* Set pointer for process memory tables    */
    err_code = ia_drc_dec_api(
        pv_ia_drc_process_api_obj, IA_API_CMD_SET_MEMTABS_PTR, 0,
        (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count]));

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    g_w_malloc_count++;

    err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                              IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS, NULL);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /* Get number of memory tables required */
    err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                              IA_API_CMD_GET_N_MEMTABS, 0, &n_mems);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    for (i = 0; i < (WORD32)n_mems - 2; i++) {
      WORD32 ui_size, ui_alignment, ui_type;
      pVOID pv_alloc_ptr;

      /* Get memory size */
      err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                IA_API_CMD_GET_MEM_INFO_SIZE, i, &ui_size);

      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      /* Get memory alignment */
      err_code =
          ia_drc_dec_api(pv_ia_drc_process_api_obj,
                         IA_API_CMD_GET_MEM_INFO_ALIGNMENT, i, &ui_alignment);

      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      /* Get memory type */
      err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                IA_API_CMD_GET_MEM_INFO_TYPE, i, &ui_type);

      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      g_pv_arr_alloc_memory[g_w_malloc_count] = malloc(ui_size + ui_alignment);

      if (g_pv_arr_alloc_memory[g_w_malloc_count] == NULL) {
        _IA_HANDLE_ERROR(&ixheaacd_ia_testbench_error_info,
                         (pWORD8) "Mem tables alloc", err_code);
      }

      ui_rem = ((SIZE_T)g_pv_arr_alloc_memory[g_w_malloc_count] % ui_alignment);
      pv_alloc_ptr = (pVOID)((WORD8 *)g_pv_arr_alloc_memory[g_w_malloc_count] +
                             ui_alignment - ui_rem);

      g_w_malloc_count++;

      /* Set the buffer pointer */
      err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                IA_API_CMD_SET_MEM_PTR, i, pv_alloc_ptr);

      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    }
    err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_MEM_PTR,
                              2, drc_ip_buf);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_SET_MEM_PTR,
                              3, drc_op_buf);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    /*ITTIAM: DRC buffers
            buf[0] - contains extension element pay load loudness related
            buf[1] - contains extension element pay load*/
    {
      VOID *p_array[2][16];
      WORD32 ii;
      WORD32 buf_sizes[2][16];
      WORD32 num_elements;
      WORD32 num_config_ext;
      WORD32 bit_str_fmt = 1;

      memset(buf_sizes, 0, 32 * sizeof(WORD32));

      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_BUF_SIZES, &buf_sizes[0][0]);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_PTR, &p_array);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                IA_CMD_TYPE_INIT_SET_BUFF_PTR, 0);

      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_NUM_ELE, &num_elements);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
          IA_ENHAACPLUS_DEC_CONFIG_NUM_CONFIG_EXT, &num_config_ext);
      _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

      for (ii = 0; ii < num_config_ext; ii++) {
        /*copy loudness bitstream*/
        if (buf_sizes[0][ii] > 0) {
          memcpy(drc_ip_buf, p_array[0][ii], buf_sizes[0][ii]);

          /*Set bitstream_split_format */
          err_code = ia_drc_dec_api(
              pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
              IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, &bit_str_fmt);

          /* Set number of bytes to be processed */
          err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                    IA_API_CMD_SET_INPUT_BYTES_IL_BS, 0,
                                    &buf_sizes[0][ii]);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          /* Execute process */
          err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                    IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF, NULL);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          drc_flag = 1;
        }
      }

      for (ii = 0; ii < num_elements; ii++) {
        /*copy config bitstream*/
        if (buf_sizes[1][ii] > 0) {
          memcpy(drc_ip_buf, p_array[1][ii], buf_sizes[1][ii]);
          /* Set number of bytes to be processed */

          /*Set bitstream_split_format */
          err_code = ia_drc_dec_api(
              pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
              IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, &bit_str_fmt);

          err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                    IA_API_CMD_SET_INPUT_BYTES_IC_BS, 0,
                                    &buf_sizes[1][ii]);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          /* Execute process */
          err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                    IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF, NULL);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          drc_flag = 1;
        }
      }

      if (drc_flag == 1) {
        mpegd_drc_present = 1;
      } else {
        mpegd_drc_present = 0;
      }

      /*Read interface buffer config file bitstream*/

      if (mpegd_drc_present == 1) {
        WORD32 interface_is_present = 1;

        err_code = ia_drc_dec_api(
            pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
            IA_DRC_DEC_CONFIG_PARAM_INT_PRESENT, &interface_is_present);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        /* Execute process */
        err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                  IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF, NULL);

        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                  IA_CMD_TYPE_INIT_PROCESS, NULL);

        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        err_code = ia_drc_dec_api(
            pv_ia_drc_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
            IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS, &uo_num_chan);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      }
    }
  }

  /* ******************************************************************/
  /* Get config params from API                                       */
  /* ******************************************************************/

  err_code = (*p_get_config_param)(pv_ia_process_api_obj, &i_samp_freq,
                                   &i_num_chan, &i_pcm_wd_sz, &i_channel_mask,
                                   &i_sbr_mode, &ui_aot);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

  if (ui_aot == 42)
    esbr_testing = 1;

  if (raw_testing) {
    skip_samples = get_start_offset_in_samples(meta_info);
    if (ui_aot >= 23 && esbr_testing) {
      skip_samples = skip_samples - 2048;
      if (skip_samples < 0) {
        skip_samples = 0;
      }
    }
    if (eld_testing == 0) total_samples = get_play_time_in_samples(meta_info);
  }

/* End second part */

#ifdef WAV_HEADER
// This condition is added so as to avoid re-writing wave header in
// middle of wave file in case of errors and when we are not opening
// new file in case of errors.

#ifndef ARM_PROFILE_BOARD

  write_wav_header(g_pf_out, 0, i_samp_freq, i_num_chan, i_pcm_wd_sz,
                   i_channel_mask);
#endif
#endif
  prev_sampling_rate = i_samp_freq;

  do {
    if (((WORD32)ui_inp_size - (WORD32)(i_buff_size - i_bytes_consumed)) > 0) {
      if (i_sbr_mode && (ui_aot < 23) && esbr_testing) {
        if (meta_info.ia_mp4_stsz_entries != frame_counter) {
          for (i = 0; i < (i_buff_size - i_bytes_consumed); i++) {
            pb_inp_buf[i] = pb_inp_buf[i + i_bytes_consumed];
          }

          FileWrapper_Read(
              g_pf_inp,
              (unsigned char *)(pb_inp_buf + i_buff_size - i_bytes_consumed),
              ((WORD32)ui_inp_size - (WORD32)(i_buff_size - i_bytes_consumed)),
              (pUWORD32)&i_bytes_read);

          i_buff_size = i_buff_size - (i_bytes_consumed - i_bytes_read);

          if ((i_buff_size <= 0) ||
              ((err_code_reinit == 0x00001804) && i_bytes_read == 0)) {
            i_buff_size = 0;
            raw_testing = 0;
            /* Tell that the input is over in this buffer */
            err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                           IA_API_CMD_INPUT_OVER, 0, NULL);

            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
          }
        }
      } else {
        for (i = 0; i < (i_buff_size - i_bytes_consumed); i++) {
          pb_inp_buf[i] = pb_inp_buf[i + i_bytes_consumed];
        }

        FileWrapper_Read(g_pf_inp, (unsigned char *)(pb_inp_buf + i_buff_size -
            i_bytes_consumed),
            ((WORD32)ui_inp_size - (WORD32)(i_buff_size - i_bytes_consumed)),
            (pUWORD32)&i_bytes_read);

        i_buff_size = i_buff_size - (i_bytes_consumed - i_bytes_read);

        if ((i_buff_size <= 0) ||
            ((err_code_reinit == 0x00001804) && i_bytes_read == 0)) {
          i_buff_size = 0;
          raw_testing = 0;

          err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
              IA_API_CMD_INPUT_OVER, 0, NULL);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
        }
      }
    }
    if (i_sbr_mode && (ui_aot < 23) && esbr_testing) {
      if (meta_info.ia_mp4_stsz_entries != frame_counter) {
        if (raw_testing) {
          ixheaacd_i_bytes_to_read =
              get_metadata_dec_exec(meta_info, frame_counter);

          if (ixheaacd_i_bytes_to_read > (WORD32)ui_inp_size)
            return IA_FATAL_ERROR;
          if (ixheaacd_i_bytes_to_read <= 0 && ec_enable == 0) {
            err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                           IA_API_CMD_INPUT_OVER, 0, NULL);

            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            return IA_NO_ERROR;
          }
          err_code =
              (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES,
                  0, &ixheaacd_i_bytes_to_read);
        } else {
          err_code = (*p_ia_process_api)(
              pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES, 0, &i_buff_size);
        }
      }
    } else {
    if (raw_testing) {
      ixheaacd_i_bytes_to_read =
          get_metadata_dec_exec(meta_info, frame_counter);

      if (ixheaacd_i_bytes_to_read > (WORD32)ui_inp_size) return IA_FATAL_ERROR;

      if (ixheaacd_i_bytes_to_read <= 0 && ec_enable == 0) {
        err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                       IA_API_CMD_INPUT_OVER, 0, NULL);

        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        return IA_NO_ERROR;
      }

        if (ec_enable == 1) {
          if (ixheaacd_i_bytes_to_read != 0) {
            err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES, 0,
                                           &ixheaacd_i_bytes_to_read);
          } else {
            if (i_buff_size != 0) {
              err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES, 0,
                                             &i_buff_size);
            }
          }
        } else {
          err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES, 0,
                                         &ixheaacd_i_bytes_to_read);
        }
    } else {
      /* Set number of bytes to be processed */
      err_code = (*p_ia_process_api)(
          pv_ia_process_api_obj, IA_API_CMD_SET_INPUT_BYTES, 0, &i_buff_size);

      if (i_buff_size <= 0)
      {
        err_code = (*p_ia_process_api)(
            pv_ia_process_api_obj, IA_API_CMD_INPUT_OVER, 0, NULL);

        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
      }
    }
    }

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

#ifdef ARM_PROFILE_HW
    start1_b = itGetMs();
#endif

    /* Execute process */

    counter_bl = frame_counter;

    err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_EXECUTE,
                                   IA_CMD_TYPE_DO_EXECUTE, NULL);

    err_code_reinit = err_code;

#ifdef ARM_PROFILE_HW
    stop1_b = itGetMs();
    cycles_b = (stop1_b - start1_b);
#endif

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    fatal_error_chk = err_code & IA_FATAL_ERROR;

    /* Checking for end of processing */
    err_code = (*p_ia_process_api)(pv_ia_process_api_obj, IA_API_CMD_EXECUTE,
                                   IA_CMD_TYPE_DONE_QUERY, &ui_exec_done);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

    err_code = (*p_ia_process_api)(
        pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
        IA_ENHAACPLUS_DEC_CONFIG_GET_NUM_PRE_ROLL_FRAMES, &num_preroll);
    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
    {
      WORD32 preroll_frame_offset = 0;
      do {
        if (mpeg_d_drc_on == 1) {
          if (ui_exec_done != 1) {
            VOID *p_array;        // ITTIAM:buffer to handle gain payload
            WORD32 buf_size = 0;  // ITTIAM:gain payload length
            WORD32 bit_str_fmt = 1;
            WORD32 gain_stream_flag = 1;

            err_code = (*p_ia_process_api)(
                pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_LEN, &buf_size);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            err_code = (*p_ia_process_api)(
                pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_BUF, &p_array);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            if (buf_size > 0) {
              /*Set bitstream_split_format */
              err_code = ia_drc_dec_api(
                  pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                  IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, &bit_str_fmt);

              memcpy(drc_ip_buf, p_array, buf_size);
              /* Set number of bytes to be processed */
              err_code =
                  ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                 IA_API_CMD_SET_INPUT_BYTES_BS, 0, &buf_size);

              err_code = ia_drc_dec_api(
                  pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                  IA_DRC_DEC_CONFIG_GAIN_STREAM_FLAG, &gain_stream_flag);

              _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

              /* Execute process */
              err_code =
                  ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                 IA_CMD_TYPE_INIT_CPY_BSF_BUFF, NULL);

              _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

              mpegd_drc_present = 1;
            }
          }
        }
        /* How much buffer is used in input buffers */
        err_code = (*p_ia_process_api)(pv_ia_process_api_obj,
                                       IA_API_CMD_GET_CURIDX_INPUT_BUF, 0,
                                       &i_bytes_consumed);

        //    printf("bytes_consumed:  %d  \n", i_bytes_consumed);
        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        /* Get the output bytes */
        err_code =
            (*p_ia_process_api)(pv_ia_process_api_obj,
                                IA_API_CMD_GET_OUTPUT_BYTES, 0, &i_out_bytes);

        _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

        if (err_code_reinit != 0) memset(pb_out_buf, 0, i_out_bytes);

        if (i_sbr_mode && (ui_aot < 23) && esbr_testing) {
           if (frame_counter > 0)
             i_total_bytes += i_out_bytes;
        } else {
          i_total_bytes += i_out_bytes;
        }
        if (mpegd_drc_present == 1) {
          WORD32 is_config_changed = 0, apply_crossfade = 0;

          err_code = (*p_ia_process_api)(
              pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
              IA_XHEAAC_DEC_CONFIG_PARAM_SBR_MODE, &i_sbr_mode);
          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          if (i_sbr_mode != 0) {
            WORD32 frame_length;
            if (i_sbr_mode == 1) {
              frame_length = 2048;
            } else if (i_sbr_mode == 3) {
              frame_length = 4096;
            } else {
              frame_length = 1024;
            }

            err_code = ia_drc_dec_api(
                pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE, &frame_length);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);
          }

          err_code =
              ia_drc_dec_api(pv_ia_drc_process_api_obj,
                             IA_API_CMD_SET_INPUT_BYTES, 0, &i_out_bytes);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          err_code = (*p_ia_process_api)(
              pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
              IA_ENHAACPLUS_DEC_DRC_IS_CONFIG_CHANGED, &is_config_changed);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          err_code = ia_drc_dec_api(
              pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
              IA_DRC_DEC_CONFIG_PARAM_CONFIG_CHANGED, &is_config_changed);
          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          err_code = (*p_ia_process_api)(
              pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
              IA_ENHAACPLUS_DEC_DRC_APPLY_CROSSFADE, &apply_crossfade);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          err_code = ia_drc_dec_api(
              pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
              IA_DRC_DEC_CONFIG_PARAM_APPLY_CROSSFADE, &apply_crossfade);
          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          if (is_config_changed == 1) {
            VOID *p_array[2][16];
            WORD32 ii;
            WORD32 num_elements;
            WORD32 num_config_ext;
            WORD32 buf_sizes[2][16];
            WORD32 bit_str_fmt = 1;

            memset(buf_sizes, 0, 32 * sizeof(WORD32));

            err_code = (*p_ia_process_api)(
                pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_BUF_SIZES, &buf_sizes[0][0]);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            err_code = (*p_ia_process_api)(
                pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_PTR, &p_array);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            err_code =
                ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                               IA_CMD_TYPE_INIT_SET_BUFF_PTR, 0);

            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            err_code = (*p_ia_process_api)(
                pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                IA_ENHAACPLUS_DEC_CONFIG_NUM_ELE, &num_elements);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            err_code = (*p_ia_process_api)(
                pv_ia_process_api_obj, IA_API_CMD_GET_CONFIG_PARAM,
                IA_ENHAACPLUS_DEC_CONFIG_NUM_CONFIG_EXT, &num_config_ext);
            _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

            for (ii = 0; ii < num_config_ext; ii++) {
              /*copy loudness bitstream*/
              if (buf_sizes[0][ii] > 0) {
                memcpy(drc_ip_buf, p_array[0][ii], buf_sizes[0][ii]);

                /*Set bitstream_split_format */
                err_code = ia_drc_dec_api(
                    pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                    IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, &bit_str_fmt);

                /* Set number of bytes to be processed */
                err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                          IA_API_CMD_SET_INPUT_BYTES_IL_BS, 0,
                                          &buf_sizes[0][ii]);

                _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

                /* Execute process */
                err_code =
                    ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                   IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF, NULL);

                _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

                drc_flag = 1;
              }
            }

            for (ii = 0; ii < num_elements; ii++) {
              /*copy config bitstream*/
              if (buf_sizes[1][ii] > 0) {
                memcpy(drc_ip_buf, p_array[1][ii], buf_sizes[1][ii]);

                /*Set bitstream_split_format */
                err_code = ia_drc_dec_api(
                    pv_ia_drc_process_api_obj, IA_API_CMD_SET_CONFIG_PARAM,
                    IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, &bit_str_fmt);

                /* Set number of bytes to be processed */
                err_code = ia_drc_dec_api(pv_ia_drc_process_api_obj,
                                          IA_API_CMD_SET_INPUT_BYTES_IC_BS, 0,
                                          &buf_sizes[1][ii]);

                _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

                /* Execute process */
                err_code =
                    ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_INIT,
                                   IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF, NULL);

                _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

                drc_flag = 1;
              }
            }
          }

          memcpy(drc_ip_buf, pb_out_buf + preroll_frame_offset, i_out_bytes);
          preroll_frame_offset += i_out_bytes;

          err_code =
              ia_drc_dec_api(pv_ia_drc_process_api_obj, IA_API_CMD_EXECUTE,
                             IA_CMD_TYPE_DO_EXECUTE, NULL);

          _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

          memcpy(pb_out_buf, drc_op_buf, i_out_bytes);
        } else {
          memmove(pb_out_buf, pb_out_buf + preroll_frame_offset, i_out_bytes);
          preroll_frame_offset += i_out_bytes;
        }

        num_preroll--;
      } while (num_preroll > 0);
    }

    if (total_samples != 0)  // Usac stream
    {
      if (raw_testing) {
        if (i_total_bytes <= skip_samples * i_num_chan * (i_pcm_wd_sz >> 3)) {
          err_code =
              (*p_get_config_param)(pv_ia_process_api_obj, &i_samp_freq,
                                    &i_num_chan, &i_pcm_wd_sz, &i_channel_mask,
                                    &i_sbr_mode, &ui_aot);

          write_flag = 0;
        } else {
          write_flag = 1;
          bytes_to_write =
              i_total_bytes - skip_samples * i_num_chan * (i_pcm_wd_sz >> 3);
          if (bytes_to_write < i_out_bytes) {
            ixheaacd_drc_offset = i_out_bytes - bytes_to_write;
            i_out_bytes = bytes_to_write;
            current_samples =
                bytes_to_write / (i_num_chan * (i_pcm_wd_sz >> 3));
          } else {
            ixheaacd_drc_offset = 0;
            current_samples = i_out_bytes / (i_num_chan * (i_pcm_wd_sz >> 3));
          }
        }
      }

      if (raw_testing) {
        samples_written += current_samples;

        if (samples_written > total_samples) {
          i_out_bytes = (total_samples - (samples_written - current_samples)) *
                        (i_num_chan * (i_pcm_wd_sz >> 3));  // hack
          if (i_out_bytes < 0) i_out_bytes = 0;
        }
      }
    }

    if (write_flag) {
#ifndef WIN32
#ifndef ARM_PROFILE_BOARD
      ia_fwrite((pVOID)(pb_out_buf + ixheaacd_drc_offset), (i_pcm_wd_sz / 8),
                i_out_bytes / (i_pcm_wd_sz / 8), g_pf_out);
#endif
#else
#ifndef ARM_PROFILE_BOARD
      if (i_sbr_mode && (ui_aot < 23) && esbr_testing) {
        if (frame_counter != 0) {
      fwrite(pb_out_buf + ixheaacd_drc_offset, sizeof(WORD8), i_out_bytes,
             g_pf_out);
      fflush(g_pf_out);
        }
      } else {
      fwrite(pb_out_buf + ixheaacd_drc_offset, sizeof(WORD8), i_out_bytes,
             g_pf_out);
      fflush(g_pf_out);
      }
#endif
#endif
    }

    frame_counter++;

#ifdef ARM_PROFILE_HW
    if (i_out_bytes != 0) {
      int i_out_samples = i_out_bytes >> 2;
      if (frame_count_b) {
        double i_out_samples_per_ch =
            (i_out_bytes) / ((i_pcm_wd_sz / 8) * i_num_chan);
        Curr_b = (((double)cycles_b / 1000000) * CLK_FREQ_BOARD_MHZ) /
                 (i_out_samples_per_ch / i_samp_freq);
        frame_count_b++;
        // fprintf(stderr, "Microseconds: %d\t", cycles_b);
        // fprintf(stderr, "MCPS: %f\n", Curr_b);
        Sum_b += Curr_b;
        Ave_b = Sum_b / frame_count_b;
        if (Peak_b < Curr_b) {
          Peak_b = Curr_b;
          Peak_frame_b = frame_count_b;
        }
      } else {
        frame_count_b++;
      }

      cycles_b = 0;
    }
#endif

    /* Do till the process execution is done */
  } while (!ui_exec_done && !fatal_error_chk);

#ifdef ARM_PROFILE_HW
  fprintf(stdout, "\n Peak MCPS = %f\n", Peak_b);
  fprintf(stdout, " Avg MCPS = %f\n", Ave_b);
  fprintf(stdout, " Peak frame = %d\n", Peak_frame_b);
#endif
  fprintf(stderr, "TOTAL FRAMES : [%5d] \n", frame_counter);
  err_code = (*p_get_config_param)(pv_ia_process_api_obj, &i_samp_freq,
                                   &i_num_chan, &i_pcm_wd_sz, &i_channel_mask,
                                   &i_sbr_mode, &ui_aot);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code);

#ifdef WAV_HEADER
#ifndef ARM_PROFILE_BOARD
  if (!fseek(g_pf_out, 0, SEEK_SET))
    write_wav_header(g_pf_out, i_total_bytes, i_samp_freq, i_num_chan,
                     i_pcm_wd_sz, i_channel_mask);
#endif
#endif

  return IA_NO_ERROR;
}

/*****************************************************************************/
/*                                                                           */
/*  Function Name : ixheaacd_main */
/*                                                                           */
/*  Description   : Main function                                            */
/*                                                                           */
/*  Inputs        : None                                                     */
/*                                                                           */
/*  Globals       : None                                                     */
/*                                                                           */
/*  Processing    : Parse the parameter file and run the ixheaacd_main process
 */
/*                                                                           */
/*  Outputs       : None                                                     */
/*                                                                           */
/*  Returns       : 0 on success, -1 on error                                */
/*                                                                           */
/*  Issues        : None                                                     */
/*                                                                           */
/*  Revision history :                                                       */
/*                                                                           */
/*        DD MM YYYY       Author                Changes                     */
/*        04 09 2005       Ittiam                Created                     */
/*                                                                           */
/*****************************************************************************/

void print_usage() {
#ifdef DISPLAY_MESSAGE
  ia_lib_info_struct str_lib_info = {0};
  ixheaacd_get_lib_id_strings(&str_lib_info);

  ia_display_id_message(str_lib_info.p_lib_name, str_lib_info.p_version_num);
#endif
  printf("\n Usage \n");
  printf("\n <executable> -ifile:<input_file> -imeta:<meta_data_file> -ofile:<output_file> "
         "[options]\n");
  printf("\n[options] can be,");
  printf("\n[-mp4:<mp4_flag>]");
  printf("\n[-pcmsz:<pcmwordsize>]");
  printf("\n[-dmix:<down_mix>]");
  printf("\n[-esbr_hq:<esbr_hq_flag>]");
  printf("\n[-esbr_ps:<esbr_ps_flag>]");
  printf("\n[-tostereo:<interleave_to_stereo>]");
  printf("\n[-dsample:<down_sample_sbr>]");
  printf("\n[-drc_cut_fac:<drc_cut_factor>]");
  printf("\n[-drc_boost_fac:<drc_boost_factor>]");
  printf("\n[-drc_target_level:<drc_target_level>]");
  printf("\n[-drc_heavy_comp:<drc_heavy_compression>]");
  printf("\n[-effect:<effect_type>]");
  printf("\n[-target_loudness:<target_loudness>]");
  printf("\n[-nosync:<disable_sync>]");
  printf("\n[-sbrup:<auto_sbr_upsample>]");
  printf("\n[-flflag:<framelength_flag>}");
  printf("\n[-fs:<RAW_sample_rate>]");
  printf("\n[-maxchannel:<maximum_num_channels>]");
#ifdef MULTICHANNEL_ENABLE
  printf("\n[-coupchannel:<coupling_channel>]");
  printf("\n[-downmix:<down_mix_stereo>]");
#endif
  printf("\n[-fs480:<ld_frame_size>]");
  printf("\n[-ld_testing:<ld_testing_flag>]");
  printf("\n[-peak_limiter_off:<peak_limiter_off_flag>]");
  printf("\n[-err_conceal:<error_concealment_flag>]");
  printf("\n[-esbr:<esbr_flag>]");
#ifdef LOUDNESS_LEVELING_SUPPORT
  printf("\n[-loudness_leveling:<loudness_leveling_flag>]");
#endif
  printf("\n\nwhere, \n  <input_file> is the input AAC-LC/HE-AACv1/HE-AACv2//AAC-LD/AAC-ELD/AAC-ELDv2/USAC file name");
  printf("\n  <meta_data_file> is a text file which contains metadata.");
  printf("\n   To be given when -mp4:1 is enabled");
  printf("\n  <output_file> is the output file name");
  printf("\n  <mp4_flag> is a flag that should be set to 1 when passing ");
  printf("\n  raw stream along with meta data text file ");
  printf("\n  <pcmwordsize> is the bits per sample info. 16/24");
  printf("\n  <down_mix> is to enable/disable always mono output. Default 1");
  printf("\n  <esbr_hq_flag> is to enable/disable high quality eSBR. Default 0");
  printf("\n  <esbr_ps_flag> is to indicate eSBR with PS. Default 0");
  printf("\n  <interleave_to_stereo> is to enable/disable always ");
  printf("\n    interleaved to stereo output. Default 1 ");
  printf("\n  <down_sample_sbr> is to enable/disable down-sampled SBR ");
  printf("\n    output. Default auto identification from header");
  printf("\n  <drc_cut_factor> is to set DRC cut factor value. Default value is 1");
  printf("\n  <drc_boost_factor> is to set DRC boost factor. Default value is 1");
  printf("\n  <drc_target_level> is to set DRC target reference level.");
  printf("\n    Default value is 108");
  printf("\n  <drc_heavy_compression> is to enable / disable DRC heavy compression.");
  printf("\n    Default value is 0");
  printf("\n  <effect_type> is set DRC effect type. Default value is 0");
  printf("\n  <target_loudness> is to set target loudness level.");
  printf("\n    Default value is -24");
  printf("\n  <disable_sync> is to disable the ADTS/ADIF sync search i.e");
  printf("\n    when enabled the decoder expects the header to ");
  printf("\n    be at the start of input buffer. Default 0");
  printf("\n  <auto_sbr_upsample> is to enable(1) or disable(0) auto SBR "
         "upsample ");
  printf("\n    in case of stream changing from SBR present to SBR not present. "
         "Default 1");
  printf("\n  <framelength_flag> is flag for decoding framelength of 1024 or 960.");
  printf("\n    1 to decode 960 frame length, 0 to decode 1024 frame length");
  printf("\n    Frame length value in the GA header will override this option.");
  printf("\n    Default 0  ");
  printf("\n  <RAW_sample_rate> is to indicate the core AAC sample rate for");
  printf("\n    a RAW stream. If this is specified no other file format");
  printf("\n    headers are searched for.");
  printf("\n  <maximum_num_channels> is the number of maxiumum ");
  printf("\n    channels the input may have. Default is 6 ");
  printf("\n    for multichannel libraries and 2 for stereo libraries");
#ifdef MULTICHANNEL_ENABLE
  printf("\n  <coupling_channel> is element instance tag of ");
  printf("\n    independent coupling channel to be mixed. Default is 0");
  printf("\n  <down_mix_stereo> is flag for Downmix. Give 1 to");
  printf("\n    get stereo (downmix) output. Default is 0");
#endif
  printf("\n  <ld_frame_size> is to indicate ld frame size.");
  printf("\n   0 is for 512 frame length, 1 is for 480 frame length.");
  printf("\n    Default value is 512 (0)");
  printf("\n  <ld_testing_flag> is to enable / disable ld decoder testing.");
  printf("\n    Default value is 0");
  printf("\n  <peak_limiter_off_flag> is to enable / disable peak limiter.");
  printf("\n    Default value is 0");
  printf("\n  <error_concealment_flag> is to enable / disable error concealment.");
  printf("\n    Default value is 0");
  printf("\n  <esbr_flag> is to enable / disable eSBR. Default value is 1\n\n");
#ifdef LOUDNESS_LEVELING_SUPPORT
  printf("\n  <loudness_leveling_flag> is to enable / disable loudness leveling.");
  printf("\n    Default value is 1");
#endif
}

/*******************************************************************************/
/*                                                                             */
/*  Function Name : ixheaacd_main */
/*                                                                             */
/*  Description   : Main function */
/*                                                                             */
/*  Inputs        : None */
/*                                                                             */
/*  Globals       : None */
/*                                                                             */
/*  Processing    : Parse the parameter file and run the ixheaacd_main_process
 */
/*                                                                             */
/*  Outputs       : None */
/*                                                                             */
/*  Returns       : 0 on success, -1 on error */
/*                                                                             */
/*  Issues        : None */
/*                                                                             */
/*  Revision history : */
/*                                                                             */
/*        DD MM YYYY       Author                Changes */
/*        04 09 2005       Ittiam                Created */
/*                                                                             */
/*                                                                             */
/*******************************************************************************/

int main(WORD32 argc, char *argv[]) {
  WORD32 i, err_code = IA_NO_ERROR;

  ia_testbench_error_handler_init();

  g_pf_inp = NULL;
  g_pf_meta = NULL;
  g_pf_out = NULL;

  for (i = 1; i < argc; i++) {
    printf("%s ", argv[i]);

    if (!strncmp((const char *)argv[i], "-ifile:", 7)) {
      pWORD8 pb_arg_val = (pWORD8)argv[i] + 7;

      g_pf_inp = FileWrapper_Open((char *)pb_arg_val);
      if (g_pf_inp == NULL) {
        err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
        ixheaacd_error_handler(&ixheaacd_ia_testbench_error_info,
                               (pWORD8) "Input File", err_code);
        exit(1);
      }
      raw_testing = 0;
    }

    if (!strncmp((const char *)argv[i], "-imeta:", 7)) {
      pWORD8 pb_arg_val = (pWORD8)argv[i] + 7;

      g_pf_meta = fopen((const char *)pb_arg_val, "r");

      if (g_pf_meta == NULL) {
        err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
        ixheaacd_error_handler(&ixheaacd_ia_testbench_error_info,
                               (pWORD8) "Metadata File", err_code);
        exit(1);
      }

      err_code = ixheaacd_read_metadata_info(g_pf_meta, &meta_info);

      if (err_code == -1) {
        exit(1);
      }
      raw_testing = 1;
    }

    if (!strncmp((const char *)argv[i], "-ofile:", 7)) {
      pWORD8 pb_arg_val = (pWORD8)argv[i] + 7;

      g_pf_out = fopen((const char *)pb_arg_val, "wb");
      if (g_pf_out == NULL) {
        err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
        ixheaacd_error_handler(&ixheaacd_ia_testbench_error_info,
                               (pWORD8) "Output File", err_code);
        exit(1);
      }
    }
  }

  if ((g_pf_inp == NULL) || (g_pf_out == NULL)) {
    print_usage();
    err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
    ixheaacd_error_handler(&ixheaacd_ia_testbench_error_info,
                           (pWORD8) "Input or Output File", err_code);
    exit(1);
  }

  g_w_malloc_count = 0;

  printf("\n");

  for (i = 0; i < argc; i++) {
    if (!strcmp((pCHAR8)argv[i], "-mp4:1")) {
      if (g_pf_meta == NULL) {
        print_usage();
        err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
        ixheaacd_error_handler(&ixheaacd_ia_testbench_error_info,
                               (pWORD8) "Metadata File", err_code);
        exit(1);
      }
    }
  }

  for (i = 0; i < argc; i++) {
    if (strcmp((pCHAR8)argv[i], "-eld_testing:1"))
      eld_testing = 0;
    else {
      eld_testing = 1;
      break;
    }
  }

  ixheaacd_main_process(argc - 1, (pWORD8 *)(&argv[1]));

  for (i = 0; i < g_w_malloc_count; i++) {
    if (g_pv_arr_alloc_memory[i]) free(g_pv_arr_alloc_memory[i]);
  }
  if (g_pf_out) fclose(g_pf_out);

  if (g_pf_meta) {
    fclose(g_pf_meta);
    metadata_mp4_stsz_size_free(&meta_info);
  }
  FileWrapper_Close(g_pf_inp);
  mpeg_d_drc_on = 0;

  return IA_NO_ERROR;
} /* end ixheaacd_main */