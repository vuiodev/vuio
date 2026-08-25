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
.global ixheaacd_conv_ergtoamplitudelp_armv7
ixheaacd_conv_ergtoamplitudelp_armv7:

    STMFD           sp!, {r4-r12, r14}

    LDR             R5, [SP, #44]
    LDR             R4, [SP, #40]
    MOVW            R11, #0x5A82
    MOVW            R10, #0x1FF

LOOP1:

    LDRSH           R6, [R2, #0]
    MOV             R12, #0
    MOV             R14, #-16
    MOVS            R6, R6
    BLE             ENDIF1_1

    LDRSH           R7, [R2, #2]
    CLZ             R8, R6
    SUB             R8, R8, #17
    SUB             R7, R7, R8
    MOV             R6, R6, LSL R8
    MOV             R6, R6, ASR #5
    AND             R6, R6, R10

    TST             R7, #1
    BIC             R6, R6, #1
    LDRH            R12, [R6, R5]
    ADDNE           R7, R7, #3
    MOV             R14, R7, ASR #1
    SMULWBNE        R12, R12, R11

ENDIF1_1:
    STRH            R14, [R2, #2]


    LDRSH           R6, [R3, #0]
    MOV             R8, #0
    MOV             R9, #-16

    MOVS            R6, R6
    BLE             ENDIF1_2

    LDRSH           R7, [R3, #2]
    CLZ             R8, R6
    SUB             R8, R8, #17
    SUB             R7, R7, R8
    MOV             R6, R6, LSL R8
    MOV             R6, R6, ASR #5
    AND             R6, R6, R10

    TST             R7, #1
    BIC             R6, R6, #1
    LDRH            R8, [R6, R5]
    ADDNE           R7, R7, #3
    MOV             R9, R7, ASR #1
    SMULWBNE        R8, R8, R11

ENDIF1_2:
    STRH            R9, [R3, #2]
    STRH            R8, [R3], #4




    LDRSH           R6, [R4, #0]
    MOV             R8, #0
    MOV             R9, #-16
    MOVS            R6, R6
    BLE             ENDIF1_3

    LDRSH           R7, [R4, #2]
    CLZ             R8, R6
    SUB             R8, R8, #17
    SUB             R7, R7, R8
    MOV             R6, R6, LSL R8
    MOV             R6, R6, ASR #5
    ANDS            R6, R6, R10

    TST             R7, #1
    BIC             R6, R6, #1
    LDRH            R8, [R6, R5]
    ADDNE           R7, R7, #3
    MOV             R9, R7, ASR #1
    SMULWBNE        R8, R8, R11

ENDIF1_3:
    STRH            R9, [R4, #2]

    SUB             R6, R1, R9
    SUBS            R6, R6, #4

    RSBLE           R6, R6, #0
    MOVGT           R8, R8, ASR R6
    MOVLE           R8, R8, LSL R6
    STRH            R8, [R4], #4


    SUBS            R6, R14, R1
    BLE             ELSE1

    CMP             R6, #15
    MOVGT           R6, #15
    MOV             R12, R12, LSL R6
    CMP             R12, #0x8000
    MVNGE           R12, #0x8000
    CMNLT           R12, #0x00008000
    MOVLT           R12, #0x00008000
    STRH            R12, [R2], #4
    SUBS            R0, R0, #1
    BGT             LOOP1

ELSE1:
    RSB             R6, R6, #0
    MOV             R12, R12, ASR R6
    STRH            R12, [R2], #4
    SUBS            R0, R0, #1
    BGT             LOOP1
    LDMFD           sp!, {r4-r12, r15}





