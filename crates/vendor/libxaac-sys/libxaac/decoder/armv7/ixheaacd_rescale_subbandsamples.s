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
    .global ixheaacd_adjust_scale_armv7
ixheaacd_adjust_scale_armv7:
    STMFD           SP!, {R4-R11, R14}
    LDR             R4, [SP, #44]
    LDR             R5, [SP, #36]
    LDR             R6, [SP, #40]
    MOVS            R4, R4
    BEQ             ENDRESSCALE
    SUBS            R3, R3, R2
    BLE             ENDRESSCALE
    SUBS            R6, R6, R5
    BLE             ENDRESSCALE

    ADD             R9, R0, R5, LSL#2
    LDR             R10, [R9], #4

    CMP             R4, #31
    MOVGT           R4, #31
    CMP             R4, #-31
    MOVLT           R4, #-31


    LDR             R8, [SP, #48]
    MOVS            R8, R8
    BEQ             ELIF1


    MOVS            R4, R4
    BLE             ELIF2_1

LOOP1:
    ADD             R10, R10, R2, LSL #2
    MOV             R7, R3

INNLOOP1:
    LDR             R11, [R10]
    SUBS            R7, R7  , #2
    LDRGE           R5, [R10, #4]

    MOV             R11, R11, LSL R4
    STR             R11, [R10], #4

    MOVGE           R5, R5, LSL R4
    STRGE           R5, [R10], #4

    BGT             INNLOOP1

    LDR             R10, [R9], #4
    SUBS            R6, R6, #1
    BGT             LOOP1

    B               ENDRESSCALE

ELIF2_1:
    RSB             R4, R4, #0

LOOP2:
    ADD             R10, R10, R2, LSL #2
    MOV             R7, R3
INNLOOP2:
    LDR             R11, [R10]
    SUBS            R7, R7  , #2
    LDRGE           R5, [R10, #4]

    MOV             R11, R11, ASR R4
    STR             R11, [R10], #4

    MOVGE           R5, R5, ASR R4
    STRGE           R5, [R10], #4

    BGT             INNLOOP2

    LDR             R10, [R9], #4
    SUBS            R6, R6, #1
    BGT             LOOP2




    B               ENDRESSCALE

ELIF1:
    ADD             R5, R1, R5, LSL#2

    MOVS            R4, R4
    BLE             ELIF2_2
LOOP3:
    LDR             R8, [R5], #4
    ADD             R10, R10, R2, LSL #2
    ADD             R8, R8, R2, LSL #2
    BICS            R7, R3, #1
    BEQ             COUNTODD1
INNLOOP3:
    LDR             R11, [R10]
    LDR             R1, [R8]
    MOV             R11, R11, LSL R4
    MOV             R1, R1, LSL R4
    STR             R11, [R10], #4
    STR             R1, [R8], #4

    LDR             R11, [R10]
    LDR             R1, [R8]
    MOV             R11, R11, LSL R4
    MOV             R1, R1, LSL R4
    STR             R11, [R10], #4
    STR             R1, [R8], #4

    SUBS            R7, R7  , #2
    BGT             INNLOOP3
COUNTODD1:
    BIC             R7, R3, #1
    CMP             R7, R3
    BEQ             INNLOOP3END

    LDR             R11, [R10]
    LDR             R1, [R8]
    MOV             R11, R11, LSL R4
    MOV             R1, R1, LSL R4
    STR             R11, [R10], #4
    STR             R1, [R8], #4



INNLOOP3END:


    LDR             R10, [R9], #4
    SUBS            R6, R6, #1
    BGT             LOOP3
    B               ENDRESSCALE

ELIF2_2:
    RSB             R4, R4, #0

LOOP4:
    LDR             R8, [R5], #4
    ADD             R10, R10, R2, LSL #2
    ADD             R8, R8, R2, LSL #2
    BICS            R7, R3, #1
    BEQ             COUNTODD2
INNLOOP4:
    LDR             R11, [R10]
    LDR             R1, [R8]
    MOV             R11, R11, ASR R4
    MOV             R1, R1, ASR R4
    STR             R11, [R10], #4
    STR             R1, [R8], #4

    LDR             R11, [R10]
    LDR             R1, [R8]
    MOV             R11, R11, ASR R4
    MOV             R1, R1, ASR R4
    STR             R11, [R10], #4
    STR             R1, [R8], #4


    SUBS            R7, R7  , #2
    BGT             INNLOOP4
COUNTODD2:
    BIC             R7, R3, #1
    CMP             R7, R3
    BEQ             INNLOOP4END

    LDR             R11, [R10]
    LDR             R1, [R8]
    MOV             R11, R11, ASR R4
    MOV             R1, R1, ASR R4
    STR             R11, [R10], #4
    STR             R1, [R8], #4


INNLOOP4END:
    LDR             R10, [R9], #4
    SUBS            R6, R6, #1
    BGT             LOOP4


ENDRESSCALE:
    LDMFD           sp!, {r4-r11, r15}



