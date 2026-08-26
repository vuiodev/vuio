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

#pragma once
struct ixheaace_mps_structure {
  ixheaace_mps_pstr_space_structure ptr_sac_encoder;

  WORD32 audio_object_type;

  ixheaace_mps_buf_descr in_buf_desc;
  ixheaace_mps_buf_descr out_buf_desc;
  ixheaace_mps_in_args in_args;
  ixheaace_mps_out_args out_args;
  VOID *p_in_buffer[2];
  UWORD32 p_in_buffer_size[2];
  UWORD32 p_in_buffer_el_size[2];
  UWORD32 p_in_buffer_type[2];

  VOID *p_out_buffer[4];
  UWORD32 p_out_buffer_size[2];
  UWORD32 p_out_buffer_el_size[4];
  UWORD32 p_out_buffer_type[4];

  UWORD8 sac_out_buffer[MPS_PAYLOAD_SIZE];

  WORD8 *ptr_scratch;
};

typedef struct {
  WORD32 audio_object_type;
  UWORD32 sbr_ratio;
  UWORD32 sampling_rate;
  UWORD32 bitrate_min;
  UWORD32 bitrate_max;
} ixheaace_mps_config_table;

typedef struct ixheaace_mps_structure ixheaace_mps_struct, *ixheaace_mps_pstr_struct;
