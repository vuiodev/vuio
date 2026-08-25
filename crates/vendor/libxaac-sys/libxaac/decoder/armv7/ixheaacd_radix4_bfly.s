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
        .global ixheaacd_radix4bfly
	.type ixheaacd_radix4bfly, %function

ixheaacd_radix4bfly:

    STMFD           sp!, {r4-r12, r14}

    SUB             sp, sp, #16

    MOV             r6, #6
    MUL             r7, r6, r3
    MOV             r4, r3
    STR             r7, [sp]



    MOV             r3, r3, lsl #1

    STR             r2, [sp, #8]
    STR             r4, [sp, #12]


    ADD             r2, r1, r3, lsl #2
    ADD             r0, r0, #8


RADIX4_OUTLOOP:
RADIX4_INLOOP:


    LDR             r6, [r1]
    LDR             r7, [r2]
    LDR             r8, [r2, r3, lsl #2]
    LDR             r9, [r2, r3, lsl #3]

    ADD             r10, r6, r8
    SUB             r11, r6, r8
    ADD             r12, r7, r9
    SUB             r14, r7, r9

    ADD             r6, r10, r12
    SUB             r7, r10, r12
    STR             r6, [r1], #4

    LDR             r8, [r1]
    LDR             r6, [r2, #4]!
    LDR             r9, [r2, r3, lsl #2]!
    LDR             r10, [r2, r3, lsl #2]!

    ADD             r12, r8, r9
    SUB             r8, r8, r9
    ADD             r9, r6, r10
    SUB             r6, r6, r10

    ADD             r10, r12, r9
    STR             r10, [r1], #4
    SUB             r12, r12, r9

    ADD             r9, r11, r6
    SUB             r10, r11, r6
    ADD             r11, r8, r14
    LDR             r5, [r0], #-4
    SUB             r6, r8, r14

    SMULWB          r14, r10, r5
    SMULWT          r8, r11, r5

    SUBS            r4, r4, #1
    SUB             r8, r8, r14
    MOV             r8, r8, lsl #1
    STR             r8, [r2], #-4

    SMULWT          r14, r10, r5
    SMLAWB          r8, r11, r5, r14
    LDR             r11, [r0], #-4
    MOV             r8, r8, lsl #1
    STR             r8, [r2], -r3, lsl #2

    SMULWT          r10, r7, r11
    SMLAWB          r8, r12, r11, r10

    LDR             r14, [r0], #20
    MOV             r5, r8, lsl #1

    SMULWB          r10, r7, r11
    SMULWT          r8, r12, r11

    STR             r5, [r2], #4
    SUB             r7, r8, r10
    MOV             r7, r7, lsl #1

    SMULWB          r11, r9, r14
    SMULWT          r12, r6, r14

    STR             r7, [r2], -r3, lsl #2
    SUB             r12, r12, r11
    MOV             r12, r12, lsl #1

    SMULWT          r10, r9, r14
    SMLAWB          r7, r6, r14, r10

    STR             r12, [r2], #-4
    MOV             r7, r7, lsl #1
    STR             r7, [r2], #8


    BNE             RADIX4_INLOOP

    LDR             r8, [sp]
    LDR             r4, [sp, #12]
    LDR             r6, [sp, #8]


    SUB             r0, r0, r8, lsl #1
    ADD             r1, r1, r8, lsl #2
    ADD             r2, r2, r8, lsl #2

    SUBS            r6, r6, #1
    STR             r6, [sp, #8]
    BNE             RADIX4_OUTLOOP



    ADD             sp, sp, #16
    LDMFD           sp!, {r4-r12, r15}


