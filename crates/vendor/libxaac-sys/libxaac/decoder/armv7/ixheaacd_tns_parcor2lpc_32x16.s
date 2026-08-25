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
    .global ixheaacd_tns_parcor_lpc_convert_armv7
ixheaacd_tns_parcor_lpc_convert_armv7:
    STMFD           SP!, {R2, R4-R12, R14}
    SUB             SP, SP, #128
    MOV             R4, SP
    MOV             R8, #0
    MOV             R5, #0x8000

OUTLOOP:
    MOV             R6, #0
    MOV             R7, #16
LOOP1:
    STR             R6, [R4], #4
    STR             R6, [R4, #60]
    SUBS            R7, R7, #1
    BGT             LOOP1

    SUB             R4, R4, #64
    MOV             R9, #0x7FFFFFFF
    MOV             R10, R9, ASR R8


    MOV             R7, R3
LOOP2:
    MOV             R11, R10
    LDRSH           R2, [R4], #2
    LDRSH           R14, [R0], #2
    MOV             R12, R3

LOOP2_1:
    SMULBB          R2, R2, R14
    QADD            R14, R10, R5
    CMP             R2, #0x40000000
    MOV             R14, R14, ASR #16
    MOVNE           R2, R2, LSL #1
    MOVEQ           R2, #0x7FFFFFFF
    QADD            R10, R10, R2
    STRH            R14, [R4, #62]
    MOVS            R2, R10
    RSBSMI          R2, R2, #0
    MOVMI           R2, #0x7FFFFFFF
    CMP             R2, #0x7FFFFFFF
    MOVEQ           R6, #1
    SUBS            R12, R12, #1
@   LDRGTSH     R2, [R4], #2
@   LDRGTSH     R14, [R0], #2
    LDRSHGT         R2, [R4], #2
    LDRSHGT         R14, [R0], #2
    BGT             LOOP2_1

    LDRSH           R2, [R4, #62]
    MOV             R12, R3
LOOP2_2:
    LDRSH           R14, [R0, #-2]!
    LDRSH           R9, [R4, #-2]!
    SMULBB          R2, R2, R14
    MOV             R9, R9, LSL #16
    CMP             R2, #0x40000000
    MOVNE           R2, R2, LSL #1
    MOVEQ           R2, #0x7FFFFFFF
    QADD            R9, R9, R2
    LDRSH           R2, [R4, #62]
    QADD            R14, R9, R5
    MOVS            R9, R9
    MOV             R14, R14, ASR #16
    STRH            R14, [R4, #2]
@   RSBMIS      R9, R9, #0
    RSBSMI          R9, R9, #0
    MOVMI           R9, #0x7FFFFFFF
    CMP             R9, #0x7FFFFFFF
    MOVEQ           R6, #1
    SUBS            R12, R12, #1
    BGT             LOOP2_2

    QADD            R11, R11, R5
    QADD            R2, R10, R5
    MOV             R11, R11, ASR #16
    MOV             R2, R2, ASR #16
    STRH            R11, [R4]
    STRH            R2, [R1], #2
    MOV             R10, #0

    SUBS            R7, R7, #1
    BGE             LOOP2

    SUB             R1, R1, R3, LSL #1
    SUB             R1, R1, #2
    SUBS            R10, R6, #1
    ADDEQ           R8, R8, #1
    BEQ             OUTLOOP

    LDR             R2, [SP, #128]
    ADD             SP, SP, #132
    STRH            R8, [R2]
    LDMFD           sp!, {r4-r12, r15}




