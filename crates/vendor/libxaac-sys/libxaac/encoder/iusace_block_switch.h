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
VOID iusace_init_block_switching(ia_block_switch_ctrl *pstr_blk_switch_ctrl,
                                 const WORD32 bit_rate, const WORD32 num_chans);
VOID iusace_block_switching(ia_block_switch_ctrl *ptr_blk_switch_ctrl, const FLOAT32 *ptr_in,
                            WORD32 ccfl);
VOID iusace_sync_block_switching(ia_block_switch_ctrl *ptr_blk_switch_left_ctrl,
                                 ia_block_switch_ctrl *ptr_blk_switch_right_ctrl);
