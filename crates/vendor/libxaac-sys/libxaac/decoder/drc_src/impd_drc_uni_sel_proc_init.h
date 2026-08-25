/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_UNI_SEL_PROC_INIT_H
#define IMPD_DRC_UNI_SEL_PROC_INIT_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

WORD32
impd_drc_sel_proc_init_dflt(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc);

WORD32
impd_drc_sel_proc_init_sel_proc_params(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct);

WORD32
impd_drc_sel_proc_init_interface_params(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_interface_struct* pstr_drc_interface);

#ifdef __cplusplus
}
#endif /* __cplusplus */

#endif
