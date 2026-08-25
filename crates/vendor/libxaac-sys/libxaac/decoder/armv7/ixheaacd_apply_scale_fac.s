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
.global ixheaacd_scale_factor_process_armv7

ixheaacd_scale_factor_process_armv7:




    STMFD           sp!, {r4-r12, r14}
    LDR             r9, [sp, #0x28]
    LDR             r11, [sp, #0x2c]

    CMP             r2, #0

    BLE             END
    MOV             r10, #0
    CMP             r11, #2
    MOVLE           r11, #0x25
    MOVGT           r11, #0x22

TBANDS_LOOP:

    LDRSH           r5, [r1], #2
    LDRB            r4, [r3], #1

    LDR             r6, [sp, #0x30]
    LDR             r7, [sp, #0x34]

    CMP             r5, #0x18
    BGE             SCALE_FACTOR_GE_12

    CMP             r4, #0
    BLE             OFFSET_ZERO

SCALE_FACTOR_LT_12:

    STR             r10, [r0], #4
    STR             r10, [r0], #4
    STR             r10, [r0], #4
    STR             r10, [r0], #4
    SUBS            r4, r4, #4
    BGT             SCALE_FACTOR_LT_12
    B               OFFSET_ZERO

SCALE_FACTOR_GE_12:

    SUBS            r6, r11, r5, ASR #2


    AND             r5, r5, #3


    LDR             r5, [r9, r5, LSL #2]

    BLE             SHIFT_LE_ZERO

    SUB             r14, r6, #1

SHIFT_POSITIVE:
    LDRD            r6, [r0, #0]

    SMULWB          r6, r6, r5
    SMULWB          r7, r7, r5

    MOV             r6, r6, ASR r14
    MOV             r7, r7, ASR r14

    STRD            r6, [r0], #8

    LDRD            r6, [r0, #0]

    SMULWB          r6, r6, r5
    SMULWB          r7, r7, r5
    SUBS            r4, r4, #4

    MOV             r6, r6, ASR r14
    MOV             r7, r7, ASR r14

    STRD            r6, [r0], #8

    BGT             SHIFT_POSITIVE
    B               OFFSET_ZERO
SHIFT_LE_ZERO:

    RSBS            r14, r6, #0
    BGT             SHIFT_NEGTIVE1

SHIFT_ZERO:
    LDRD            r6, [r0, #0]

    SMULWB          r6, r6, r5
    SMULWB          r7, r7, r5
    MOV             r6, r6, LSL #1
    MOV             r7, r7, LSL #1

    STRD            r6, [r0], #8

    SUBS            r4, r4, #2

    BGT             SHIFT_ZERO
    B               OFFSET_ZERO

SHIFT_NEGTIVE1:
    SUB             r14, r14, #1
SHIFT_NEGTIVE:
    LDRD            r6, [r0, #0]
    MOV             r6, r6, LSL r14
    MOV             r7, r7, LSL r14

    SMULWB          r6, r6, r5
    SMULWB          r7, r7, r5
    MOV             r6, r6, LSL #2
    MOV             r7, r7, LSL #2

    STRD            r6, [r0], #8

    SUBS            r4, r4, #2

    BGT             SHIFT_NEGTIVE

OFFSET_ZERO:
    SUBS            r2, r2, #1
    BGT             TBANDS_LOOP
END:
    LDMFD           sp!, {r4-r12, r15}
