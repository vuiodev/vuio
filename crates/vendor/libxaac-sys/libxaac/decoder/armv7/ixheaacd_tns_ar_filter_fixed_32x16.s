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
        .global ixheaacd_tns_ar_filter_armv7

ixheaacd_tns_ar_filter_armv7:

    STMFD           r13! , {r4 - r12, r14}
    SUB             sp, sp, #4
    LDR             r4, [sp, #44]
    LDR             r6, [sp, #48]
    STR             r1, [sp]
    LDR             r12, [sp, #56]
    ANDS            r5, r4, #3
    ADD             r12, r12, #4096
    BEQ             FILTER_LOOP



    MOV             r8, #0
    ADD             r14, r3, r4, LSL #1
    RSBS            r7, r5, #3
    BEQ             ORDER_LOOPEND
ORDER_LOOP:
    STRH            r8, [r14, #2]!
    SUBS            r7, r7, #1
    BGT             ORDER_LOOP
ORDER_LOOPEND:
    STRH            r8, [r14, #2]
    BIC             r4, r4, #3
    ADD             r4, r4, #4


FILTER_LOOP:
    LDR             r1, [sp, #52]




    CMP             r2, #1
    MOV             r7, r4
    BNE             NEG_INC

    LDR             r8   , [r0]
    SUBS            r7   , r7   , #1
    MOV             r8, r8, lsl r1
    MOV             r9, r8, asr r1
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12], #-4
    STR             r9, [r0], #4

    BEQ             FILTER_LOOP2
FILTER_LOOP1:
    LDR             r8   , [r0]
    SUB             r5   , r4  , r7
    MOV             r5   , r5  , lsl #1
    MOV             r11  , #0
    ADD             r14, r12, r5, lsl #1
INNER_LOOP1:
    LDRSH           r9   , [r3  , r5]
    LDR             r10  , [r14], #-4
    SUBS            r5   , r5   , #2
    SMLAWB          r11  , r10, r9, r11
    BGT             INNER_LOOP1

    MOV             r8, r8, lsl r1
    SUB             r8    , r8    , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #4
    SUBS            r7   , r7   , #1
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12], #-4
    BGT             FILTER_LOOP1

FILTER_LOOP2:
    LDR             r1, [sp]
    SUBS            r7   , r1  , r4
    BLE             EXIT

    LDR             r1, [sp, #52]




    CMP             r6, #1
    BEQ             SHIFT_1

OUTER_LOOP2:
    LDR             r8   , [r0]
    MOV             r5   , r4  , lsl #1
    MOV             r11  , #0
    LDR             r9   , [r3  , r5]
    ADD             r14  , r12, r5, lsl #1
    SUB             r5   , r5   , #4
INNER_LOOP2:
    LDR             r10   , [r14], #-4
    LDR             r2  , [r14] , #-4

    SMLAWB          r11, r10  , r9, r11
    LDR             r9   , [r3  , r5]
    SUB             r5   , r5   , #4

    SMLAWT          r11, r2  , r9, r11
    LDR             r10  , [r14]  , #-4
    LDR             r2  , [r14] , #-4

    SMLAWB          r11, r10  , r9, r11
    LDR             r9   , [r3  , r5]
    SUBS            r5   , r5   , #4

    SMLAWT          r11, r2  , r9, r11

    BGT             INNER_LOOP2

    MOV             r8, r8, lsl r1
    SUB             r8   , r8   , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #4
    MOV             r2   , r8   , lsl r6
    STR             r2   , [r12], #-4
    SUBS            r7   , r7   , #1
    BGT             OUTER_LOOP2
    B               EXIT

SHIFT_1:
    MOV             r6, r3

OUTER_LOOP2_SHIFT_1:
    ADD             r3, r6, r4  , lsl #1
    LDR             r9   , [r3  ], #-4

    LDR             r8   , [r0]
    ADD             r14  , r12, r4, lsl #2
    MOV             r5   , r4
    MOV             r11  , #0

INNER_LOOP2_SHIFT_1:
    LDR             r10   , [r14] , #-4
    LDR             r2    , [r14] , #-4
    SMLAWB          r11  , r10  , r9, r11

    LDR             r9   , [r3]  , #-4
    LDR             r10  , [r14]  , #-4
    SMLAWT          r11, r2  , r9, r11

    LDR             r2    , [r14] , #-4
    SMLAWB          r11, r10  , r9, r11
    LDR             r9   , [r3  ], #-4

    SUBS            r5   , r5   , #4
    SMLAWT          r11, r2  , r9, r11

    BGT             INNER_LOOP2_SHIFT_1

    MOV             r8, r8, lsl r1
    SUB             r8   , r8   , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #4
    MOV             r2   , r8   , lsl #1
    STR             r2   , [r12], #-4
    SUBS            r7   , r7   , #1


    BGT             OUTER_LOOP2_SHIFT_1


    B               EXIT


NEG_INC:

    LDR             r8   , [r0]
    SUBS            r7   , r7   , #1
    MOV             r8, r8, lsl r1
    MOV             r9, r8, asr r1
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12], #-4
    STR             r9, [r0], #-4

    BEQ             FILTER_LOOP2_NEG
FILTER_LOOP1_NEG:
    LDR             r8   , [r0]
    SUB             r5   , r4  , r7
    MOV             r5   , r5  , lsl #1
    MOV             r11  , #0
    ADD             r14, r12, r5, lsl #1
INNER_LOOP1_NEG:
    LDRSH           r9   , [r3  , r5]
    LDR             r10  , [r14], #-4
    SUBS            r5   , r5   , #2
    SMLAWB          r11  , r10, r9, r11
    BGT             INNER_LOOP1_NEG

    MOV             r8, r8, lsl r1
    SUB             r8    , r8    , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #-4
    SUBS            r7   , r7   , #1
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12], #-4
    BGT             FILTER_LOOP1_NEG

FILTER_LOOP2_NEG:
    LDR             r1, [sp]
    SUBS            r7   , r1  , r4
    BLE             EXIT

    LDR             r1, [sp, #52]




OUTER_LOOP2_NEG:
    LDR             r8   , [r0]
    MOV             r5   , r4  , lsl #1
    MOV             r11  , #0
    LDR             r9   , [r3  , r5]
    ADD             r14  , r12, r5, lsl #1
    SUB             r5   , r5   , #4
INNER_LOOP2_NEG:
    LDR             r10   , [r14], #-4
    LDR             r2  , [r14] , #-4

    SMLAWB          r11, r10  , r9, r11
    LDR             r9   , [r3  , r5]
    SUB             r5   , r5   , #4

    SMLAWT          r11, r2  , r9, r11
    LDR             r10  , [r14]  , #-4
    LDR             r2  , [r14] , #-4

    SMLAWB          r11, r10  , r9, r11
    LDR             r9   , [r3  , r5]
    SUBS            r5   , r5   , #4

    SMLAWT          r11, r2  , r9, r11

    BGT             INNER_LOOP2_NEG

    MOV             r8, r8, lsl r1
    SUB             r8   , r8   , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #-4
    MOV             r2   , r8   , lsl r6
    STR             r2   , [r12], #-4
    SUBS            r7   , r7   , #1
    BGT             OUTER_LOOP2_NEG

EXIT:
    ADD             sp, sp , #4
    LDMFD           r13!, {r4 - r12, r15}

