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
    .global ixheaacd_harm_idx_zerotwolp_armv7
ixheaacd_harm_idx_zerotwolp_armv7:
    STMFD           sp!, {r4-r12}
    SUB             r5, r2, #1
    MOV             r2, #-1
    LDR             r6, [sp, #52]
    LDR             r12, [sp, #48]
    ADD             r10, sp, #36
    LDR             r4, [sp, #44]
    LDMIA           r10, {r9, r10}
    CMP             r4, #0
    BLE             EXIT
    CMP             r12, #0
    BNE             NO_NOISE



LOOP1:
    LDR             r12, [r0, #0]
    LDRSH           r7, [r1], #2
    LDRSH           r8, [r1], #2

    ADD             r2, r2, #1
    SMULWB          r7, r12, r7
    SUBS            r8, r8, r5

    LDRH            r12, [r3], #4
    RSBLE           r8, r8, #0
    MOVLE           r8, r7, ASR r8
    MOVGT           r8, r7, LSL r8

    MOVS            r12, r12, LSL #16
    BEQ             NEXT

    CMP             r6, #0
    QADDEQ          r8, r8, r12
    QSUBNE          r8, r8, r12
    SUBS            r4, r4, #1
    B               STORE

NEXT:


    LDR             r7, [r9, r2, LSL #2]
    ADD             r12, r10, r2, LSL #2
    LDRSH           r12, [r12, #0]
    SUBS            r4, r4, #1
    SMULTB          r7, r7, r12
    ADD             r8, r8, r7, LSL #1

STORE:
    STR             r8, [r0], #4
    BGT             LOOP1
    B               EXIT


NO_NOISE:

LOOP2:
    LDR             r12, [r0, #0]
    LDRSH           r7, [r1], #2
    LDRSH           r9, [r1], #2
    LDRH            r10, [r3], #4

    SMULWB          r7, r12, r7
    SUBS            r9, r9, r5
    RSBMI           r9, r9, #0

    MOVMI           r12, r7, ASR r9
    MOVPL           r12, r7, LSL r9

    MOV             r7, r10, LSL #16

    CMP             r6, #0
    QADDEQ          r12, r12, r7
    QSUBNE          r12, r12, r7

    SUBS            r4, r4, #1
    STR             r12, [r0], #4
    BGT             LOOP2

EXIT:
    LDMFD           sp!, {r4-r12}
    BX              lr


