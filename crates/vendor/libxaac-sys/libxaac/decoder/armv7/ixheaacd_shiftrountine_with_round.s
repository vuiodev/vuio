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


        .code 32
       .eabi_attribute 24, 1            @Tag_ABI_align_needed
       .eabi_attribute 25, 1            @Tag_ABI_align_preserved
.text
.p2align 2
    .global ixheaacd_shiftrountine_with_rnd
    .type ixheaacd_shiftrountine_with_rnd, %function
ixheaacd_shiftrountine_with_rnd:
    STMFD           sp!, {r4-r12, r14}
    MOV             r4, #0x1f
    ADD             r12, r2, r3, LSL #1
    MOV             r9, #0x8000
    SUBS            r3, r3, #1
    BMI             S_WITH_R_L6

S_WITH_R_L5:
    LDR             r5, [r1, r3, LSL #2]
    LDR             r7, [r0, r3, LSL #2]
    LDR             r14, [r0], #4
    LDR             r10, [r1], #4

    ADD             r6, r5, r7
    SUB             r5, r5, r7
    ADD             r7, r10, r14
    SUB             r4, r10, r14











    MOVS            r10, r4, ASR #0x15
    CMNLT           r10, #1

    MOVLT           r4, #0x80000000
    MVNGT           r4, #0x80000000
    MOVEQ           r4, r4, LSL #10

    MOVS            r10, r5, ASR #0x15
    QADD            r4, r4, r9
    CMNLT           r10, #1
    MOV             r4, r4, ASR #16
    MOVLT           r5, #0x80000000
    MVNGT           r5, #0x80000000
    MOVEQ           r5, r5, LSL #10
    MOV             r14, r3, lsl #1


    MOVS            r10, r6, ASR #0x15
    QADD            r5, r5, r9
    CMNLT           r10, #1
    MOV             r5, r5, ASR #16
    MOVLT           r6, #0x80000000

    STRH            r5, [r2, r14]
    MVNGT           r6, #0x80000000
    MOVEQ           r6, r6, LSL #10

    MOVS            r10, r7, ASR #0x15
    QADD            r6, r6, r9
    CMNLT           r10, #1
    MOV             r6, r6, ASR #16
    MOVLT           r7, #0x80000000
    MVNGT           r7, #0x80000000
    MOVEQ           r7, r7, LSL #10

    QADD            r7, r7, r9
    STRH            r4, [r2], #2

    MOV             r7, r7, ASR #16


    STRH            r7, [r12, r14]
    SUBS            r3, r3, #2
    STRH            r6, [r12], #2
    BGE             S_WITH_R_L5
S_WITH_R_L6:
    LDMFD           sp!, {r4-r12, r15}








