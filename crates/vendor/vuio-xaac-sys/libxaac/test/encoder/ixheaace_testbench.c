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
/*****************************************************************************/
/* File includes                                                             */
/*****************************************************************************/
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_user_config.h"
#include "iusace_cnst.h"
#include "ixheaace_api.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_handler.h"
#include "ixheaace_loudness_measurement.h"

VOID ia_enhaacplus_enc_error_handler_init();
VOID ia_testbench_error_handler_init();

extern ia_error_info_struct ia_testbench_error_info;
extern ia_error_info_struct ia_enhaacplus_enc_error_info;

/*****************************************************************************/
/* Constant hash defines                                                     */
/*****************************************************************************/
#define IA_MAX_CMD_LINE_LENGTH 300
#define IA_MAX_ARGS 20
#define IA_SCREEN_WIDTH 80
#define APP_BITRES_SIZE_CONFIG_PARAM_DEF_VALUE_LC (768)
#define APP_BITRES_SIZE_CONFIG_PARAM_DEF_VALUE_LD (384)

#define PARAMFILE "paramfilesimple.txt"

/*****************************************************************************/
/* Error codes for the testbench                                             */
/*****************************************************************************/
#define IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED 0xFFFFA001
#define DRC_CONFIG_FILE "impd_drc_config_params.txt"
/*****************************************************************************/
/* Application Context structure                                                          */
/*****************************************************************************/
typedef struct {
  FILE *pf_inp;
  FILE *pf_out;
  FILE *pf_meta;
  WORD32 use_ga_hdr;
} ixheaace_app_context;

int ia_enhaacplus_enc_fread(void *buf, int size, int bytes, FILE *fp) {
  return (int)fread(buf, size, bytes, fp);
}

IA_ERRORCODE ia_enhaacplus_enc_pcm_data_read(FILE *in_file, UWORD32 num_samples,
                                             WORD32 num_channels, WORD16 **data) {
  UWORD32 count = 0;
  WORD16 temp;
  WORD16 *buf = &temp;
  UWORD8 channel_no;
  UWORD32 sample_no = 0;

  while (count < num_samples) {
    sample_no = count / num_channels;
    channel_no = (UWORD8)(count % num_channels);
    if (fread(buf, sizeof(WORD16), 1, in_file) != 1) {
      if (feof(in_file)) {
        printf("End of file reached.\n");
      } else {
        printf("Error reading a file.\n");
        return -1;
      }
    }
    data[channel_no][sample_no] = temp;
    count++;
  }
  return IA_NO_ERROR;
}

int ia_enhaacplus_enc_fwrite(void *pb_buf, FILE *pf_out, WORD32 i_out_bytes) {
  fwrite(pb_buf, sizeof(char), i_out_bytes, pf_out);
  return 1;
}

IA_ERRORCODE ia_enhaacplus_enc_wav_header_decode(FILE *in_file, UWORD32 *n_channels,
                                                 UWORD32 *i_channel_mask, UWORD32 *sample_rate,
                                                 UWORD32 *pcm_sz, WORD32 *length) {
  WORD8 wav_hdr[40 + 36];
  WORD8 data_start[4];
  WORD16 num_ch;
  UWORD32 f_samp;
  WORD16 output_format;
  WORD32 check, count = 0;
  FLAG wav_format_pcm = 0, wav_format_extensible = 0;
  UWORD16 cb_size = 0;
  WORD32 curr_pos, size;

  *i_channel_mask = 0;

  if (fread(wav_hdr, 1, 40, in_file) != 40) return 1;

  if (wav_hdr[0] != 'R' && wav_hdr[1] != 'I' && wav_hdr[2] != 'F' && wav_hdr[3] != 'F') {
    return 1;
  }

  if (wav_hdr[20] == 01 && wav_hdr[21] == 00) {
    wav_format_pcm = 1;
  } else if (wav_hdr[20] == ((WORD8)-2) && wav_hdr[21] == ((WORD8)-1)) {
    wav_format_extensible = 1;
  } else {
    return 1;
  }

  num_ch = (WORD16)((UWORD8)wav_hdr[23] * 256 + (UWORD8)wav_hdr[22]);
  f_samp = ((UWORD8)wav_hdr[27] * 256 * 256 * 256);
  f_samp += ((UWORD8)wav_hdr[26] * 256 * 256);
  f_samp += ((UWORD8)wav_hdr[25] * 256);
  f_samp += ((UWORD8)wav_hdr[24]);

  output_format = ((UWORD8)wav_hdr[35] * 256);
  output_format += ((UWORD8)wav_hdr[34]);

  *n_channels = num_ch;
  *sample_rate = f_samp;
  *pcm_sz = output_format;

  if (wav_format_pcm) {
    data_start[0] = wav_hdr[36];
    data_start[1] = wav_hdr[37];
    data_start[2] = wav_hdr[38];
    data_start[3] = wav_hdr[39];
  } else if (wav_format_extensible) {
    cb_size |= ((UWORD8)wav_hdr[37] << 8);
    cb_size |= ((UWORD8)wav_hdr[36]);

    if (fread(&(wav_hdr[40]), 1, (UWORD16)(cb_size - 2 + 4), in_file) !=
        (UWORD16)(cb_size - 2 + 4))
      return 1;

    if (cb_size > 34) {
      return 1;
    }

    *i_channel_mask = 0;
    *i_channel_mask |= (UWORD8)wav_hdr[43] << 24;
    *i_channel_mask |= (UWORD8)wav_hdr[42] << 16;
    *i_channel_mask |= (UWORD8)wav_hdr[41] << 8;
    *i_channel_mask |= (UWORD8)wav_hdr[40];

    data_start[0] = wav_hdr[40 + cb_size - 2 + 0];
    data_start[1] = wav_hdr[40 + cb_size - 2 + 1];
    data_start[2] = wav_hdr[40 + cb_size - 2 + 2];
    data_start[3] = wav_hdr[40 + cb_size - 2 + 3];
  }

  check = 1;
  while (check) {
    if (data_start[0] == 'd' && data_start[1] == 'a' && data_start[2] == 't' &&
        data_start[3] == 'a') {
      if (1 != fread(length, 4, 1, in_file)) return 1;
      check = 0;

      curr_pos = ftell(in_file);
      fseek(in_file, 0L, SEEK_END);
      size = ftell(in_file) - curr_pos;
      if (*length > size) {
        printf("\n Inconsitent file size \n");
        *length = size;
      }
      fseek(in_file, curr_pos, SEEK_SET);
    } else {
      data_start[0] = data_start[1];
      data_start[1] = data_start[2];
      data_start[2] = data_start[3];
      if (1 != fread(&data_start[3], 1, 1, in_file)) return 1;
    }
    count++;
    if (count > 40) {
      *length = 0xffffffff;
      return (1);
    }
  }
  return IA_NO_ERROR;
}

void ia_enhaacplus_enc_print_usage() {
  printf("\nUsage:\n");
  printf("\n<executable> -ifile:<inputfile> -ofile:<outputfile> [options]\n");
  printf("\nor\n");
  printf("\n<executable> -paramfile:<paramfile>\n");
  printf("\n[options] can be,");
  printf("\n[-br:<bitrate>]");
  printf("\n[-mps:<use_mps>]");
  printf("\n[-adts:<use_adts_flag>]");
  printf("\n[-tns:<use_tns_flag>]");
  printf("\n[-nf:<use_noise_filling>]");
  printf("\n[-cmpx_pred:<use_complex_prediction>]");
  printf("\n[-framesize:<framesize_to_be_used>]");
  printf("\n[-aot:<audio_object_type>]");
  printf("\n[-esbr:<esbr_flag>]");
  printf("\n[-full_bandwidth:<enable_full_bandwidth>]");
  printf("\n[-max_out_buffer_per_ch:<bitreservoir_size>]");
  printf("\n[-tree_cfg:<tree_config>]");
  printf("\n[-usac:<usac_encoding_mode>]");
  printf("\n[-ccfl_idx:<corecoder_framelength_index>]");
  printf("\n[-pvc_enc:<pvc_enc_flag>]");
  printf("\n[-harmonic_sbr:<harmonic_sbr_flag>]");
  printf("\n[-esbr_hq:<esbr_hq_flag>]");
  printf("\n[-drc:<drc_flag>]");
  printf("\n[-inter_tes_enc:<inter_tes_enc_flag>]");
  printf("\n[-rap:<random access interval in ms>]");
  printf("\n[-stream_id:<stream identifier>]");
  printf("\n[-delay_adjust:<delay adjustment>]");
  printf("\n\nwhere, \n  <paramfile> is the parameter file with multiple commands");
  printf("\n  <inputfile> is the input 16-bit WAV or PCM file name");
  printf("\n  <outputfile> is the output ADTS/ES file name");
  printf("\n  <bitrate> is the bit-rate in bits per second. Default value is 48000. ");
  printf("\n  <use_mps> Valid values are 0 (disable MPS) and 1 (enable MPS). Default is 0.");
  printf(
      "\n  <use_adts_flag> Valid values are 0 ( No ADTS header) and 1 ( generate ADTS header). "
      "Default is 0.");
  printf("\n  <use_tns_flag> Valid values are 0 (disable TNS) and 1 (enable TNS). Default is 1.");
  printf("\n  <use_noise_filling> controls usage of noise filling in encoding. Default 0.");
  printf(
      "\n  <use_complex_prediction> controls usage of complex prediction in encoding. Default "
      "0.");
  printf("\n  <framesize_to_be_used> is the framesize to be used.");
  printf(
      "\n        For AOT 23, 39 (LD core coder profiles) valid values are 480 and 512. Default "
      "is "
      "512.");
  printf(
      "\n        For AOT 2, 5, 29 (LC core coder profiles) valid values are 960 and 1024. "
      "Default "
      "is 1024.");
  printf(
      "\n        For AOT 42 (USAC profile) valid values are 768 and 1024. "
      "Default "
      "is 1024.");
  printf("\n  <audio_object_type> is the Audio object type");
  printf("\n        2 for AAC-LC");
  printf("\n        5 for HE-AACv1(Legacy SBR)");
  printf("\n        23 for AAC-LD");
  printf("\n        29 for HE-AACv2");
  printf("\n        39 for AAC-ELD");
  printf("\n        42 for USAC");
  printf("\n        Default is 2 for AAC-LC.");
  printf("\n  <esbr_flag> Valid values are 0 (disable eSBR) and 1 (enable eSBR).");
  printf("\n      Default is 0 for HE-AACv1 profile (legacy SBR) and 1 for USAC profile.");
  printf(
      "\n  <enable_full_bandwidth> Enable use of full bandwidth of input. Valid values are "
      "0(disable full bandwidth) and 1(enable full bandwidth). Default is 0.");
  printf("\n  <bitreservoir_size> is the maximum size of bit reservoir to be used.");
  printf(
      "\n        Valid values are from -1 to 6144. -1 will omit use of bit reservoir. Default is "
      "384.");
  printf(
      "\n  <tree_config> MPS tree config"
      "\n        0 for '212'"
      "\n        1 for '5151'"
      "\n        2 for '5152'"
      "\n        3 for '525'"
      "\n        Default '212' for stereo input and '5151' for 6ch input.");
  printf(
      "\n  <usac_encoding_mode> USAC encoding mode to be chose"
      "0 for 'usac_switched'"
      "1 for 'usac_fd'"
      "2 for 'usac_td'"
      "Default 'usac_fd'");
  printf(
      "\n  <corecoder_framelength_index> is the core coder framelength index for USAC encoder");
  printf("\n    Valid values are 0, 1, 2, 3, 4. eSBR enabling is implicit");
  printf("\n    0 - Core coder framelength of USAC is  768 and eSBR is disabled");
  printf("\n    1 - Core coder framelength of USAC is 1024 and eSBR is disabled");
  printf("\n    2 - Core coder framelength of USAC is  768 and eSBR ratio 8:3");
  printf("\n    3 - Core coder framelength of USAC is 1024 and eSBR ratio 2:1");
  printf("\n    4 - Core coder framelength of USAC is 1024 and eSBR ratio 4:1");
  printf(
      "\n  <pvc_enc_flag> Valid values are 0 (disable PVC encoding) and "
      "1 (enable PVC encoding). Default is 0.");
  printf(
      "\n  <harmonic_sbr_flag> Valid values are 0 (disable harmonic SBR) and "
      "1 (enable harmonic SBR). Default is 0.");
  printf(
      "\n  <esbr_hq_flag> Valid values are 0 (disable high quality eSBR) and "
      "1 (enable high quality eSBR). Default is 0.");
  printf(
      "\n  <drc_flag> Valid values are 0 (disable DRC encoding) and "
      "1 (enable DRC encoding). Default is 0.");
  printf(
      "\n  <inter_tes_enc_flag> Valid values are 0 (disable inter - TES encoding) and "
      "1 (enable inter - TES encoding). Default is 0.");
  printf(
      "\n  <random access interval in ms> is the time interval between audio preroll frames in "
      "ms. It is applicable only for AOT 42."
      "\n        Valid values are -1 (Audio preroll sent only at beginning of file) and "
      "greater than 1000 ms. Default is -1.");
  printf(
      "\n <stream identifier> is the stream id used to uniquely identify configuration of a "
      "stream within a set of associated streams."
      "\n        It is applicable only for AOT 42. Valid values are 0 to 65535. Default is 0.");
  printf(
    "\n <delay adjustment> is used to discard algorithmic delay from the decoded file."
    "\n        It is applicable only for AOT 42. Valid values are 0 and 1. Default is 1.");
  exit(1);
}

static VOID ixheaace_parse_config_param(WORD32 argc, pWORD8 argv[], pVOID ptr_enc_api) {
  LOOPIDX i;
  ixheaace_user_config_struct *pstr_enc_api = (ixheaace_user_config_struct *)ptr_enc_api;

  for (i = 0; i < argc; i++) {
    /* Stream bit rate */
    if (!strncmp((const char *)argv[i], "-br:", 4)) {
      char *pb_arg_val = (char *)argv[i] + 4;
      pstr_enc_api->input_config.i_bitrate = atoi(pb_arg_val);
      fprintf(stdout, "Stream bit rate = %d\n", pstr_enc_api->input_config.i_bitrate);
    }
    /* MPS */
    if (!strncmp((const char *)argv[i], "-mps:", 5)) {
      char *pb_arg_val = (char *)argv[i] + 5;
      pstr_enc_api->input_config.i_use_mps = atoi(pb_arg_val);
    }
    /* Use TNS */
    if (!strncmp((const char *)argv[i], "-tns:", 5)) {
      char *pb_arg_val = (char *)argv[i] + 5;
      pstr_enc_api->input_config.aac_config.use_tns = atoi(pb_arg_val);
      pstr_enc_api->input_config.user_tns_flag = 1;
    }
    /*noise filling*/
    if (!strncmp((pCHAR8)argv[i], "-nf:", 4)) {
      pCHAR8 pb_arg_val = (pCHAR8)argv[i] + 4;
      pstr_enc_api->input_config.aac_config.noise_filling = atoi(pb_arg_val);
    }
    /* Complex Prediction */
    if (!strncmp((pCHAR8)argv[i], "-cmpx_pred:", 11)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 11);
      pstr_enc_api->input_config.cplx_pred = atoi(pb_arg_val);
    }
    /*Use full bandwidth*/
    if (!strncmp((const char *)argv[i], "-full_bandwidth:", 16)) {
      char *pb_arg_val = (char *)argv[i] + 16;
      pstr_enc_api->input_config.aac_config.full_bandwidth = atoi(pb_arg_val);
    }
    /* frame size */
    if (!strncmp((const char *)argv[i], "-framesize:", 11)) {
      char *pb_arg_val = (char *)argv[i] + 11;
      pstr_enc_api->input_config.frame_length = atoi(pb_arg_val);
      pstr_enc_api->input_config.frame_cmd_flag = 1;
    }
    if (!strncmp((const char *)argv[i], "-aot:", 5)) {
      char *pb_arg_val = (char *)argv[i] + 5;
      pstr_enc_api->input_config.aot = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-esbr:", 6)) {
      char *pb_arg_val = (char *)argv[i] + 6;
      pstr_enc_api->input_config.esbr_flag = atoi(pb_arg_val);
      pstr_enc_api->input_config.user_esbr_flag = 1;
    }

    if (!strncmp((const char *)argv[i], "-max_out_buffer_per_ch:", 23)) {
      char *pb_arg_val = (char *)argv[i] + 23;
      pstr_enc_api->input_config.aac_config.bitreservoir_size = atoi(pb_arg_val);
      pstr_enc_api->input_config.out_bytes_flag = 1;
    }
    if (!strncmp((const char *)argv[i], "-tree_cfg:", 10)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 10);
      pstr_enc_api->input_config.i_mps_tree_config = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-adts:", 6)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 6);
      pstr_enc_api->input_config.i_use_adts = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-usac:", 6)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 6);
      pstr_enc_api->input_config.codec_mode = atoi(pb_arg_val);
      pstr_enc_api->input_config.usac_en = 1;
      pstr_enc_api->input_config.aot = AOT_USAC;
    }
    if (!strncmp((const char *)argv[i], "-ccfl_idx:", 10)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 10);
      pstr_enc_api->input_config.ccfl_idx = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-pvc_enc:", 9)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 9);
      pstr_enc_api->input_config.pvc_active = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-harmonic_sbr:", 14)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 14);
      pstr_enc_api->input_config.harmonic_sbr = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-esbr_hq:", 9)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 9);
      pstr_enc_api->input_config.hq_esbr = atoi(pb_arg_val);
    }
    /* DRC */
    if (!strncmp((pCHAR8)argv[i], "-drc:", 5)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 5);
      pstr_enc_api->input_config.use_drc_element = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-inter_tes_enc:", 15)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 15);
      pstr_enc_api->input_config.inter_tes_active = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-rap:", 5)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 5);
      pstr_enc_api->input_config.random_access_interval = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-stream_id:", 11)) {
      pCHAR8 pb_arg_val = (pCHAR8)(argv[i] + 11);
      pstr_enc_api->input_config.stream_id = atoi(pb_arg_val);
    }
    if (!strncmp((const char *)argv[i], "-delay_adjust:", 14)) {
      pWORD8 pb_arg_val = argv[i] + 14;
      pstr_enc_api->input_config.use_delay_adjustment = atoi((const char *)pb_arg_val);
    }
  }

  return;
}

VOID ia_enhaacplus_enc_display_id_message(WORD8 lib_name[], WORD8 lib_version[]) {
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
    spclen = IA_SCREEN_WIDTH / 2 - (WORD32)(strlen((const char *)str[i]) / 2);
    spaces[spclen] = '\0';
    printf("%s", (const char *)spaces);
    spaces[spclen] = ' ';
    printf("%s", (const char *)str[i]);
  }
}

pVOID malloc_global(UWORD32 size, UWORD32 alignment) {
#ifdef WIN32
  return _aligned_malloc(size, alignment);
#else
  pVOID ptr = NULL;
  if (posix_memalign((VOID **)&ptr, alignment, size)) {
    ptr = NULL;
  }
  return ptr;
#endif
}

VOID free_global(pVOID ptr) {
#ifdef WIN32
  _aligned_free(ptr);
#else
  free(ptr);
#endif
  ptr = NULL;
}

static VOID iaace_aac_set_default_config(ixheaace_aac_enc_config *config) {
  /* make the pre initialization of the structs flexible */
  memset(config, 0, sizeof(*config));

  /* default configurations */
  config->bitrate = 48000;
  config->bandwidth = 0;
  config->inv_quant = 2;
  config->use_tns = 0;
  config->noise_filling = 0;
  config->bitreservoir_size = APP_BITRES_SIZE_CONFIG_PARAM_DEF_VALUE_LC;
}

static VOID ixheaace_print_drc_config_params(ixheaace_input_config *pstr_input_config,
                                             ixheaace_input_config *pstr_input_config_user,
                                             ixheaace_output_config *pstr_output_config) {
  WORD32 flag = 0, i, j, k;
  ia_drc_input_config *drc_cfg = (ia_drc_input_config *)(pstr_input_config->pv_drc_cfg);
  ia_drc_input_config *drc_cfg_user = (ia_drc_input_config *)(pstr_input_config_user->pv_drc_cfg);

  ia_drc_uni_drc_config_struct *pstr_uni_drc_config = &drc_cfg->str_uni_drc_config;
  ia_drc_uni_drc_config_struct *pstr_uni_drc_config_user = &drc_cfg_user->str_uni_drc_config;

  ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set =
      &drc_cfg->str_enc_loudness_info_set;
  ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set_user =
      &drc_cfg_user->str_enc_loudness_info_set;
  ia_drc_loudness_info_set_ext_eq_struct *pstr_enc_loudness_info_set_ext =
      &drc_cfg->str_enc_loudness_info_set.str_loudness_info_set_extension
           .str_loudness_info_set_ext_eq;
  ia_drc_loudness_info_set_ext_eq_struct *pstr_enc_loudness_info_set_ext_user =
      &drc_cfg_user->str_enc_loudness_info_set.str_loudness_info_set_extension
           .str_loudness_info_set_ext_eq;

  for (i = 0; i < pstr_uni_drc_config->drc_instructions_uni_drc_count; i++) {
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].additional_downmix_id_count !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].additional_downmix_id_count) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_location !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].drc_location) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
            .drc_set_target_loudness_value_upper !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i]
            .drc_set_target_loudness_value_upper) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
            .drc_set_target_loudness_value_lower !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i]
            .drc_set_target_loudness_value_lower) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
            .drc_set_target_loudness_value_lower !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i]
            .drc_set_target_loudness_value_lower) {
      flag = 1;
    }
    for (j = 0; j < MAX_CHANNEL_COUNT; j++) {
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].gain_set_index[j] !=
          pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].gain_set_index[j]) {
        flag = 1;
      }
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].num_drc_channel_groups !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].num_drc_channel_groups) {
      flag = 1;
    }
    for (j = 0; j < pstr_uni_drc_config->str_drc_instructions_uni_drc[i].num_drc_channel_groups;
         j++) {
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
              .str_gain_modifiers[j]
              .attenuation_scaling[0] != pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i]
                                             .str_gain_modifiers[j]
                                             .attenuation_scaling[0]) {
        flag = 1;
      }
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
              .str_gain_modifiers[j]
              .amplification_scaling[0] !=
          pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i]
              .str_gain_modifiers[j]
              .amplification_scaling[0]) {
        flag = 1;
      }
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
              .str_gain_modifiers[j]
              .gain_offset[0] != pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i]
                                     .str_gain_modifiers[j]
                                     .gain_offset[0]) {
        flag = 1;
      }
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].limiter_peak_target !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].limiter_peak_target) {
      flag = 1;
    }
#ifdef LOUDNESS_LEVELING_SUPPORT
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_set_effect !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].drc_set_effect) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].leveling_present !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].leveling_present) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present !=
        pstr_uni_drc_config_user->str_drc_instructions_uni_drc[i].ducking_only_set_present) {
      flag = 1;
    }
#endif
  }
  if (flag == 1) {
    printf("\nDRC : Invalid config str_drc_instructions_uni_drc");
    flag = 0;
  }
  for (i = 0; i < pstr_uni_drc_config->drc_coefficients_uni_drc_count; i++) {
    if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].drc_location !=
        pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i].drc_location) {
      flag = 1;
    }
    if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].gain_set_count !=
        pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i].gain_set_count) {
      flag = 1;
    }
    for (j = 0; j < pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].gain_set_count; j++) {
      if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
              .str_gain_set_params[j]
              .gain_coding_profile != pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i]
                                          .str_gain_set_params[j]
                                          .gain_coding_profile) {
        flag = 1;
      }
      if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
              .str_gain_set_params[j]
              .band_count != pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i]
                                 .str_gain_set_params[j]
                                 .band_count) {
        flag = 1;
      }
      for (k = 0;
           k <
           pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].str_gain_set_params[j].band_count;
           k++) {
        if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                .str_gain_set_params[j]
                .gain_params[k]
                .nb_points != pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i]
                                  .str_gain_set_params[j]
                                  .gain_params[k]
                                  .nb_points) {
          flag = 1;
        }
        if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                .str_gain_set_params[j]
                .gain_params[k]
                .drc_characteristic != pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i]
                                           .str_gain_set_params[j]
                                           .gain_params[k]
                                           .drc_characteristic) {
          flag = 1;
        }
        if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                .str_gain_set_params[j]
                .gain_params[k]
                .crossover_freq_index != pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i]
                                             .str_gain_set_params[j]
                                             .gain_params[k]
                                             .crossover_freq_index) {
          flag = 1;
        }
        if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                .str_gain_set_params[j]
                .gain_params[k]
                .start_sub_band_index != pstr_uni_drc_config_user->str_drc_coefficients_uni_drc[i]
                                             .str_gain_set_params[j]
                                             .gain_params[k]
                                             .start_sub_band_index) {
          flag = 1;
        }
      }
    }
  }
  if (flag == 1) {
    printf("\nDRC : Invalid config: str_drc_coefficients_uni_drc");
    flag = 0;
  }
#ifdef LOUDNESS_LEVELING_SUPPORT
  ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext =
      &pstr_uni_drc_config->str_uni_drc_config_ext;
  ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext_user =
      &pstr_uni_drc_config_user->str_uni_drc_config_ext;

  for (i = 0; i < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count; i++) {
    if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].drc_set_effect !=
        pstr_uni_drc_config_ext_user->str_drc_instructions_uni_drc_v1[i].drc_set_effect) {
      flag = 1;
    }
    if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].leveling_present !=
        pstr_uni_drc_config_ext_user->str_drc_instructions_uni_drc_v1[i].leveling_present) {
      flag = 1;
    }
    if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].ducking_only_set_present !=
        pstr_uni_drc_config_ext_user->str_drc_instructions_uni_drc_v1[i]
            .ducking_only_set_present) {
      flag = 1;
    }
  }

  if (flag == 1) {
    printf("\nDRC : Invalid config str_drc_instructions_uni_drc_v1");
    flag = 0;
  }
#endif

  if (pstr_output_config->is_loudness_configured) {
    for (i = 0; i < pstr_enc_loudness_info_set->loudness_info_count; i++) {
      if (pstr_enc_loudness_info_set->str_loudness_info[i].sample_peak_level !=
          pstr_enc_loudness_info_set_user->str_loudness_info[i].sample_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info[i].true_peak_level !=
          pstr_enc_loudness_info_set_user->str_loudness_info[i].true_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info[i].true_peak_level_measurement_system !=
          pstr_enc_loudness_info_set_user->str_loudness_info[i]
              .true_peak_level_measurement_system) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info[i].true_peak_level_reliability !=
          pstr_enc_loudness_info_set_user->str_loudness_info[i].true_peak_level_reliability) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info[i].measurement_count !=
          pstr_enc_loudness_info_set_user->str_loudness_info[i].measurement_count) {
        flag = 1;
      }
      for (j = 0; j < pstr_enc_loudness_info_set->str_loudness_info[i].measurement_count; j++) {
        if (pstr_enc_loudness_info_set->str_loudness_info[i]
                .str_loudness_measure[j]
                .method_definition != pstr_enc_loudness_info_set_user->str_loudness_info[i]
                                          .str_loudness_measure[j]
                                          .method_definition) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set->str_loudness_info[i]
                .str_loudness_measure[j]
                .method_value != pstr_enc_loudness_info_set_user->str_loudness_info[i]
                                     .str_loudness_measure[j]
                                     .method_value) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set->str_loudness_info[i]
                .str_loudness_measure[j]
                .measurement_system != pstr_enc_loudness_info_set_user->str_loudness_info[i]
                                           .str_loudness_measure[j]
                                           .measurement_system) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set->str_loudness_info[i]
                .str_loudness_measure[j]
                .reliability != pstr_enc_loudness_info_set_user->str_loudness_info[i]
                                    .str_loudness_measure[j]
                                    .reliability) {
          flag = 1;
        }
      }
    }
    if (flag == 1) {
      printf("\nDRC : Invalid config str_loudness_info");
      flag = 0;
    }

    for (i = 0; i < pstr_enc_loudness_info_set->loudness_info_album_count; i++) {
      if (pstr_enc_loudness_info_set->str_loudness_info_album[i].sample_peak_level !=
          pstr_enc_loudness_info_set_user->str_loudness_info_album[i].sample_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info_album[i].true_peak_level !=
          pstr_enc_loudness_info_set_user->str_loudness_info_album[i].true_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info_album[i]
              .true_peak_level_measurement_system !=
          pstr_enc_loudness_info_set_user->str_loudness_info_album[i]
              .true_peak_level_measurement_system) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info_album[i].true_peak_level_reliability !=
          pstr_enc_loudness_info_set_user->str_loudness_info_album[i]
              .true_peak_level_reliability) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set->str_loudness_info_album[i].measurement_count !=
          pstr_enc_loudness_info_set_user->str_loudness_info_album[i].measurement_count) {
        flag = 1;
      }
      for (j = 0; j < pstr_enc_loudness_info_set->str_loudness_info_album[i].measurement_count;
           j++) {
        if (pstr_enc_loudness_info_set->str_loudness_info_album[i]
                .str_loudness_measure[j]
                .method_definition != pstr_enc_loudness_info_set_user->str_loudness_info_album[i]
                                          .str_loudness_measure[j]
                                          .method_definition) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set->str_loudness_info_album[i]
                .str_loudness_measure[j]
                .method_value != pstr_enc_loudness_info_set_user->str_loudness_info_album[i]
                                     .str_loudness_measure[j]
                                     .method_value) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set->str_loudness_info_album[i]
                .str_loudness_measure[j]
                .measurement_system != pstr_enc_loudness_info_set_user->str_loudness_info_album[i]
                                           .str_loudness_measure[j]
                                           .measurement_system) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set->str_loudness_info_album[i]
                .str_loudness_measure[j]
                .reliability != pstr_enc_loudness_info_set_user->str_loudness_info_album[i]
                                    .str_loudness_measure[j]
                                    .reliability) {
          flag = 1;
        }
      }
    }
    if (flag == 1) {
      printf("\nDRC : Invalid config str_loudness_info_album");
    }

    for (i = 0; i < pstr_enc_loudness_info_set_ext->loudness_info_v1_count; i++) {
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].sample_peak_level !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i].sample_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].true_peak_level !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i].true_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
              .true_peak_level_measurement_system !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i]
              .true_peak_level_measurement_system) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].true_peak_level_reliability !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i]
              .true_peak_level_reliability) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].measurement_count !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i].measurement_count) {
        flag = 1;
      }
      for (j = 0; j < pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].measurement_count;
           j++) {
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                .str_loudness_measure[j]
                .method_definition != pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i]
                                          .str_loudness_measure[j]
                                          .method_definition) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                .str_loudness_measure[j]
                .method_value != pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i]
                                     .str_loudness_measure[j]
                                     .method_value) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                .str_loudness_measure[j]
                .measurement_system !=
            pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i]
                .str_loudness_measure[j]
                .measurement_system) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                .str_loudness_measure[j]
                .reliability != pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1[i]
                                    .str_loudness_measure[j]
                                    .reliability) {
          flag = 1;
        }
      }
    }
    if (flag == 1) {
      printf("\nDRC : Invalid config str_loudness_info_v1");
      flag = 0;
    }

    for (i = 0; i < pstr_enc_loudness_info_set_ext->loudness_info_v1_album_count; i++) {
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].sample_peak_level !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i].sample_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].true_peak_level !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i].true_peak_level) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
              .true_peak_level_measurement_system !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i]
              .true_peak_level_measurement_system) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
              .true_peak_level_reliability !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i]
              .true_peak_level_reliability) {
        flag = 1;
      }
      if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].measurement_count !=
          pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i].measurement_count) {
        flag = 1;
      }
      for (j = 0;
           j < pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].measurement_count;
           j++) {
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .method_definition !=
            pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .method_definition) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .method_value !=
            pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .method_value) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .measurement_system !=
            pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .measurement_system) {
          flag = 1;
        }
        if (pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                .str_loudness_measure[j]
                .reliability != pstr_enc_loudness_info_set_ext_user->str_loudness_info_v1_album[i]
                                    .str_loudness_measure[j]
                                    .reliability) {
          flag = 1;
        }
      }
    }
    if (flag == 1) {
      printf("\nDRC : Invalid config str_loudness_info_v1_album");
    }
  }
}

static VOID ixheaace_print_config_params(ixheaace_input_config *pstr_input_config,
                                         ixheaace_input_config *pstr_input_config_user,
                                         ixheaace_output_config *pstr_output_config) {
  printf(
      "\n*************************************************************************************"
      "***********\n");
  printf("\nParameters Taken:\n");
  if (pstr_input_config_user->aot == pstr_input_config->aot) {
    printf("\nAOT : %d", pstr_input_config->aot);
  } else {
    printf("\nAOT (Invalid config value, setting to default) : %d", pstr_input_config->aot);
  }
  if (pstr_input_config->aot == AOT_AAC_LC) {
    printf(" - AAC LC ");
  } else if (pstr_input_config->aot == AOT_SBR) {
    if (pstr_input_config->esbr_flag == 1) {
      printf(" - HEAACv1 (eSBR) ");
    } else {
      printf(" - HEAACv1 (Legacy SBR) ");
    }
  } else if (pstr_input_config->aot == AOT_AAC_ELD) {
    printf(" - AAC ELD");
    if (pstr_input_config->i_use_mps == 1) {
      printf("v2");
    }
  } else if (pstr_input_config->aot == AOT_AAC_LD) {
    printf(" - AAC LD ");
  } else if (pstr_input_config->aot == AOT_USAC) {
    printf(" - USAC ");
    if (pstr_input_config->i_use_mps == 1) {
      printf(" MPS ");
    }
  } else if (pstr_input_config->aot == AOT_PS) {
    printf(" - HEAACv2 ");
  }
  if (pstr_input_config->aot == AOT_PS || pstr_input_config->aot == AOT_SBR) {
    if (pstr_input_config_user->esbr_flag == pstr_input_config->esbr_flag) {
      printf("\nESBR Flag : %d", pstr_input_config->esbr_flag);
    } else {
      printf("\nESBR Flag (Invalid config value, setting to default) : %d",
             pstr_input_config->esbr_flag);
    }
  }
  if (pstr_input_config->aot == AOT_USAC) {
    if (pstr_input_config_user->codec_mode == pstr_input_config->codec_mode) {
      printf("\nUSAC Codec Mode : ");
    } else {
      printf("\nUSAC Codec Mode (Invalid config value, setting to default) : ");
    }

    if (pstr_input_config->usac_en) {
      if (pstr_input_config->codec_mode == USAC_SWITCHED) {
        printf("Switched Mode");
      } else if (pstr_input_config->codec_mode == USAC_ONLY_FD) {
        printf("FD Mode");
      } else if (pstr_input_config->codec_mode == USAC_ONLY_TD) {
        printf("TD Mode");
      }
    } else {
      printf("Not Enabled");
    }
  }

  if (pstr_input_config->aot == AOT_USAC) {
    if (pstr_input_config_user->harmonic_sbr == pstr_input_config->harmonic_sbr) {
      printf("\nHarmonic SBR : %d", pstr_input_config->harmonic_sbr);
    } else {
      printf("\nHarmonic SBR (Invalid config value, setting to default) : %d",
             pstr_input_config->harmonic_sbr);
    }
    if (pstr_input_config_user->hq_esbr == pstr_input_config->hq_esbr) {
      printf("\nHigh quality esbr : %d", pstr_input_config->hq_esbr);
    } else {
      printf("\nHigh quality esbr Flag (Invalid config value, setting to default) : %d",
             pstr_input_config->hq_esbr);
    }
    if (pstr_input_config_user->cplx_pred == pstr_input_config->cplx_pred) {
      printf("\nComplex Prediction Flag : %d", pstr_input_config->cplx_pred);
    } else {
      printf("\nComplex Prediction Flag (Invalid config value, setting to default) : %d",
             pstr_input_config->cplx_pred);
    }
    if (pstr_input_config_user->aac_config.use_tns == pstr_input_config->aac_config.use_tns) {
      printf("\nTNS Flag : %d", pstr_input_config->aac_config.use_tns);
    } else {
      printf("\nTNS Flag (Invalid config value, setting to default) : %d",
             pstr_input_config->aac_config.use_tns);
    }

    if (pstr_input_config_user->ccfl_idx == pstr_input_config->ccfl_idx) {
      printf("\nCore-coder framelength index : %d", pstr_input_config->ccfl_idx);
    } else {
      if (pstr_input_config_user->ccfl_idx >= NO_SBR_CCFL_768 &&
          pstr_input_config_user->ccfl_idx <= SBR_4_1) {
        if (pstr_input_config_user->ccfl_idx == NO_SBR_CCFL_768 &&
            pstr_input_config->ccfl_idx == SBR_8_3) {
          printf(
              "\nCore-coder framelength index (Unsupported configuration, enabling 8:3 eSBR) : "
              "%d",
              pstr_input_config->ccfl_idx);
        }
        else if (pstr_input_config_user->ccfl_idx == NO_SBR_CCFL_1024 &&
            pstr_input_config->ccfl_idx == SBR_2_1) {
          printf(
              "\nCore-coder framelength index (Unsupported configuration, enabling 2:1 eSBR) : "
              "%d",
              pstr_input_config->ccfl_idx);
        }
        else if (pstr_input_config_user->ccfl_idx != pstr_input_config->ccfl_idx)
        {
          printf(
            "\nCore-coder framelength index changed from %d to %d ",
            pstr_input_config_user->ccfl_idx, pstr_input_config->ccfl_idx);
        }
      } else {
          printf(
            "\nCore-coder framelength index (Invalid input config value, setting to default): %d"
            , pstr_input_config->ccfl_idx);
      }
    }
  }
  if (pstr_input_config_user->aac_config.noise_filling ==
      pstr_input_config->aac_config.noise_filling) {
    printf("\nNoise Filling Flag : %d", pstr_input_config->aac_config.noise_filling);
  } else {
    printf("\nNoise Filling Flag (Invalid config value, setting to default) : %d",
           pstr_input_config->aac_config.noise_filling);
  }
  if (pstr_input_config_user->i_use_adts == pstr_input_config->i_use_adts) {
    printf("\nUse ADTS Flag : %d", pstr_input_config->i_use_adts);
  } else {
    printf("\nUse ADTS Flag (Invalid config value, setting to default) : %d",
           pstr_input_config->i_use_adts);
  }

  if (pstr_input_config->aot != AOT_USAC) {
    if (pstr_input_config_user->aac_config.full_bandwidth ==
        pstr_input_config->aac_config.full_bandwidth) {
      printf("\nFull Bandwidth Flag : %d", pstr_input_config->aac_config.full_bandwidth);
    } else {
      printf("\nFull Bandwidth Flag (Invalid config value, setting to default) : %d",
             pstr_input_config->aac_config.full_bandwidth);
    }
  }
  if (pstr_input_config_user->aac_config.use_tns == pstr_input_config->aac_config.use_tns) {
    printf("\nUse TNS Flag : %d", pstr_input_config->aac_config.use_tns);
  } else {
    printf("\nUse TNS Flag (Invalid config value, setting to default) : %d",
           pstr_input_config->aac_config.use_tns);
  }
  if (pstr_input_config->aot == AOT_AAC_LD || pstr_input_config->aot == AOT_AAC_ELD) {
    if (pstr_input_config_user->aac_config.bitreservoir_size ==
        pstr_input_config->aac_config.bitreservoir_size) {
      printf("\nBitreservoir Size : %d", pstr_input_config->aac_config.bitreservoir_size);
    } else {
      printf("\nBitreservoir Size (Invalid config value, setting to default) : %d",
             pstr_input_config->aac_config.bitreservoir_size);
    }
  }

  printf("\nBitrate : %d bps", pstr_input_config->i_bitrate);

  if (pstr_input_config_user->i_use_mps != pstr_input_config->i_use_mps) {
    printf("\nMPS (Invalid config value, setting to default) : %d ",
           pstr_input_config->i_use_mps);
  }
  if (pstr_input_config->i_use_mps) {
    if (pstr_input_config_user->i_mps_tree_config == pstr_input_config->i_mps_tree_config) {
      printf("\nTree config : %d ", pstr_input_config->i_mps_tree_config);
    } else {
      printf("\nTree config (Invalid tree config value, setting to default) : %d ",
             pstr_input_config->i_mps_tree_config);
    }
  } else if (!(pstr_input_config->i_use_mps) && (pstr_input_config_user->i_mps_tree_config !=
                                                 pstr_input_config->i_mps_tree_config)) {
    printf("\nTree config (Invalid tree config value ) : %d ",
           pstr_input_config->i_mps_tree_config);
  }
  if (pstr_input_config->frame_cmd_flag) {
    if (pstr_input_config_user->frame_length == pstr_input_config->frame_length) {
      printf("\nFrame Length : %d", pstr_input_config->frame_length);
    } else {
      printf("\nFrame Length (Invalid config value, setting to default) : %d",
             pstr_input_config->frame_length);
    }
  } else {
    printf("\nFrame Length : %d", pstr_input_config->frame_length);
  }
  if ((pstr_input_config_user->i_samp_freq != pstr_input_config->i_samp_freq) &&
      (pstr_input_config->i_use_adts == 1)) {
    printf(
        "\nSampling Frequency (With adts:1, Setting non-standard Sampling Frequency to mapped "
        "standard Sampling Frequency) : %d Hz",
        pstr_input_config->i_samp_freq);
  } else {
    printf("\nSampling Frequency : %d Hz", pstr_input_config_user->i_samp_freq);
  }

  // DRC validation
  if (pstr_input_config->aot == AOT_USAC) {
    if (pstr_input_config->use_drc_element != pstr_input_config_user->use_drc_element) {
      printf("\nDRC (Invalid config value, setting to default) : %d",
             pstr_input_config->use_drc_element);
    }
    if (pstr_input_config->use_drc_element) {
      printf("\nDRC : 1");
      ixheaace_print_drc_config_params(pstr_input_config, pstr_input_config_user,
                                       pstr_output_config);
    }

    if (pstr_input_config->random_access_interval !=
        pstr_input_config_user->random_access_interval) {
      printf("\nRandom access interval (Invalid config value, setting to default) : %d",
             pstr_input_config->random_access_interval);
    }

    if (pstr_input_config->use_delay_adjustment !=
      pstr_input_config_user->use_delay_adjustment) {
      printf("\nDelay compensation (Invalid config value, setting to default) : %d",
        pstr_input_config->use_delay_adjustment);
    }
  }

  printf(
      "\n*************************************************************************************"
      "***********\n\n");
}

static IA_ERRORCODE ixheaace_calculate_loudness_measure(ixheaace_input_config *pstr_in_cfg,
                                                        ixheaace_output_config *pstr_out_cfg,
                                                        FILE *in_file) {
  WORD32 temp_pos, input_size;
  WORD32 count = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  temp_pos = ftell(in_file);
  VOID *loudness_handle =
      malloc_global(ixheaace_loudness_info_get_handle_size(), DEFAULT_MEM_ALIGN_8);
  if (loudness_handle == NULL) {
    printf("fatal error: libxaac encoder: Memory allocation failed");
    return -1;
  }
  input_size = (pstr_in_cfg->i_samp_freq / 10) * (pstr_in_cfg->i_channels);
  err_code = ixheaace_loudness_init_params(loudness_handle, pstr_in_cfg, pstr_out_cfg);
  if (err_code) {
    free_global(loudness_handle);
    return -1;
  }
  WORD16 **samples = 0;
  samples =
      (WORD16 **)malloc_global(pstr_in_cfg->i_channels * sizeof(*samples), DEFAULT_MEM_ALIGN_8);
  if (samples == NULL) {
    printf("fatal error: libxaac encoder: Memory allocation failed");
    free_global(loudness_handle);
    return -1;
  }
  for (count = 0; count < pstr_in_cfg->i_channels; count++) {
    samples[count] = (WORD16 *)malloc_global(
        (pstr_out_cfg->samp_freq / 10) * sizeof(*samples[count]), DEFAULT_MEM_ALIGN_8);
    if (samples[count] == NULL) {
      printf("fatal error: libxaac encoder: Memory allocation failed");
      while (count) {
        count--;
        free_global(samples[count]);
      }
      free_global(samples);
      free_global(loudness_handle);
      return -1;
    }
    memset(samples[count], 0, (pstr_out_cfg->samp_freq / 10) * sizeof(*samples[count]));
  }
  count = 0;
  WORD32 no_samples_per_frame = (WORD32)(pstr_out_cfg->samp_freq * 0.1 * pstr_in_cfg->i_channels);
  while (count <= ((pstr_in_cfg->aac_config.length / 2) - no_samples_per_frame)) {
    err_code =
        ia_enhaacplus_enc_pcm_data_read(in_file, input_size, pstr_in_cfg->i_channels, samples);
    if (err_code) {
      printf("fatal error: libxaac encoder: Reading PCM data failed");
      for (count = 0; count < pstr_in_cfg->i_channels; count++) {
        free_global(samples[count]);
      }
      free_global(samples);
      free_global(loudness_handle);
      return -1;
    }
    pstr_in_cfg->measured_loudness = ixheaace_measure_loudness(loudness_handle, samples);
    count += no_samples_per_frame;
  }
  if (pstr_in_cfg->method_def == METHOD_DEFINITION_PROGRAM_LOUDNESS) {
    pstr_in_cfg->measured_loudness = ixheaace_measure_integrated_loudness(loudness_handle);
    pstr_in_cfg->sample_peak_level = ixheaace_measure_sample_peak_value(loudness_handle);
  }
  fseek(in_file, temp_pos, SEEK_SET);
  for (count = 0; count < pstr_in_cfg->i_channels; count++) {
    free_global(samples[count]);
  }
  free_global(samples);
  free_global(loudness_handle);
  return err_code;
}

IA_ERRORCODE ia_enhaacplus_enc_main_process(ixheaace_app_context *pstr_context, WORD32 argc,
                                            pWORD8 argv[]) {
  LOOPIDX frame_count = 0;

  /* Error code */
  IA_ERRORCODE err_code = IA_NO_ERROR;

  /* API obj */
  pVOID pv_ia_process_api_obj;
  /* First part                                        */
  /* Error Handler Init                                */
  /* Get Library Name, Library Version and API Version */
  /* Initialize API structure + Default config set     */
  /* Set config params from user                       */
  /* Initialize memory tables                          */
  /* Get memory information and allocate memory        */

  pWORD8 pb_inp_buf = NULL, pb_out_buf = NULL;
  WORD32 i_bytes_read = 0;
  WORD32 input_size = 0;
  WORD32 samp_freq;
  FLOAT32 down_sampling_ratio = 1;
  WORD32 i_out_bytes = 0;
  WORD32 i_total_length = 0;
  WORD32 start_offset_samples = 0, i_dec_len = 0;
  UWORD32 *ia_stsz_size = NULL;
  UWORD32 ui_samp_freq, ui_num_chan, ui_pcm_wd_sz, ui_pcm = 0, ui_channel_mask,
                                                   ui_num_coupling_chans = 0;
  WORD32 max_frame_size = 0;
  WORD32 expected_frame_count = 0;
  FILE *pf_drc_inp = NULL;
  /* The error init function */
  VOID (*p_error_init)();

  /* The process error info structure */
  ia_error_info_struct *p_proc_err_info;

  /* ******************************************************************/
  /* The API config structure                                         */
  /* ******************************************************************/
  ixheaace_user_config_struct str_enc_api = {{0}, {0}};
  ixheaace_input_config *pstr_in_cfg = &str_enc_api.input_config;
  ixheaace_output_config *pstr_out_cfg = &str_enc_api.output_config;
  pstr_in_cfg->pv_drc_cfg = malloc_global(sizeof(ia_drc_input_config), DEFAULT_MEM_ALIGN_8);
  if (pstr_in_cfg->pv_drc_cfg == NULL) {
    printf("fatal error: libxaac encoder: Memory allocation failed");
    return -1;
  }
  ia_drc_input_config *pstr_drc_cfg = (ia_drc_input_config *)pstr_in_cfg->pv_drc_cfg;

  /* Stack process struct initing */
  p_error_init = ia_enhaacplus_enc_error_handler_init;
  p_proc_err_info = &ia_enhaacplus_enc_error_info;
  /* Stack process struct initing end */

  /* ******************************************************************/
  /* Initialize the error handler                                     */
  /* ******************************************************************/
  (*p_error_init)();

  /* ******************************************************************/
  /* Parse input configuration parameters                             */
  /* ******************************************************************/

  iaace_aac_set_default_config(&pstr_in_cfg->aac_config);

  pstr_in_cfg->i_bitrate = pstr_in_cfg->aac_config.bitrate;
  pstr_in_cfg->aot = AOT_AAC_LC;
  pstr_in_cfg->codec_mode = USAC_ONLY_FD;
  pstr_in_cfg->i_channels = 2;
  pstr_in_cfg->i_samp_freq = 44100;
  pstr_in_cfg->i_use_mps = 0;
  pstr_in_cfg->i_mps_tree_config = -1;
  pstr_in_cfg->i_use_adts = 0;
  pstr_in_cfg->esbr_flag = 0;
  pstr_in_cfg->i_use_es = 1;
  pstr_in_cfg->cplx_pred = 0;
  pstr_in_cfg->ccfl_idx = NO_SBR_CCFL_1024;
  pstr_in_cfg->pvc_active = 0;
  pstr_in_cfg->harmonic_sbr = 0;
  pstr_in_cfg->inter_tes_active = 0;
  pstr_in_cfg->use_drc_element = 0;
  pstr_in_cfg->i_channels_mask = 0;
  pstr_in_cfg->i_num_coupling_chan = 0;
  pstr_in_cfg->aac_config.full_bandwidth = 0;
  pstr_out_cfg->malloc_xheaace = &malloc_global;
  pstr_out_cfg->free_xheaace = &free_global;
  pstr_in_cfg->frame_cmd_flag = 0;
  pstr_in_cfg->out_bytes_flag = 0;
  pstr_in_cfg->user_tns_flag = 0;
  pstr_in_cfg->user_esbr_flag = 0;
  pstr_in_cfg->i_use_adts = !pstr_context->use_ga_hdr;
  pstr_in_cfg->random_access_interval = DEFAULT_RAP_INTERVAL_IN_MS;
  pstr_in_cfg->method_def = METHOD_DEFINITION_PROGRAM_LOUDNESS;
  pstr_in_cfg->measurement_system = MEASUREMENT_SYSTEM_BS_1770_3;
  pstr_in_cfg->use_delay_adjustment = USAC_DEFAULT_DELAY_ADJUSTMENT_VALUE;

  /* ******************************************************************/
  /* Parse input configuration parameters                             */
  /* ******************************************************************/
  ixheaace_parse_config_param(argc, argv, &str_enc_api);

  {
    if (pstr_in_cfg->aot == AOT_AAC_LC || pstr_in_cfg->aot == AOT_SBR ||
        pstr_in_cfg->aot == AOT_PS) {
      if (pstr_in_cfg->frame_cmd_flag == 0) {
        pstr_in_cfg->frame_length = 1024;
      }
      if (pstr_in_cfg->out_bytes_flag == 0) {
        pstr_in_cfg->aac_config.bitreservoir_size = APP_BITRES_SIZE_CONFIG_PARAM_DEF_VALUE_LC;
      }
      if (pstr_in_cfg->user_tns_flag == 0) {
        pstr_in_cfg->aac_config.use_tns = 1;
      }
    } else if (pstr_in_cfg->aot == AOT_AAC_LD || pstr_in_cfg->aot == AOT_AAC_ELD) {
      if (pstr_in_cfg->frame_cmd_flag == 0) {
        pstr_in_cfg->frame_length = 512;
      }
      if (pstr_in_cfg->out_bytes_flag == 0) {
        pstr_in_cfg->aac_config.bitreservoir_size = APP_BITRES_SIZE_CONFIG_PARAM_DEF_VALUE_LD;
      }

      if (pstr_in_cfg->user_tns_flag == 0) {
        pstr_in_cfg->aac_config.use_tns = 1;
      }
    } else if (pstr_in_cfg->aot == AOT_USAC) {
      if (pstr_in_cfg->user_esbr_flag == 0) {
        pstr_in_cfg->esbr_flag = 1;
      }
      if (pstr_in_cfg->user_tns_flag == 0) {
        pstr_in_cfg->aac_config.use_tns = 1;
      }
    }
  }

  ui_samp_freq = pstr_in_cfg->i_samp_freq;
  ui_num_chan = pstr_in_cfg->i_channels;
  ui_channel_mask = pstr_in_cfg->i_channels_mask;
  ui_num_coupling_chans = pstr_in_cfg->i_num_coupling_chan;
  ui_pcm_wd_sz = pstr_in_cfg->ui_pcm_wd_sz;
  if (!(pstr_in_cfg->i_use_adts)) {
    pstr_in_cfg->i_use_es = 1;
  } else {
    pstr_in_cfg->i_use_es = 0;
  }

  if (!ui_pcm) {
    /* Decode WAV header */
    if (ia_enhaacplus_enc_wav_header_decode(pstr_context->pf_inp, &ui_num_chan, &ui_channel_mask,
                                            &ui_samp_freq, &ui_pcm_wd_sz, &i_total_length) == 1) {
      fprintf(stdout, "Unable to Read Input WAV File\n");
      exit(1);
    }

    /* PCM Word Size (For single input file) */
    pstr_in_cfg->ui_pcm_wd_sz = ui_pcm_wd_sz;
    /* Sampling Frequency */
    pstr_in_cfg->i_samp_freq = ui_samp_freq;
    /* Total Number of Channels */
    pstr_in_cfg->i_channels = ui_num_chan;
    /* Number of coupling channels*/
    pstr_in_cfg->i_num_coupling_chan = ui_num_coupling_chans;
    /* Channels Mask */
    pstr_in_cfg->i_channels_mask = ui_channel_mask;

    pstr_in_cfg->aac_config.length = i_total_length;
  }

  /*1st pass -> Loudness Measurement */
  if (pstr_in_cfg->aot == AOT_USAC || pstr_in_cfg->usac_en) {
    err_code =
        ixheaace_calculate_loudness_measure(pstr_in_cfg, pstr_out_cfg, pstr_context->pf_inp);
    if (err_code) {
      printf("\n Error in calculating loudness.\n");
      exit(1);
    } else {
      printf("\n loudness level : %lf", pstr_in_cfg->measured_loudness);
      printf("\n sample_peak_level : %lf \n", pstr_in_cfg->sample_peak_level);
    }
  }

  ixheaace_input_config pstr_in_cfg_user = *pstr_in_cfg;

  ia_drc_input_config *pstr_drc_cfg_user = NULL;

  /* Get library id and version number and display it */
  ixheaace_get_lib_id_strings((pVOID)&pstr_out_cfg->version);
  ia_enhaacplus_enc_display_id_message(pstr_out_cfg->version.p_lib_name,
                                       pstr_out_cfg->version.p_version_num);

  /* ******************************************************************/
  /* Initialize API structure and set config params to default        */
  /* ******************************************************************/
  /* DRC */
  if (pstr_in_cfg->use_drc_element == 1 && pstr_in_cfg->aot == AOT_USAC) {
    CHAR8 drc_config_file_name[IA_MAX_CMD_LINE_LENGTH];
    strcpy(drc_config_file_name, DRC_CONFIG_FILE);

    pf_drc_inp = fopen(drc_config_file_name, "rt");

    if (!pf_drc_inp) {
      printf("\nError in opening DRC configuration file\n\n");
      pstr_in_cfg->use_drc_element = 0;
    }

    if (pf_drc_inp != 0) {
      memset(pstr_drc_cfg, 0, sizeof(ia_drc_input_config));
      ixheaace_read_drc_config_params(
          pf_drc_inp, &pstr_drc_cfg->str_enc_params, &pstr_drc_cfg->str_uni_drc_config,
          &pstr_drc_cfg->str_enc_loudness_info_set, &pstr_drc_cfg->str_enc_gain_extension,
          pstr_in_cfg->i_channels);

      pstr_drc_cfg_user =
          (ia_drc_input_config *)malloc_global(sizeof(ia_drc_input_config), DEFAULT_MEM_ALIGN_8);
      if (pstr_drc_cfg_user == NULL) {
        printf("fatal error: libxaac encoder: Memory allocation failed");
        free_global(pstr_in_cfg->pv_drc_cfg);
        return -1;
      }
      // Copy DRC config to user DRC config
      memcpy(pstr_drc_cfg_user, pstr_drc_cfg, sizeof(ia_drc_input_config));

      pstr_in_cfg_user.pv_drc_cfg = pstr_drc_cfg_user;
    }
  }

  err_code = ixheaace_create((pVOID)pstr_in_cfg, (pVOID)pstr_out_cfg);
  _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code, pstr_out_cfg);

  pv_ia_process_api_obj = pstr_out_cfg->pv_ia_process_api_obj;

  pb_inp_buf = (pWORD8)pstr_out_cfg->mem_info_table[IA_MEMTYPE_INPUT].mem_ptr;
  pb_out_buf = (pWORD8)pstr_out_cfg->mem_info_table[IA_MEMTYPE_OUTPUT].mem_ptr;

  ixheaace_print_config_params(pstr_in_cfg, &pstr_in_cfg_user, pstr_out_cfg);

  if (pstr_drc_cfg_user) {
    free_global(pstr_drc_cfg_user);
    pstr_drc_cfg_user = NULL;
  }

  start_offset_samples = 0;
  input_size = pstr_out_cfg->input_size;
  expected_frame_count = pstr_out_cfg->expected_frame_count;

  if (NULL == ia_stsz_size) {
    ia_stsz_size = (UWORD32 *)malloc_global((expected_frame_count + 2) * sizeof(*ia_stsz_size),
                                            DEFAULT_MEM_ALIGN_8);
    if (ia_stsz_size == NULL) {
      if (pstr_in_cfg->pv_drc_cfg) {
        free_global(pstr_in_cfg->pv_drc_cfg);
      }
      printf("fatal error: libxaac encoder: Memory allocation failed");
      return -1;
    }
    memset(ia_stsz_size, 0, (expected_frame_count + 2) * sizeof(*ia_stsz_size));
  }
  down_sampling_ratio = pstr_out_cfg->down_sampling_ratio;
  samp_freq = (WORD32)(pstr_out_cfg->samp_freq / down_sampling_ratio);

  { ia_enhaacplus_enc_fwrite(pb_out_buf, pstr_context->pf_out, 0); }

  if ((pstr_in_cfg->usac_en || pstr_in_cfg->i_use_es)) {
    i_dec_len = pstr_out_cfg->i_out_bytes;
    ia_enhaacplus_enc_fwrite(pb_out_buf, pstr_context->pf_out, pstr_out_cfg->i_out_bytes);
    fflush(pstr_context->pf_out);
  }

  i_bytes_read =
      ia_enhaacplus_enc_fread((pVOID)pb_inp_buf, sizeof(WORD8), input_size, pstr_context->pf_inp);

  while (i_bytes_read) {
    /*****************************************************************************/
    /* Print frame number */
    /*****************************************************************************/
    fprintf(stdout, "Frames Processed [%d]\r", frame_count);
    fflush(stdout);

    if (i_bytes_read != input_size) {
      memset((pb_inp_buf + i_bytes_read), 0, (input_size - i_bytes_read));
    }

    /*****************************************************************************/
    /* Perform Encoding of frame data */
    /*****************************************************************************/

    err_code = ixheaace_process(pv_ia_process_api_obj, (pVOID)pstr_in_cfg, (pVOID)pstr_out_cfg);

    _IA_HANDLE_ERROR(p_proc_err_info, (pWORD8) "", err_code, pstr_out_cfg);

    /* Get the output bytes */
    i_out_bytes = pstr_out_cfg->i_out_bytes;

    if (max_frame_size < i_out_bytes) max_frame_size = i_out_bytes;
    if (i_out_bytes) {
      frame_count++;
      ia_stsz_size[frame_count - 1] = pstr_out_cfg->i_out_bytes;

      ia_enhaacplus_enc_fwrite(pb_out_buf, pstr_context->pf_out, i_out_bytes);
      fflush(pstr_context->pf_out);
      if (!pstr_in_cfg->use_delay_adjustment) {
         i_bytes_read = ia_enhaacplus_enc_fread((pVOID)pb_inp_buf, sizeof(WORD8), input_size,
           pstr_context->pf_inp);
      }
    }
    if (pstr_in_cfg->use_delay_adjustment) {
      i_bytes_read = ia_enhaacplus_enc_fread((pVOID)pb_inp_buf, sizeof(WORD8), input_size,
        pstr_context->pf_inp);
    }

    if (frame_count == expected_frame_count) break;
  }

  fprintf(stdout, "\n");
  fflush(stdout);

  // Error handler is not invoked here to avoid invoking ixheaace_delete() twice.
  err_code = ixheaace_delete((pVOID)pstr_out_cfg);
  if ((err_code)&IA_FATAL_ERROR) {
    if (pstr_in_cfg->pv_drc_cfg) {
      free_global(pstr_in_cfg->pv_drc_cfg);
    }
    if (ia_stsz_size != NULL) {
      free_global(ia_stsz_size);
    }
    return (err_code);
  }

  if ((pstr_in_cfg->usac_en || pstr_in_cfg->i_use_es) && (pstr_context->pf_meta)) {
    fprintf(pstr_context->pf_meta, "-dec_info_init:%d\n", i_dec_len);
    fprintf(pstr_context->pf_meta, "-g_track_count:%d\n", 1);
    fprintf(pstr_context->pf_meta, "-ia_mp4_stsz_entries:%d\n", frame_count);
    fprintf(pstr_context->pf_meta, "-movie_time_scale:%d\n", samp_freq);
    fprintf(pstr_context->pf_meta, "-media_time_scale:%d\n", samp_freq);
    fprintf(pstr_context->pf_meta, "-playTimeInSamples:%d\n",
            i_total_length / ((ui_pcm_wd_sz >> 3) * ui_num_chan));
    fprintf(pstr_context->pf_meta, "-startOffsetInSamples:%d\n-useEditlist:%d\n",
            start_offset_samples, 1);
    for (WORD32 i = 0; i < frame_count; i++)
      fprintf(pstr_context->pf_meta, "-ia_mp4_stsz_size:%d\n", ia_stsz_size[i]);
  }
  if (pstr_in_cfg->pv_drc_cfg) {
    free_global(pstr_in_cfg->pv_drc_cfg);
  }
  if (ia_stsz_size != NULL) {
    free_global(ia_stsz_size);
  }
  if (pf_drc_inp) {
    fclose(pf_drc_inp);
  }
  return IA_NO_ERROR;
}

int main(WORD32 argc, pCHAR8 argv[]) {
  FILE *param_file_id = NULL;
  WORD32 usac_en = 0;
  WORD8 curr_cmd[IA_MAX_CMD_LINE_LENGTH];
  WORD32 fargc, curpos;
  WORD32 processcmd = 0;
  WORD8 fargv[IA_MAX_ARGS][IA_MAX_CMD_LINE_LENGTH];

  pWORD8 pargv[IA_MAX_ARGS];

  WORD8 pb_input_file_path[IA_MAX_CMD_LINE_LENGTH] = "";
  WORD8 pb_output_file_path[IA_MAX_CMD_LINE_LENGTH] = "";
  WORD8 pb_drc_file_path[IA_MAX_CMD_LINE_LENGTH] = "";
  ixheaace_app_context str_context;
  memset(&str_context, 0, sizeof(ixheaace_app_context));
  str_context.use_ga_hdr = 1;
  ia_testbench_error_handler_init();

  ixheaace_version instance = {0};
  if (argc == 1 || argc == 2) {
    if (argc == 2 && (!strncmp((const char *)argv[1], "-paramfile:", 11))) {
      pWORD8 paramfile = (pWORD8)argv[1] + 11;

      param_file_id = fopen((const char *)paramfile, "r");
      if (param_file_id == NULL) {
        ixheaace_get_lib_id_strings(&instance);
        ia_enhaacplus_enc_display_id_message(instance.p_lib_name, instance.p_version_num);
        ia_enhaacplus_enc_print_usage();
        return IA_NO_ERROR;
      }
    } else {
      param_file_id = fopen(PARAMFILE, "r");
      if (param_file_id == NULL) {
        ixheaace_get_lib_id_strings(&instance);
        ia_enhaacplus_enc_display_id_message(instance.p_lib_name, instance.p_version_num);
        ia_enhaacplus_enc_print_usage();
        return IA_NO_ERROR;
      }
    }

    /* Process one line at a time */
    while (fgets((char *)curr_cmd, IA_MAX_CMD_LINE_LENGTH, param_file_id)) {
      WORD8 pb_meta_file_name[IA_MAX_CMD_LINE_LENGTH] = "";
      curpos = 0;
      fargc = 0;
      usac_en = 0;
      /* if it is not a param_file command and if */
      /* CLP processing is not enabled */
      if (curr_cmd[0] != '@' && !processcmd) { /* skip it */
        continue;
      }

      while (sscanf((char *)curr_cmd + curpos, "%s", fargv[fargc]) != EOF) {
        if (fargv[0][0] == '/' && fargv[0][1] == '/') break;
        if (strcmp((const char *)fargv[0], "@echo") == 0) break;
        if (strcmp((const char *)fargv[fargc], "@New_line") == 0) {
          if (NULL == fgets((char *)curr_cmd + curpos, IA_MAX_CMD_LINE_LENGTH, param_file_id))
            break;
          continue;
        }
        curpos += (WORD32)strlen((const char *)fargv[fargc]);
        while (*(curr_cmd + curpos) == ' ' || *(curr_cmd + curpos) == '\t') curpos++;
        fargc++;
      }

      if (fargc < 1) /* for blank lines etc. */
        continue;

      if (strcmp((const char *)fargv[0], "@Output_path") == 0) {
        if (fargc > 1)
          strcpy((char *)pb_output_file_path, (const char *)fargv[1]);
        else
          strcpy((char *)pb_output_file_path, "");
        continue;
      }

      if (strcmp((const char *)fargv[0], "@Input_path") == 0) {
        if (fargc > 1)
          strcpy((char *)pb_input_file_path, (const char *)fargv[1]);
        else
          strcpy((char *)pb_input_file_path, "");
        strcpy((char *)pb_drc_file_path, (const char *)pb_input_file_path);
        continue;
      }

      if (strcmp((const char *)fargv[0], "@Start") == 0) {
        processcmd = 1;
        continue;
      }

      if (strcmp((const char *)fargv[0], "@Stop") == 0) {
        processcmd = 0;
        continue;
      }

      /* otherwise if this a normal command and its enabled for execution */
      if (processcmd) {
        int i;
        int err_code = IA_NO_ERROR;
        WORD8 pb_input_file_name[IA_MAX_CMD_LINE_LENGTH] = "";
        WORD8 pb_output_file_name[IA_MAX_CMD_LINE_LENGTH] = "";
        WORD32 aot_value = 0;
        WORD32 is_ld_eld = 0;  // If set to 1, it denotes AOT 23 or AOT 39

        int file_count = 0;
        for (i = 0; i < fargc; i++) {
          printf("%s ", fargv[i]);
          pargv[i] = fargv[i];

          if (!strncmp((const char *)fargv[i], "-ifile:", 7)) {
            pWORD8 pb_arg_val = fargv[i] + 7;

            strcat((char *)pb_input_file_name, (const char *)pb_input_file_path);
            strcat((char *)pb_input_file_name, (const char *)pb_arg_val);

            str_context.pf_inp = NULL;
            str_context.pf_inp = fopen((const char *)pb_input_file_name, "rb");
            if (str_context.pf_inp == NULL) {
              err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
              ia_error_handler(&ia_testbench_error_info, (pWORD8) "Input File", err_code);
            }
            file_count++;
          }

          if (!strncmp((const char *)fargv[i], "-ofile:", 7)) {
            pWORD8 pb_arg_val = fargv[i] + 7;

            strcat((char *)pb_output_file_name, (const char *)pb_output_file_path);
            strcat((char *)pb_output_file_name, (const char *)pb_arg_val);

            str_context.pf_out = NULL;
            str_context.pf_out = fopen((const char *)pb_output_file_name, "wb");
            if (str_context.pf_out == NULL) {
              err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
              ia_error_handler(&ia_testbench_error_info, (pWORD8) "Output File", err_code);
            }
            file_count++;
          }
          if (!strncmp((const char *)fargv[i], "-usac:", 6)) {
            usac_en = 1;
          }
          if (!strncmp((const char *)fargv[i], "-aot:", 5)) {
            pWORD8 pb_arg_val = fargv[i] + 5;
            aot_value = atoi((const char *)(pb_arg_val));
            if (aot_value == 23 || aot_value == 39) {
              is_ld_eld = 1;
            }
          }
          if (!strncmp((const char *)fargv[i], "-adts:", 6)) {
            pWORD8 pb_arg_val = fargv[i] + 6;

            if ((atoi((const char *)pb_arg_val))) str_context.use_ga_hdr = 0;
          }
        }

        printf("\n");

        if (file_count != 2) {
          err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
          ia_error_handler(&ia_testbench_error_info, (pWORD8) "Input or Output File", err_code);
        }
        if (is_ld_eld) {
          str_context.use_ga_hdr = 1;
        }

        if ((strcmp((const char *)pb_output_file_name, "")) &&
            (usac_en || str_context.use_ga_hdr)) {
          char *file_name = strrchr((const char *)pb_output_file_name, '.');
          SIZE_T idx = file_name - (char *)pb_output_file_name;
          memcpy(pb_meta_file_name, pb_output_file_name, idx);
          strcat((char *)pb_meta_file_name, ".txt");
          str_context.pf_meta = NULL;
          str_context.pf_meta = fopen((const char *)pb_meta_file_name, "wt");
          if (str_context.pf_meta == NULL) {
            err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
            ia_error_handler(&ia_testbench_error_info, (pWORD8) "Meta File", err_code);
          }
        }
        if (err_code == IA_NO_ERROR) ia_enhaacplus_enc_main_process(&str_context, fargc, pargv);

        str_context.use_ga_hdr = 1;

        if (str_context.pf_inp) fclose(str_context.pf_inp);
        if (str_context.pf_out) fclose(str_context.pf_out);
        if (str_context.pf_meta != NULL) {
          fclose(str_context.pf_meta);
          str_context.pf_meta = NULL;
        }
      }
    }
  } else {
    int i;
    int err_code = IA_NO_ERROR;
    int file_count = 0;
    WORD32 aot_value = 0;
    WORD32 is_ld_eld = 0;  // If set to 1, it denotes AOT 23 or AOT 39

    WORD8 pb_input_file_name[IA_MAX_CMD_LINE_LENGTH] = "";
    WORD8 pb_output_file_name[IA_MAX_CMD_LINE_LENGTH] = "";
    WORD8 pb_meta_file_name[IA_MAX_CMD_LINE_LENGTH] = "";
    for (i = 1; i < argc; i++) {
      printf("%s ", argv[i]);

      if (!strncmp((const char *)argv[i], "-ifile:", 7)) {
        pWORD8 pb_arg_val = (pWORD8)argv[i] + 7;
        strcat((char *)pb_input_file_name, (const char *)pb_input_file_path);
        strcat((char *)pb_input_file_name, (const char *)pb_arg_val);

        str_context.pf_inp = NULL;
        str_context.pf_inp = fopen((const char *)pb_input_file_name, "rb");
        if (str_context.pf_inp == NULL) {
          err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
          ia_error_handler(&ia_testbench_error_info, (pWORD8) "Input File", err_code);
        }
        file_count++;
      }

      if (!strncmp((const char *)argv[i], "-ofile:", 7)) {
        pWORD8 pb_arg_val = (pWORD8)argv[i] + 7;

        strcat((char *)pb_output_file_name, (const char *)pb_output_file_path);
        strcat((char *)pb_output_file_name, (const char *)pb_arg_val);

        str_context.pf_out = NULL;
        str_context.pf_out = fopen((const char *)pb_output_file_name, "wb");
        if (str_context.pf_out == NULL) {
          err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
          ia_error_handler(&ia_testbench_error_info, (pWORD8) "Output File", err_code);
        }
        file_count++;
      }

      if (!strncmp((const char *)argv[i], "-usac:", 6)) {
        usac_en = 1;
      }
      if (!strncmp((const char *)argv[i], "-aot:", 5)) {
        pCHAR8 pb_arg_val = argv[i] + 5;
        aot_value = atoi((const char *)(pb_arg_val));
        if (aot_value == 23 || aot_value == 39) {
          is_ld_eld = 1;
        }
      }

      if (!strncmp((const char *)argv[i], "-adts:", 6)) {
        pCHAR8 pb_arg_val = argv[i] + 6;
        if (atoi((const char *)pb_arg_val)) str_context.use_ga_hdr = 0;
      }

      if (!strncmp((const char *)argv[i], "-help", 5)) {
        ia_enhaacplus_enc_print_usage();
      }
    }

    printf("\n");
    if (file_count != 2) {
      err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
      ia_error_handler(&ia_testbench_error_info, (pWORD8) "Input or Output File", err_code);
    }
    if (is_ld_eld) {
      str_context.use_ga_hdr = 1;
    }
#ifdef _WIN32
#pragma warning(suppress : 6001)
#endif

    if ((strcmp((const char *)pb_output_file_name, "")) && (usac_en || str_context.use_ga_hdr)) {
      char *file_name = strrchr((const char *)pb_output_file_name, '.');
      SIZE_T idx = file_name - (char *)pb_output_file_name;
      memcpy(pb_meta_file_name, pb_output_file_name, idx);
      strcat((char *)pb_meta_file_name, ".txt");
      str_context.pf_meta = NULL;
      str_context.pf_meta = fopen((const char *)pb_meta_file_name, "wt");
      if (str_context.pf_meta == NULL) {
        err_code = IA_TESTBENCH_MFMAN_FATAL_FILE_OPEN_FAILED;
        ia_error_handler(&ia_testbench_error_info, (pWORD8) "Meta File", err_code);
      }
    }
    if (err_code == IA_NO_ERROR)
      ia_enhaacplus_enc_main_process(&str_context, argc - 1, (pWORD8 *)&argv[1]);

    str_context.use_ga_hdr = 1;
    if (str_context.pf_inp) fclose(str_context.pf_inp);
    if (str_context.pf_out) fclose(str_context.pf_out);
    if (str_context.pf_meta != NULL) {
      fclose(str_context.pf_meta);
      str_context.pf_meta = NULL;
    }
  }
  if (param_file_id != NULL) {
    fclose(param_file_id);
  }

  return IA_NO_ERROR;
}
