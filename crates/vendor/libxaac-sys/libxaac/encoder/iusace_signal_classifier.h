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
#define FD_MODE 2
#define TD_MODE 0
#define MIN_POW -200
#define INDEXOFLOWFREQUENCY 160

#define NFRAMEAHEAD 1
#define AVE_TONAL_LENGTH 100
#define AVE_TONAL_LENGTH_SHORT 10
#define SPECTRAL_TILT_LENGTH 80
#define SPECTRAL_TILT_LENGTH_SHORT 20
#define SMOOTHING_LENGTH 100

#define NO_BORDER 0
#define BORDER_MUSIC_SPEECH 1
#define BORDER_MUSIC_SPEECH_DEFINITE 2
#define BORDER_SPEECH_MUSIC 3
#define BORDER_SPEECH_MUSIC_DEFINITE 4

#define TBD 0
#define SPEECH_DEFINITE 1
#define SPEECH 2
#define MUSIC_DEFINITE 3
#define MUSIC 4
#define LOG_1024_BASE_10 3.01029995664f
#define LOG_768_BASE_10 (2.88536122003f)

typedef struct {
  WORD32 smoothing_result_buf[100];            /**<buffer of smoothed mode decisions */
  WORD32 init_result_behind[100];              /**<buffer of past mode decisions */
  WORD32 init_result_ahead[NFRAMEAHEAD];       /**<buffer of ahead mode decisions */
  WORD32 flag_border_buf_behind[10];           /**<buffer of past border flags */
  WORD32 flag_border_buf_ahead[NFRAMEAHEAD];   /**<buffer of ahead border flags */
  FLOAT32 frame_energy_buf_behind[10];         /**<buffer of past energies */
  FLOAT32 frame_energy_buf_ahead[NFRAMEAHEAD]; /**<buffer of ahead energies */
} ia_classification_buf_struct;

typedef struct {
  WORD32 coding_mode; /**< coding mode of the frame */
  WORD32 pre_mode;    /**< coding mode of the previous frame */

  FLOAT32 input_samples[3840 * 2];
  WORD32 n_buffer_samples;
  WORD32 class_buf[10];
  WORD32 n_buf_class;
  WORD32 n_class_frames;

  WORD32 is_switch_mode;

  WORD32 framecnt;
  WORD32 init_flag;
  WORD32 framecnt_xm;

  ia_classification_buf_struct buffers;
  FLOAT32 spec_tilt_buf[100]; /* buffer of spectral tilt */
  WORD32 n_tonal[100];        /* buffer of tonal */
  WORD32 n_tonal_low_frequency[100];
  FLOAT32 msd_spec_tilt_buf[5];
  FLOAT32 msd_spec_tilt_short_buf[5]; /* buffer of the MSD of spectral tilt */
  FLOAT32 ave_n_tonal_short_buf[5];
  FLOAT32 ave_n_tonal_buf[5]; /* buffer of the AVE of tonal */
} ia_classification_struct;

typedef struct {
  FLOAT32 *time_signal;          /**<input signals */
  WORD32 framecnt_xm;            /**<frame counter
                                  */
  WORD32 *n_tonal;               /**<buffer of the numbers of tonal
                                  */
  WORD32 *n_tonal_low_frequency; /**<buffer of the numbers of tonal in the low frequency domain
                                  */
  FLOAT32 *n_tonal_low_frequency_ratio; /**<the ratio of distribution of the numbers of tonal in
                                           the low frequency domain*/
  FLOAT32 *ave_n_tonal;                 /**<long - term AVE of tonal
                                         */
  FLOAT32 *ave_n_tonal_short;           /**<short - term AVE of tonal */
} ia_tonal_params_struct;

typedef struct {
  WORD32 framecnt;                     /**< frame counter*/
  WORD32 *framecnt_xm;                 /**< frame counter*/
  WORD32 *flag_border;                 /**< flag of current border*/
  FLOAT32 ave_n_tonal_short;           /**< short - term AVE of tonal*/
  FLOAT32 ave_n_tonal;                 /**< long - term AVE of tonal*/
  FLOAT32 *ave_n_tonal_short_buf;      /**< buffer of short - term AVE of tonal*/
  FLOAT32 *ave_n_tonal_buf;            /**< buffer long - term AVE of tonal*/
  FLOAT32 msd_spec_tilt;               /**< long - term MSD of spectral tilt*/
  FLOAT32 msd_spec_tilt_short;         /**< short - term MSD of spectral tilt*/
  FLOAT32 *msd_spec_tilt_buf;          /**< buffer of long - term MSD of spectral tilt*/
  FLOAT32 *msd_spec_tilt_short_buf;    /**< buffer of short - term MSD of spectral tilt*/
  FLOAT32 n_tonal_low_frequency_ratio; /**< the ratio of distribution of the numbers of tonal in
                                          the low frequency domain*/
  FLOAT32 frame_energy;                /**< the energy of current frame*/
} ia_mode_params_struct;

typedef struct {
  WORD32 init_mode_decision_result; /**<  initial mode decision				*/
  WORD32 *init_result_behind;       /**<  buffer of past mode decisions		*/
  WORD32 *init_result_ahead;        /**<  buffer of ahead mode decisions	*/
  WORD32 flag_border;               /**<  current flag of border			*/
  WORD32 *flag_border_buf_behind;   /**<  buffer of past flags of border	*/
  WORD32 *flag_border_buf_ahead;    /**<  buffer of ahead flags of border	*/
  FLOAT32 frame_energy;             /**<  the energy of current frame		*/
  FLOAT32 *frame_energy_buf_behind; /**<  buffer of past frame energies		*/
  FLOAT32 *frame_energy_buf_ahead;  /**<  buffer of ahead frame energies	*/
  WORD32 *smoothing_result_buf;     /**<  buffer of smoothed mode decision	*/
  WORD32 flag_speech_definite;
  WORD32 count_small_energy;
  WORD32 flag_music_definite;
  WORD32 num_smoothing;
} ia_smooth_params_struct; /**<  final mode decision result        */

typedef struct {
  FLOAT32 *time_signal;         /**<input signals                    */
  WORD32 framecnt_xm;           /**<frame counter					*/
  FLOAT32 *spec_tilt_buf;       /**<buffer of spectral tilt			*/
  FLOAT32 *msd_spec_tilt;       /**<long - term MSD of spectral tilt	*/
  FLOAT32 *msd_spec_tilt_short; /**<short - term MSD of spectral tilt*/
  FLOAT32 frame_energy;
} ia_spec_tilt_params_struct;

typedef struct {
  const FLOAT64 twiddle_table_fft_float[514];
  const FLOAT64 hanning_window_1024[FRAME_LEN_LONG];
  const FLOAT64 absolute_threshold_1024[FRAME_LEN_LONG / 2];
  const FLOAT64 hanning_window_768[LEN_SUPERFRAME_768];
  const FLOAT64 absolute_threshold_768[LEN_SUPERFRAME_768 / 2];
} ia_signal_classifier_tables;

extern const ia_signal_classifier_tables iusace_classify_arrays;

VOID iusace_init_classification(ia_classification_struct *pstr_sig_class);
