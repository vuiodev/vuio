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
.global ixheaacd_apply_rot_armv7

ixheaacd_apply_rot_armv7:
    STMFD           SP!, {R4-R12, R14}
    MOVW            R5, #0x59e
    MOV             R4, #22
    ADD             R11, R0, R5
LOOP1:
    LDRSH           R5, [R11, #-98]
    LDRSH           R6, [R11, #94]
    LDRSH           R7, [R11, #-96]
    LDRSH           R8, [R11, #96]
    ADD             R9, R5, R6
    STRH            R9, [R11, #-98]
    ADD             R10, R7, R8
    STRH            R10, [R11, #-96]


    LDRSH           R5, [R11, #-2]
    LDRSH           R6, [R11, #190]
    LDRSH           R7, [R11]
    LDRSH           R8, [R11, #192]
    ADD             R9, R5, R6
    STRH            R9, [R11, #-2]
    ADD             R10, R7, R8
    STRH            R10, [R11], #4

    LDRSH           R5, [R11, #-98]
    LDRSH           R6, [R11, #94]
    LDRSH           R7, [R11, #-96]
    LDRSH           R8, [R11, #96]
    ADD             R9, R5, R6
    STRH            R9, [R11, #-98]
    ADD             R10, R7, R8
    STRH            R10, [R11, #-96]

    SUBS            R4, R4, #2

    LDRSH           R5, [R11, #-2]
    LDRSH           R6, [R11, #190]
    LDRSH           R7, [R11]
    LDRSH           R8, [R11, #192]
    ADD             R9, R5, R6
    STRH            R9, [R11, #-2]
    ADD             R10, R7, R8
    STRH            R10, [R11], #4

    BGT             LOOP1

    MOVW            R4, #0x53C
    LDR             R12, [R0, #44]
    ADD             R11, R0, R4
    MOV             R4, #10

LOOP2:

    LDR             R5, [R12]
    LDR             R7, [R11], #4
    LDR             R6, [R12, #0x80]
    LDR             R8, [R11, #92]

    SMULWB          R9, R5, R7
    SMULWB          R10, R6, R8
    SMULWT          R14, R5, R7

    QADD            R5, R9, R10
    SMULWT          R6, R6, R8

    MOV             R5, R5, LSL #2
    QADD            R14, R14, R6
    STR             R5, [R12], #4
    MOV             R14, R14, LSL #2
    STR             R14, [R12, #0x7c]

    LDR             R5, [R12, #0x3c]
    LDR             R6, [R12, #0xbc]

    SMULWB          R9, R5, R7
    SMULWB          R10, R6, R8
    SMULWT          R14, R5, R7

    QADD            R5, R9, R10

    SMULWT          R6, R6, R8

    MOV             R5, R5, LSL #2
    QADD            R14, R14, R6
    STR             R5, [R12, #0x3c]
    MOV             R14, R14, LSL #2
    STR             R14, [R12, #0xbc]

    SUBS            R4, R4, #1

    BGT             LOOP2

    MOVW            R11, #0x6c2
    MOVW            R5, #0x564
    LDRSH           R14, [R0, R11]
    ADD             R11, R0, R5
    LDR             R5, [SP, #44]
    SUB             SP, SP, #512
    MOV             R12, SP
    LDR             R6, [R5, #12]
    MOV             R4, #12
    ADD             R6, R6, #0xb8

LOOP3:
    LDRSH           R5, [R6], #2
    LDRSH           R7, [R6, #-4]
    LDR             R10, [R11, #96]
    LDR             R9, [R11], #4
    CMP             R14, R5
    SUB             R8, R14, R7
    SUBGT           R8, R5, R7
    ADD             R5, R12, R7, LSL #3

LOOP3INN1:
    STR             R10, [R5, #4]
    STR             R9, [R5], #8
    SUBS            R8, R8, #1
    BGT             LOOP3INN1

    SUBS            R4, R4, #1
    BGT             LOOP3

    MOV             R4, #3
    LDR             R12, [R0, #44]
    LDR             R9, [SP, #48+512]
    LDR             R0, [SP, #40+512]
    STR             R14, [SP, #-4]!

LOOP4:
    LDR             R5, [R12], #4
    LDR             R6, [R12, #0x3c]
    LDR             R7, [R12, #0x7c]
    LDRSH           R10, [R9], #2
    LDR             R8, [R12, #0xbc]
    MOV             R11, #5
    CMP             R10, #6
    SUBLT           R11, R10, #1

LOOP4INN1:
    LDR             R10, [R12], #4
    LDR             R14, [R12, #0x3C]
    QADD            R5, R5, R10
    QADD            R6, R6, R14
    LDR             R10, [R12, #0x7C]
    LDR             R14, [R12, #0xBC]
    QADD            R7, R7, R10
    QADD            R8, R8, R14
    SUBS            R11, R11, #1
    BGT             LOOP4INN1

    STR             R5, [R1], #4
    STR             R6, [R2], #4
    STR             R7, [R3], #4
    STR             R8, [R0], #4
    SUBS            R4, R4, #1
    BGT             LOOP4

    LDR             R14, [SP]
    ADD             R11, SP, #28
    SUB             R4, R14, #3

LOOP5:
    LDR             R5, [R1]
    LDR             R7, [R11], #4
    LDR             R6, [R3]
    LDR             R8, [R11], #4

    SMULWB          R9, R5, R7
    SMULWB          R10, R6, R8
    SMULWT          R14, R5, R7

    QADD            R5, R9, R10
    SMULWT          R6, R6, R8

    MOV             R5, R5, LSL #2
    QADD            R14, R14, R6
    STR             R5, [R1], #4
    MOV             R14, R14, LSL #2
    STR             R14, [R3], #4

    SUBS            R4, R4, #1

    LDR             R5, [R2]
    LDR             R6, [R0]

    SMULWB          R9, R5, R7
    SMULWB          R10, R6, R8
    SMULWT          R14, R5, R7

    QADD            R5, R9, R10

    SMULWT          R6, R6, R8

    MOV             R5, R5, LSL #2
    QADD            R14, R14, R6
    STR             R5, [R2], #4
    MOV             R14, R14, LSL #2
    STR             R14, [R0], #4

    BGT             LOOP5
    ADD             SP, SP, #516
    LDMFD           sp!, {r4-r12, r15}

