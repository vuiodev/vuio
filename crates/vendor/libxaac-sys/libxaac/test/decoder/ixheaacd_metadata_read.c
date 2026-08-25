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
#include "ixheaac_type_def.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "ixheaacd_metadata_read.h"

#define IA_MAX_CMDLINE_LENGTH 100

void metadata_mp4_stsz_size_free(metadata_info *meta_info) {
  if (meta_info->ia_mp4_stsz_size != NULL) {
    free(meta_info->ia_mp4_stsz_size);
    meta_info->ia_mp4_stsz_size = NULL;
  }
}

int ixheaacd_read_metadata_info(FILE *g_pf_metadata, metadata_info *meta_info) {
  char cmd[IA_MAX_CMDLINE_LENGTH];

  WORD32 i, j, k, l;
  i = j = k = l = 0;
  metadata_mp4_stsz_size_free(meta_info);
  while (fgets((char *)cmd, IA_MAX_CMDLINE_LENGTH, g_pf_metadata)) {
    if (!strncmp((pCHAR8)cmd, "-dec_info_init:", 15)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 15);
      UWORD32 dec_info_init = atoi(pb_arg_val);
      meta_info->dec_info_init = dec_info_init;
    } else if (!strncmp((pCHAR8)cmd, "-g_track_count:", 15)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 15);
      UWORD32 g_track_count = atoi(pb_arg_val);
      meta_info->g_track_count = g_track_count;
    }

    else if (!strncmp((pCHAR8)cmd, "-movie_time_scale:", 18)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 18);
      UWORD32 movie_time_scale = atoi(pb_arg_val);
      meta_info->movie_time_scale = movie_time_scale;
    }

    else if (!strncmp((pCHAR8)cmd, "-media_time_scale:", 18)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 18);
      UWORD32 media_time_scale = atoi(pb_arg_val);
      meta_info->media_time_scale = media_time_scale;
    }

    else if (!strncmp((pCHAR8)cmd, "-ia_mp4_stsz_entries:", 21)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 21);
      UWORD32 ia_mp4_stsz_entries = atoi(pb_arg_val);
      meta_info->ia_mp4_stsz_entries = ia_mp4_stsz_entries;
      metadata_mp4_stsz_size_free(meta_info);
      meta_info->ia_mp4_stsz_size =
          (UWORD32 *)malloc(sizeof(UWORD32) * ia_mp4_stsz_entries);
      memset(meta_info->ia_mp4_stsz_size, 0,
             sizeof(UWORD32) * ia_mp4_stsz_entries);
    }
    /*
    else if {
      //printf("Wrong file order,Check file order");
      //return -1;
    }
    */
    else if (!strncmp((pCHAR8)cmd, "-playTimeInSamples:", 19)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 19);
      UWORD32 playTimeInSamples = atoi(pb_arg_val);
      meta_info->playTimeInSamples[i] = playTimeInSamples;
      i++;
    }

    else if (!strncmp((pCHAR8)cmd, "-startOffsetInSamples:", 22)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 22);
      UWORD32 startOffsetInSamples = atoi(pb_arg_val);
      meta_info->startOffsetInSamples[j] = startOffsetInSamples;
      j++;
    }

    else if (!strncmp((pCHAR8)cmd, "-useEditlist:", 13)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 13);
      UWORD32 useEditlist = atoi(pb_arg_val);
      meta_info->useEditlist[k] = useEditlist;
      k++;
    }

    else if (!strncmp((pCHAR8)cmd, "-ia_mp4_stsz_size:", 18)) {
      pCHAR8 pb_arg_val = (pCHAR8)(cmd + 18);
      UWORD32 ia_mp4_stsz_size = atoi(pb_arg_val);
      meta_info->ia_mp4_stsz_size[l] = ia_mp4_stsz_size;
      l++;
    }

    else {
      printf("Command not found");
      return -1;
    }
  }

  for (; i < MAX_TRACKS_PER_LAYER; i++) {
    meta_info->playTimeInSamples[i] = 0;
  }

  for (; j < MAX_TRACKS_PER_LAYER; j++) {
    meta_info->startOffsetInSamples[j] = 0;
  }

  for (; k < MAX_TRACKS_PER_LAYER; k++) {
    meta_info->useEditlist[k] = 0;
  }

  return 0;
}

int get_metadata_dec_info_init(metadata_info meta_info) {
  return meta_info.dec_info_init;
}

WORD32 get_metadata_dec_exec(metadata_info meta_info, int frame) {
  if (frame < (int)meta_info.ia_mp4_stsz_entries) {
    return meta_info.ia_mp4_stsz_size[frame];
  } else {
    return 0;
  }
}

int get_movie_time_scale(metadata_info meta_info) {
  return meta_info.movie_time_scale;
}

int get_media_time_scale(metadata_info meta_info) {
  return meta_info.media_time_scale;
}

int get_g_track_count(metadata_info meta_info) {
  return meta_info.g_track_count;
}

int get_use_edit_list(metadata_info meta_info) {
  return meta_info.useEditlist[0];
}

int get_start_offset_in_samples(metadata_info meta_info) {
  return (meta_info.startOffsetInSamples[0] + 2048);
}

int get_play_time_in_samples(metadata_info meta_info) {
  return meta_info.playTimeInSamples[0];
}
/*
void update_start_offset_in_samples(metadata_info meta_info, int update)
{

        meta_info.startOffsetInSamples[0]=update;
}
*/

/*
void update_play_time_in_samples(metadata_info meta_info, int update)
{

        meta_info.playTimeInSamples[0]=update;

}
*/
