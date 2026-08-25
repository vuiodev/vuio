@/******************************************************************************
@ *
@ * Copyright (C) 2018 The Android Open Source Project
@ *
@ * Licensed under the Apache License, Version 2.0 (the "License");
@ * you may not use this file except in compliance with the License.
@ * You may obtain a copy of the License at:
@ *
@ * http://www.apache.org/licenses/LICENSE-2.0
@ *
@ * Unless required by applicable law or agreed to in writing, software
@ * distributed under the License is distributed on an "AS IS" BASIS,
@ * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
@ * See the License for the specific language governing permissions and
@ * limitations under the License.
@ *
@ *****************************************************************************
@ * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
@*/


.text
.p2align 2

    .global ixheaacd_overlap_out_copy_armv7
    .global ixheaacd_spec_to_overlapbuf_armv7
    .global ixheaacd_overlap_buf_out_armv7

ixheaacd_overlap_buf_out_armv7:

    STMFD           sp!, {r4-r5}
    MOV             r3, r3, lsl #2

OUTSAMPLE_LOOP:

    LDR             r4, [r1], #4
    LDR             r5, [r1], #4
    SUBS            r2, r2, #2

    LSL             r4,r4,#15
    LSL             r5,r5,#15

    STR            r4, [r0], r3
    STR            r5, [r0], r3

    BGT             OUTSAMPLE_LOOP

    LDMFD           sp!, {r4-r5}
    BX              lr


ixheaacd_overlap_out_copy_armv7:

    STMFD           sp!, {r4-r9, r14}
    MOV             r9, #32
    MOV             r8, r1
    MOV             r3, r3, LSL #2


OUT_OVERLAP_LOOP:
    LDR             r4, [r1], #4
    LDR             r5, [r1], #4
    SUBS            r9, r9, #1

    LSL             r4,r4,#15
    LSL             r5,r5,#15

    LDR             r6, [r2], #4
    LDR             r7, [r2], #4

    STR            r4, [r0], r3
    STR            r5, [r0], r3

    STR             r6, [r8], #4
    STR             r7, [r8], #4

    BGT             OUT_OVERLAP_LOOP

    LDMFD           sp!, {r4-r9, r15}




ixheaacd_spec_to_overlapbuf_armv7:

    STMFD           sp!, {r4-r10, r14}

    MOV             r6, #1
    RSB             r2, r2, #16
    AND             r2, r2, #0xFF
    SUB             r7, r2, #1
    LSL             r14, r6, r7
    MOV             r3, r3, ASR #1

OVERLAP_LOOP1:


    LDMIA           r1!, {r4-r5}
    SUBS            r3, r3, #1

    QADD            r4, r4, r14
    QADD            r5, r5, r14

    MOV             r4, r4, ASR r2
    MOV             r5, r5, ASR r2

    STR             r4, [r0], #4
    STR             r5, [r0], #4


    BGT             OVERLAP_LOOP1
    LDMFD           sp!, {r4-r10, pc}

