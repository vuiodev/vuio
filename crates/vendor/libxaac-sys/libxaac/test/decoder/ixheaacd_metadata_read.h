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
#ifndef __METADATA_H
#define __METADATA_H

#define MAX_TRACKS_PER_LAYER 50

typedef struct {
  UWORD32 *ia_mp4_stsz_size;
  UWORD32 ia_mp4_stsz_entries;
  UWORD32 fill_once;
  UWORD32 movie_time_scale;
  UWORD32 media_time_scale;

  UWORD32 dec_info_init;
  UWORD32 g_track_count;
  UWORD32 useEditlist[MAX_TRACKS_PER_LAYER];
  UWORD32 startOffsetInSamples[MAX_TRACKS_PER_LAYER];
  UWORD32 playTimeInSamples[MAX_TRACKS_PER_LAYER];

} metadata_info;

int ixheaacd_read_metadata_info(FILE *fp, metadata_info *meta_info);

int get_metadata_dec_info_init(metadata_info meta_info);

int get_metadata_dec_exec(metadata_info meta_info, int frame);

int get_start_offset_in_samples(metadata_info meta_info);

int get_play_time_in_samples(metadata_info meta_info);

void update_start_offset_in_samples(metadata_info meta_info, int update);

void update_play_time_in_samples(metadata_info meta_info, int update);

#endif
