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
        .global ixheaacd_shiftrountine
        .type ixheaacd_shiftrountine, %function
ixheaacd_shiftrountine:
    CMP             r3, #0
    STMFD           sp!, {r4-r7, r12}
    MOV             r12, #0x1f
    BGE             SROUTINE_L1
    RSB             r3, r3, #0
    CMP             r3, r12
    MOVGT           r3, r12
    SUBS            r2, r2, #2
@    LDMMIFD     sp!, {r4-r7, r12}
    LDMFDMI         sp!, {r4-r7, r12}
    BXMI            lr
SROUTINE_L2:
    LDR             r12, [r0, #0]
    LDR             r4, [r1, #0]
    MOV             r12, r12, ASR r3
    MOV             r4, r4, ASR r3
    STR             r12, [r0], #4
    STR             r4, [r1], #4

    LDR             r12, [r0, #0]
    LDR             r4, [r1, #0]
    MOV             r12, r12, ASR r3
    MOV             r4, r4, ASR r3
    SUBS            r2, r2, #2
    STR             r12, [r0], #4
    STR             r4, [r1], #4

    BPL             SROUTINE_L2
    LDMFD           sp!, {r4-r7, r12}
    BX              lr
SROUTINE_L1:
    SUBS            r4, r2, #2
    RSB             r2, r3, #0x1f
@    LDMMIFD     sp!, {r4-r7, r12}
    LDMFDMI         sp!, {r4-r7, r12}
    BXMI            lr
SROUTINE_L3:
    LDR             r12, [r0, #0]
    LDR             r5, [r1, #0]

    MOVS            r7, r12, ASR r2
    CMNLT           r7, #1
    MOVLT           r6, #0x80000000
    MVNGT           r6, #0x80000000
    MOVEQ           r6, r12, LSL r3

    MOVS            r7, r5, ASR r2
    CMNLT           r7, #1
    MOVLT           r12, #0x80000000
    MVNGT           r12, #0x80000000
    MOVEQ           r12, r5, LSL r3
    STR             r6, [r0], #4
    STR             r12, [r1], #4

    LDR             r12, [r0, #0]
    LDR             r5, [r1, #0]

    MOVS            r7, r12, ASR r2
    CMNLT           r7, #1
    MOVLT           r6, #0x80000000
    MVNGT           r6, #0x80000000
    MOVEQ           r6, r12, LSL r3

    MOVS            r7, r5, ASR r2
    CMNLT           r7, #1
    MOVLT           r12, #0x80000000
    MVNGT           r12, #0x80000000
    MOVEQ           r12, r5, LSL r3
    SUBS            r4, r4, #2
    STR             r6, [r0], #4
    STR             r12, [r1], #4

    BPL             SROUTINE_L3
    LDMFD           sp!, {r4-r7, r12}
    BX              lr


