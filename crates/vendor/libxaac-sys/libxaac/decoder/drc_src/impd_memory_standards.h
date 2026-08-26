/******************************************************************************
 *
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

#ifndef IMPD_MEMORY_STANDARDS_H
#define IMPD_MEMORY_STANDARDS_H

/*****************************************************************************/
/* Type definitions                                                          */
/*****************************************************************************/
/* standard memory table descriptor for libraries */
typedef struct {
  UWORD32 ui_size;         /* size of the memory in bytes  */
  UWORD32 ui_alignment;    /* alignment in bytes           */
  UWORD32 ui_type;         /* type of memory               */
  UWORD32 ui_placement[2]; /* 64 bit placement info        */
  UWORD32 ui_priority;     /* the importance for placement */
  UWORD32 ui_placed[2];    /* the o_red location for placement */
} ia_mem_info_struct;

/*****************************************************************************/
/* Constant hash defines                                                     */
/*****************************************************************************/
/* when you don't need alignment, pass this to memory library */
#define IA_MEM_NO_ALIGN 0x01

/* ittiam standard memory types */
/* to be used inter frames */
#define IA_MEMTYPE_PERSIST 0x00
/* read write, to be used intra frames */
#define IA_MEMTYPE_SCRATCH 0x01
/* read only memory, intra frame */
#define IA_MEMTYPE_INPUT 0x02
/* read-write memory, for usable output, intra frame */
#define IA_MEMTYPE_OUTPUT 0x03
/* readonly memory, inter frame */
#define IA_MEMTYPE_TABLE 0x04
/* input buffer before mem tabs allocation */
#define IA_MEMTYPE_PRE_FRAME_INPUT 0x05
/* input buffer before mem tabs allocation */
#define IA_MEMTYPE_PRE_FRAME_SCRATCH 0x06
/* for local variables */
#define IA_MEMTYPE_AUTO_VAR 0x80

/* ittiam standard memory priorities */
#define IA_MEMPRIORITY_ANYWHERE 0x00
#define IA_MEMPRIORITY_LOWEST 0x01
#define IA_MEMPRIORITY_LOW 0x02
#define IA_MEMPRIORITY_NORM 0x03
#define IA_MEMPRIORITY_ABOVE_NORM 0x04
#define IA_MEMPRIORITY_HIGH 0x05
#define IA_MEMPRIORITY_HIGHER 0x06
#define IA_MEMPRIORITY_CRITICAL 0x07

/* ittiam standard memory placements */
/* placement is defined by 64 bits */

#define IA_MEMPLACE_FAST_RAM_0 0x000001
#define IA_MEMPLACE_FAST_RAM_1 0x000002
#define IA_MEMPLACE_FAST_RAM_2 0x000004
#define IA_MEMPLACE_FAST_RAM_3 0x000008
#define IA_MEMPLACE_FAST_RAM_4 0x000010
#define IA_MEMPLACE_FAST_RAM_5 0x000020
#define IA_MEMPLACE_FAST_RAM_6 0x000040
#define IA_MEMPLACE_FAST_RAM_7 0x000080

#define IA_MEMPLACE_INT_RAM_0 0x000100
#define IA_MEMPLACE_INT_RAM_1 0x000200
#define IA_MEMPLACE_INT_RAM_2 0x000400
#define IA_MEMPLACE_INT_RAM_3 0x000800
#define IA_MEMPLACE_INT_RAM_4 0x001000
#define IA_MEMPLACE_INT_RAM_5 0x002000
#define IA_MEMPLACE_INT_RAM_6 0x004000
#define IA_MEMPLACE_INT_RAM_7 0x008000

#define IA_MEMPLACE_EXT_RAM_0 0x010000
#define IA_MEMPLACE_EXT_RAM_1 0x020000
#define IA_MEMPLACE_EXT_RAM_2 0x040000
#define IA_MEMPLACE_EXT_RAM_3 0x080000
#define IA_MEMPLACE_EXT_RAM_4 0x100000
#define IA_MEMPLACE_EXT_RAM_5 0x200000
#define IA_MEMPLACE_EXT_RAM_6 0x400000
#define IA_MEMPLACE_EXT_RAM_7 0x800000

#define IA_MEMPLACE_DONTCARE_H 0xFFFFFFFF
#define IA_MEMPLACE_DONTCARE_L 0xFFFFFFFF

/* the simple common PC RAM */
#define IA_PC_RAM_H 0x00000000
#define IA_PC_RAM_L IA_MEMPLACE_EXT_RAM_0

#endif
