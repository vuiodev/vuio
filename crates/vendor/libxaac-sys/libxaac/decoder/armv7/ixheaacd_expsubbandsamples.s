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
.global ixheaacd_expsubbandsamples_armv7
ixheaacd_expsubbandsamples_armv7:

    STMFD           sp!, {r4-r12}

    LDR             r7, [sp, #0x24]
    LDR             r10, [sp, #0x28]
    SUB             r11, r3, r2
    MOV             r12, #1
    CMP             r7, r10
    BGE             EXIT

    MOVS            r3, r11
    BEQ             EXIT


    LDR             r4, [sp, #0x2c]
    CMP             r4, #0
    BEQ             HQ_OUTER_LOOP



    SUB             r10, r10, r7
    ADD             r0, r0, r7, LSL #2
    LDR             r1, [r0], #4

OUTERLOOP:
    MOV             r3, r11
    ADD             r5, r1, r2, LSL #2
INLOOP:

    LDR             r4, [r5], #4
    SUBS            r3, r3, #2
    LDRGE           r8, [r5], #4
    EOR             r1 , r4 , r4, asr #31
    ORR             r12, r12, r1
    EORGE           r1 , r8 , r8, asr #31
    ORRGE           r12, r12, r1
    BGT             INLOOP

    SUBS            r10, r10, #1
    LDR             r1, [r0], #4
    BGT             OUTERLOOP



    B               EXIT


HQ_OUTER_LOOP:
    LDR             r6, [r0, r7, LSL #2]
    LDR             r5, [r1, r7, LSL #2]
    ADD             r6, r6, r2, LSL #2
    ADD             r5, r5, r2, LSL #2
    MOV             r4, r11

HQ_IN_LOOP:
    LDR             r8, [r6], #4
    LDR             r9, [r5], #4
    SUBS            r4, r4, #2

    EOR             r3 , r8 , r8, asr #31
    ORR             r12, r12, r3

    EOR             r3 , r9 , r9, asr #31
    ORR             r12, r12, r3

    LDRGE           r8, [r6], #4
    LDRGE           r9, [r5], #4

    EORGE           r3 , r8 , r8, asr #31
    ORRGE           r12, r12, r3

    EORGE           r3 , r9 , r9, asr #31
    ORRGE           r12, r12, r3

    BGT             HQ_IN_LOOP

INLOEN:
    ADD             r7, r7, #1
    CMP             r7, r10
    BLT             HQ_OUTER_LOOP

EXIT:

    CLZ             r0, r12
    SUB             r0, r0, #1
    LDMFD           sp!, {r4-r12}
    BX              lr

