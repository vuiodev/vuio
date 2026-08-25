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
.global ixheaacd_enery_calc_per_subband_armv7

ixheaacd_enery_calc_per_subband_armv7:
    STMFD           sp!, {r4-r12, r14}

    LDR             r10, [sp, #0x34]
    MOV             R4, R2
    MOV             R5, R3
    MOV             R2, R0
    MOV             R3, R1
    SUB             R12, R3, R2
    LDR             r10, [r10, #0]
    ADD             r10, r10, r12, LSL #1
    LDRSH           r9, [r10, #0x20]
    LDR             R1, [sp, #0x28]
    MOV             R1, R1, LSL #1

    SUBS            R5, R5, R4
    LDR             R0, [sp, #0x38]
    LDR             R7, [sp, #0x2C]


    LDR             R8, [sp, #0x30]

    BLE             ENDCALC


    MOVS            R8, R8
    BEQ             HQ_PART


    ADD             R0, R0, R4, LSL #2
    ADD             R0, R0, R2, LSL #8
    SUB             R2, R3, R2
    MOV             R10, #20


    B               LP_SBR_LOOP

HQ_PART:
    ADD             R0, R0, R4, LSL #2
    ADD             R0, R0, R2, LSL #9
    SUB             R2, R3, R2
    MOV             R2, R2, LSL #1
    MOV             R10, #21
    SUB             R1, R1, #1



LP_SBR_LOOP:

    MOV             R6, #0
    MOV             R8, R0
    MOVS            R11, R2
    BLE             STORE_ZEROES
    MOV             R6, #1

LOOP1_CALC_MAX:
    LDR             R4, [R8], #0x100
    LDR             R12, [R8], #0x100
    EOR             R4, R4, R4, ASR #31
    ORR             R6, R6, R4
    EOR             R12, R12, R12, ASR #31
    SUBS            R11, R11, #2
    ORRGE           R6, R6, R12
    BGT             LOOP1_CALC_MAX

CALC_NORM:

    CLZ             R6, R6
    RSBS            R14, R6, R10
    MOV             R6, #0
    MOV             R8, R0
    MOV             R11, R2
    BLE             NEG_SHIFT

LOOP2_APPLY_POS_SHIFT:
    LDR             R4, [R8], #0x100
    LDR             R12, [R8], #0x100
    SUBS            R11, R11, #2
    MOV             R4, R4, ASR R14
    SMLABB          R6, R4, R4, R6
    MOV             R12, R12, ASR R14
    SMLABB          R6, R12, R12, R6
    BGT             LOOP2_APPLY_POS_SHIFT

    B               CONVERT_TO_MANT_EXP

NEG_SHIFT:
    RSB             R12, R14, #0

LOOP2_APPLY_NEG_SHIFT:
    LDR             R4, [R8], #0x100
    LDR             R3, [R8], #0x100
    SUBS            R11, R11, #2
    MOV             R4, R4, LSL R12
    SMLABB          R6, R4, R4, R6
    MOV             R3, R3, LSL R12
    SMLABB          R6, R3, R3, R6
    BGT             LOOP2_APPLY_NEG_SHIFT


CONVERT_TO_MANT_EXP:
    SUB             R14, R14, #23
    ADD             R0, R0, #4
    MOVS            R6, R6
    BEQ             STORE_ZEROES

    CLZ             R12, R6
    RSB             R12, R12, #17
    MOV             R4, R6, ASR  R12

    SMULBB          R11, R4, R9
    ADD             R12, R12, R14, LSL#1

    MOV             R11, R11, ASR #15
    CMP             R11, #0x00008000
    MVNEQ           R11, R11
    STRH            R11, [R7], #2
    ADD             R11, R1, R12
    STRH            R11, [R7], #2
    SUBS            R5, R5, #1
    BGT             LP_SBR_LOOP

    B               ENDCALC

STORE_ZEROES:
    STR             R6, [R7], #4
    SUBS            R5, R5, #1
    BGT             LP_SBR_LOOP

ENDCALC:

    LDMFD           sp!, {r4-r12, r15}

