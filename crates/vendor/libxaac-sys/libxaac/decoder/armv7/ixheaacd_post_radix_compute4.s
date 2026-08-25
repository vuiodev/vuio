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
        .global ixheaacd_postradixcompute4
        .type ixheaacd_postradixcompute4, %function


ixheaacd_postradixcompute4:

    STMFD           sp!, {r4-r12, r14}












    ADD             r4, r1, r3, lsl #1
    MOV             r3, #2


POSTRADIX4_START:

    LDMIA           r1!, {r5-r12}

    ADD             r14, r5, r9
    SUB             r5, r5, r9

    ADD             r9, r6, r10
    SUB             r6, r6, r10

    ADD             r10, r7, r11
    SUB             r7, r7, r11

    ADD             r11, r8, r12
    SUB             r8, r8, r12

    ADD             r12, r14, r10
    SUB             r14, r14, r10

    ADD             r10, r9, r11
    SUB             r9, r9, r11

    ADD             r11, r5, r8
    SUB             r5, r5, r8

    ADD             r8, r6, r7
    SUB             r6, r6, r7


    STR             r12, [r0], #4
    STR             r10, [r0], #14<<1

    STR             r11, [r0], #4
    STR             r6 , [r0], #14<<1

    STR             r14, [r0], #4
    STR             r9 , [r0], #14<<1

    STR             r5, [r0], #4
    STR             r8, [r0], #0

    LDMIA           r4!, {r5-r12}
    SUB             r0, r0, #92


    ADD             r14, r5, r9
    SUB             r5, r5, r9

    ADD             r9, r6, r10
    SUB             r6, r6, r10

    ADD             r10, r7, r11
    SUB             r7, r7, r11

    ADD             r11, r8, r12
    SUB             r8, r8, r12

    ADD             r12, r14, r10
    SUB             r14, r14, r10

    ADD             r10, r9, r11
    SUB             r9, r9, r11

    ADD             r11, r5, r8
    SUB             r5, r5, r8

    ADD             r8, r6, r7
    SUB             r6, r6, r7

    STR             r12, [r0], #4
    STR             r10, [r0], #14<<1

    STR             r11, [r0], #4
    STR             r6, [r0], #14<<1

    STR             r14, [r0], #4
    STR             r9, [r0], #14<<1


    STR             r5, [r0], #4
    STR             r8, [r0], #0

    ADD             r1, r1, #1 << 5
    ADD             r4, r4, #1 << 5
    SUB             r0, r0, #100-8

    SUBS            r3, r3, #1

    BGT             POSTRADIX4_START

    LDMFD           sp!, {r4-r12, r15}


