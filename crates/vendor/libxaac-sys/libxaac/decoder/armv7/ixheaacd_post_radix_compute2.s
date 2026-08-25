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
        .global ixheaacd_postradixcompute2
        .type ixheaacd_postradixcompute2, %function


ixheaacd_postradixcompute2:

    STMFD           sp!, {r4-r12, r14}

    SUB             sp, sp, #20












    STR             r0, [sp, #16]


    ADD             r4, r1, r3, lsl #1
    MOV             r3, r3, asr #4
    STR             r3, [sp, #4]
    MOV             r5, #1
    STR             r5, [sp]

POSTRADIX2_START:

    LDR             r14, [r2]

    LDMIA           r1!, {r5-r12}
    ADD             r0, r0, r14

    ADD             r14, r5, r7
    SUB             r5, r5, r7

    ADD             r7, r9, r11
    SUB             r9, r9, r11

    ADD             r11, r6, r8
    SUB             r6, r6, r8

    ADD             r8, r10, r12
    SUB             r10, r10, r12

    STR             r14, [r0], #4
    STR             r11, [r0], #32-4

    STR             r7, [r0], #4
    STR             r8, [r0], #(32+(32<<1))-4

    STR             r5, [r0], #4
    STR             r6, [r0], #32-4

    STR             r9, [r0], #4
    STR             r10, [r0], #0

    LDR             r0, [sp, #16]

    LDR             r14, [r2], #4

    LDMIA           r4!, {r5-r12}

    ADD             r0, r0, r14


    ADD             r0, r0, #8

    ADD             r14, r5, r7
    SUB             r5, r5, r7

    ADD             r7, r9, r11
    SUB             r9, r9, r11

    ADD             r11, r6, r8
    SUB             r6, r6, r8

    ADD             r8, r10, r12
    SUB             r10, r10, r12

    STR             r14, [r0], #4
    STR             r11, [r0], #32-4

    STR             r7, [r0], #4
    STR             r8, [r0], #(32+(32<<1))-4

    STR             r5, [r0], #4
    STR             r6, [r0], #32-4


    STR             r9, [r0], #4
    STR             r10, [r0], #0

    SUBS            r3, r3, #1


    LDR             r0, [sp, #16]
    BGT             POSTRADIX2_START

    LDR             r0, [sp, #16]

    LDR             r3, [sp, #4]
    LDR             r6, [sp]

    ADD             r1, r1, r3, lsl #5
    ADD             r4, r4, r3, lsl #5

    SUBS            r6, r6, #1
    STR             r6, [sp]


    BPL             POSTRADIX2_START


    ADD             sp, sp, #20
    LDMFD           sp!, {r4-r12, r15}


