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
.global ixheaacd_conv_ergtoamplitude_armv7
ixheaacd_conv_ergtoamplitude_armv7:

    STMFD           sp!, {r4-r12, r14}

    LDR             R5, [SP, #44]
    LDR             R4, [SP, #40]
    MOVW            R14, #0x1FF
    MOVW            R10, #0x5A82

LOOP1:

    LDRSH           R6, [R2], #2
    LDRSH           R7, [R2], #2
    MOV             R12, #0
    MOV             R9, #-16
    MOVS            R6, R6
    BLE             ENDIF1_1
    CLZ             R8, R6
    SUB             R8, R8, #17
    SUB             R7, R7, R8
    MOV             R11, R6, LSL R8

    MOV             R11, R11, ASR #5
    ANDS            R11, R11, R14

    BIC             R11, R11, #1
    LDRH            R12, [R11, R5]

    TST             R7, #1
    ADDNE           R7, R7, #3
    SMULWBNE        R12, R12, R10


    MOV             R9, R7, ASR #1
ENDIF1_1:
    STRH            R12, [R2, #-4]
    STRH            R9, [R2, #-2]


    LDRSH           R6, [R3], #2
    LDRSH           R7, [R3], #2
    MOV             R8, #0
    MOV             R9, #-16
    MOVS            R6, R6
    BLE             ENDIF1_2
    CLZ             R8, R6
    SUB             R8, R8, #17
    SUB             R7, R7, R8
    MOV             R11, R6, LSL R8
    MOV             R11, R11, ASR #5
    ANDS            R11, R11, R14

    BIC             R11, R11, #1
    LDRH            R8, [R11, R5]
    TST             R7, #1
    ADDNE           R7, R7, #3

    SMULWBNE        R8, R8, R10


    MOV             R9, R7, ASR #1
ENDIF1_2:
    STRH            R8, [R3, #-4]
    STRH            R9, [R3, #-2]



    LDRSH           R6, [R4], #2
    LDRSH           R7, [R4], #2
    MOV             R8, #0
    MOV             R9, #-16
    MOVS            R6, R6
    BLE             ENDIF1_3
    CLZ             R8, R6
    SUB             R8, R8, #17
    SUB             R7, R7, R8
    MOV             R11, R6, LSL R8
    MOV             R11, R11, ASR #5
    ANDS            R11, R11, R14

    BIC             R11, R11, #1
    LDRH            R8, [R11, R5]
    TST             R7, #1
    ADDNE           R7, R7, #3

    SMULWBNE        R8, R8, R10


    MOV             R9, R7, ASR #1
ENDIF1_3:
    STRH            R9, [R4, #-2]

    SUB             R6, R1, R9
    SUBS            R6, R6, #4

    RSBLE           R6, R6, #0
    MOVGT           R8, R8, ASR R6
    MOVLE           R8, R8, LSL R6
    STRH            R8, [R4, #-4]

    SUBS            R0, R0, #1
    BGT             LOOP1
    LDMFD           sp!, {r4-r12, r15}





