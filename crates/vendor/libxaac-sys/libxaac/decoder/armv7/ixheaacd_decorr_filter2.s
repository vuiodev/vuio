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
.global ixheaacd_decorr_filter2_armv7
    @PRESERVE8

ixheaacd_decorr_filter2_armv7:

    STMFD           r13!, {r4-r12, r14}
    LDR             r12, [r0, #24]

    LDRSH           r4, [r0, #12]
    LDRSH           r5, [r0, #14]
    LDRSH           r6, [r0, #16]

    MOV             r11, #384
    LDR             r9, [r13, #44]
    LDR             r14, [r13, #48]
    LDR             r10, [r13, #40]

    MLA             r4, r11, r4, r12

    MLA             r5, r11, r5, r12

    MLA             r6, r11, r6, r12


    ADD             r11, r9, #0x0150
    LDRH            r7, [r0, #28]
    ADD             r2, r2, #12
    ADD             r10, r10, #12

    ADD             r5, r5, #140

    ADD             r6, r6, #268


    STR             r10, [r13, #-4]!
    STR             r2, [r13, #-4]!
    LDR             r8, [r0]
    ADD             r11, r11, #0x0c

    MOV             r0, #128
    ADD             r2, r9, #0x012
    ADD             r12, r11, #0x0a0


    SUB             r10, r11, #0x03A
    ADD             r3, r3, #12
    MLA             r0, r7, r0, r8




    STR             r10, [r13, #-8]!
    STR             r3, [r13, #-4]!
    STR             r14, [r13, #-4]!
    ADD             r1, r1, #12
    ADD             r0, r0, #12
    ADD             r4, r4, #12






DE_COR_FIL2_LOOP1:




































    ADD             r12, r12, #12


    ADD             r11, r11, #12




    MOV             r9, #20

    STR             r9, [r13, #-4]!
    STR             r1, [r13, #16]


DE_COR_FIL2_LOOP2:

    LDR             r7, [r0]
    LDR             r8, [r11], #4
    LDR             r14, [r1], #4

    SMULBT          r3, r7, r8
    SMULBB          r9, r7, r8


    STR             r1, [r13, #16]

    SMULTT          r10, r7, r8

    LDR             r1, [r13, #20]

    SMLATB          r3, r7, r8, r3

    MOV             r8, #0x8000

    QSUB            r9, r9, r10
    LDR             r10, [r1], #4
    QADD            r14, r14, r8

    STR             r1, [r13, #20]
    QADD            r1, r10, r8

    MOV             r7, r9, asr #15
    MOV             r8, r3, asr #15

    LDR             r10, [r12], #4
    LDR             r9, [r4]


    MOV             r14, r14, asr #16
    MOV             r1, r1, asr #16

    SMULBT          r3, r10, r9


    STRH            r14, [r0], #2
    STRH            r1, [r0], #2





    SMULTT          r14, r10, r9
    SMLATB          r3, r10, r9, r3
    SMULBB          r9, r10, r9

    LDRSH           r10, [r2], #2
    MOV             r3, r3, asr #15
    QSUB            r9, r9, r14

    SMULBB          r14, r10, r8
    SMULBB          r1, r10, r7
    MOV             r9, r9, asr #15

    SUB             r3, r3, r14, asr #15
    SUB             r9, r9, r1, asr #15

    SMULBB          r14, r9, r10
    SMULBB          r1, r3, r10

    LDR             r10, [r12, #124 ]

    ADD             r14, r7, r14, asr #15
    LDR             r7, [r5]

    ADD             r1, r8, r1, asr #15

    SMULBT          r8, r10, r7

    STRH            r14, [r4], #2
    STRH            r1, [r4], #2

    SMULTT          r14, r10, r7
    SMLATB          r8, r10, r7, r8
    SMULBB          r7, r10, r7

    LDRSH           r10, [r2], #2
    MOV             r8, r8, asr #15
    QSUB            r7, r7, r14

    SMULBB          r14, r10, r3
    SMULBB          r1, r10, r9
    MOV             r7, r7, asr #15

    SUB             r8, r8, r14, asr #15
    SUB             r7, r7, r1, asr #15

    SMULBB          r14, r7, r10
    SMULBB          r1, r8, r10

    LDR             r10, [r12, #252]

    ADD             r14, r9, r14, asr #15
    LDR             r9, [r6]

    ADD             r1, r3, r1, asr #15

    SMULBT          r3, r10, r9


    STRH            r14, [r5], #2
    STRH            r1, [r5], #2

    SMULTT          r14, r10, r9
    SMLATB          r3, r10, r9, r3
    SMULBB          r9, r10, r9

    LDRSH           r10, [r2], #2
    MOV             r3, r3, asr #15
    QSUB            r9, r9, r14

    SMULBB          r14, r10, r8
    SMULBB          r1, r10, r7
    MOV             r9, r9, asr #15

    SUB             r3, r3, r14, asr #15
    SUB             r9, r9, r1, asr #15

    SMULBB          r14, r9, r10
    SMULBB          r1, r3, r10

    LDR             r10, [r13, #12]
    ADD             r7, r7, r14, asr #15
    ADD             r1, r8, r1, asr #15

    LDRH            r8, [r10], #2
    STR             r10, [r13, #12]
    STRH            r7, [r6], #2
    LDR             r10, [r13, #4]

    STRH            r1, [r6], #2

    MOV             r8, r8, lsl #1
    LDRH            r8, [r10, r8]
    LDR             r10, [r13, #8]

    LDR             r1, [r13, #16]
    LDR             r14, [r13]
    LDR             r7, [r13, #24]

    SMULBB          r3, r3, r8
    SMULBB          r9, r9, r8

    SUBS            r14, r14, #1
    MOV             r3, r3, lsl #1
    MOV             r9, r9, lsl #1

    STR             r3, [r7] , #4
    STR             r9, [r10], #4
    STR             r14, [r13]
    STR             r10, [r13, #8]
    STR             r7, [r13, #24]
    BGT             DE_COR_FIL2_LOOP2

    ADD             r13, r13, #28

    LDMFD           r13!, {r4-r12, pc}




.text
.p2align 2
.global ixheaacd_decorr_filter1_armv7

ixheaacd_decorr_filter1_armv7:

    STMFD           r13!, {r4-r12, r14}
    ADD             r12, r0, #0x0bc

    LDRSH           r4, [r0, #12]
    LDRSH           r5, [r0, #14]
    LDRSH           r6, [r0, #16]

    MOV             r11, #192
    LDR             r3, [r0, #0x2c]

    MLA             r4, r11, r4, r12

    MLA             r5, r11, r5, r12

    MLA             r6, r11, r6, r12


    ADD             r11, r1, #0x01b0
    LDRH            r7, [r0, #28]

    ADD             r5, r5, #64

    ADD             r6, r6, #128


    ADD             r8, r0, #0x03c
    ADD             r11, r11, #0x0c

    MOV             r0, #64
    ADD             r12, r11, #0x1c0


    SUB             r10, r11, #0x0b4
    MLA             r0, r7, r0, r8


    MOV             r9, #10
    STR             r10, [r13, #-4]!
    STR             r3, [r13, #-4]!
    STR             r2, [r13, #-4]!
    STR             r9, [r13, #-4]!
    ADD             r2, r1, #0x096
    MOV             r1, r3



DE_COR_FIL1_LOOP:

    LDR             r7, [r0]
    LDR             r8, [r11], #4
    LDR             r14, [r1], #4

    SMULBT          r3, r7, r8
    SMULBB          r9, r7, r8

    STR             r1, [r13, #8]

    SMULTT          r10, r7, r8
    SMLATB          r3, r7, r8, r3

    MOV             r8, #0x8000

    QSUB            r9, r9, r10
    LDR             r10, [r1, #0x3c]
    QADD            r14, r14, r8
    QADD            r1, r10, r8

    MOV             r7, r9, asr #15
    MOV             r8, r3, asr #15

    LDR             r10, [r12], #4
    LDR             r9, [r4]


    MOV             r14, r14, asr #16
    MOV             r1, r1, asr #16

    SMULBT          r3, r10, r9

    STRH            r14, [r0], #2
    STRH            r1, [r0], #2





    SMULTT          r14, r10, r9
    SMLATB          r3, r10, r9, r3
    SMULBB          r9, r10, r9

    LDRSH           r10, [r2]
    MOV             r3, r3, asr #15
    QSUB            r9, r9, r14

    SMULBB          r14, r10, r8
    SMULBB          r1, r10, r7
    MOV             r9, r9, asr #15

    SUB             r3, r3, r14, asr #15
    SUB             r9, r9, r1, asr #15

    SMULBB          r14, r9, r10
    SMULBB          r1, r3, r10

    LDR             r10, [r12, #60 ]

    ADD             r14, r7, r14, asr #15
    LDR             r7, [r5]

    ADD             r1, r8, r1, asr #15

    SMULBT          r8, r10, r7
    STRH            r14, [r4], #2
    STRH            r1, [r4], #2

    SMULTT          r14, r10, r7
    SMLATB          r8, r10, r7, r8
    SMULBB          r7, r10, r7

    LDRSH           r10, [r2, #2]
    MOV             r8, r8, asr #15
    QSUB            r7, r7, r14

    SMULBB          r14, r10, r3
    SMULBB          r1, r10, r9
    MOV             r7, r7, asr #15

    SUB             r8, r8, r14, asr #15
    SUB             r7, r7, r1, asr #15

    SMULBB          r14, r7, r10
    SMULBB          r1, r8, r10

    LDR             r10, [r12, #124]

    ADD             r14, r9, r14, asr #15
    LDR             r9, [r6]

    ADD             r1, r3, r1, asr #15

    SMULBT          r3, r10, r9

    STRH            r14, [r5], #2
    STRH            r1, [r5], #2

    SMULTT          r14, r10, r9
    SMLATB          r3, r10, r9, r3
    SMULBB          r9, r10, r9

    LDRSH           r10, [r2, #4]
    MOV             r3, r3, asr #15
    QSUB            r9, r9, r14

    SMULBB          r14, r10, r8
    SMULBB          r1, r10, r7
    MOV             r9, r9, asr #15

    SUB             r3, r3, r14, asr #15
    SUB             r9, r9, r1, asr #15

    SMULBB          r14, r9, r10
    SMULBB          r1, r3, r10

    LDR             r10, [r13, #12]
    ADD             r7, r7, r14, asr #15
    ADD             r1, r8, r1, asr #15

    LDRH            r8, [r10], #2
    STR             r10, [r13, #12]
    STRH            r7, [r6], #2
    LDR             r10, [r13, #4]

    STRH            r1, [r6], #2

    MOV             r8, r8, lsl #1
    LDRH            r8, [r10, r8]

    LDR             r14, [r13]
    LDR             r1, [r13, #8]

    SMULBB          r3, r3, r8
    SMULBB          r9, r9, r8

    SUBS            r14, r14, #1
    MOV             r3, r3, lsl #1
    MOV             r9, r9, lsl #1

    STR             r3, [r1 , #0x0bc]
    STR             r9, [r1, #0x7c]
    STR             r14, [r13]
    BGT             DE_COR_FIL1_LOOP

    ADD             r13, r13, #16

    LDMFD           r13!, {r4-r12, pc}











.text
.p2align 2
.global ixheaacd_divide16_pos_armv7

ixheaacd_divide16_pos_armv7:
    CMP             r1, #0
    MOV             r2, r1
@    MVNLTS      r2, r2
    MVNSLT          r2, r2
    CLZ             r2, r2
    SUB             r2, r2, #1
    MOV             r0, r0, LSL r2
    MOV             r1, r1, LSL r2
    MOV             r2, r1, LSR #16
    MOV             r0, r0, LSR #16
    MOVS            r0, r0, LSL #16
    MOV             r2, r2, LSL #16
    BXEQ            r14
    MOV             r1, #0x10
    MOV             r3, #1
FFR_DIV_LOOP:
    CMP             r0, r2
    MOVCC           r0, r0, LSL #1
    SUBCS           r0, r0, r2
    ADDCS           r0, r3, r0, LSL #1
    SUBS            r1, r1, #1
    BGT             FFR_DIV_LOOP
    BX              r14



















.text
.p2align 2
.global ixheaacd_decorrelation_armv7




ixheaacd_decorrelation_armv7:

    STMFD           r13!, {r4-r12, r14}

    LDR             r4, [r13, #40]
    LDR             r5, [r13, #44]
    LDR             r6, [r0, #0x2c]
    STR             r1, [r13, #-4]!
    STR             r2, [r13, #-4]!
    STR             r3, [r13, #-4]!
    STR             r4, [r13, #-4]!
    SUB             r13, r13, #124


    LDR             r7, [r6]
    LDR             r8, [r6, #20]
    LDR             r9, [r6, #64]
    LDR             r10, [r6, #84]

    SMULWT          r11, r7, r7
    SMULWT          r12, r9, r9
    SMLAWT          r11, r8, r8, r11
    SMLAWT          r12, r10, r10, r12

    LDR             r10, [r6, #16]
    LDR             r8, [r6, #80]
    LDR             r9, [r6, #4]
    QADD            r14, r11, r12
    LDR             r7, [r6, #68]
    SMULWT          r11, r10, r10
    STR             r14, [r13]

    SMULWT          r12, r9, r9
    SMLAWT          r11, r7, r7, r11
    LDRSH           r7, [r5, #0x0aa]
    SMLAWT          r12, r8, r8, r12



    ADD             r10, r6, #0x040
    LDR             r8, [r6, r7, lsl #2]
    QADD            r14, r11, r12
    LDR             r9, [r10, r7, lsl #2]
    LDRSH           r7, [r5, #0x0ac]
    SMULWT          r11, r8, r8
    STR             r14, [r13, #4]

    LDR             r8, [r6, r7, lsl #2]
    SMLAWT          r12, r9, r9, r11
    LDR             r9, [r10, r7, lsl #2]
    LDRSH           r7, [r5, #0x0ae]
    SMULWT          r11, r8, r8
    STR             r12, [r13, #8]

    LDR             r8, [r6, r7, lsl #2]
    SMLAWT          r12, r9, r9, r11
    LDR             r9, [r10, r7, lsl #2]
    LDRSH           r7, [r5, #0x0b0]
    SMULWT          r11, r8, r8
    STR             r12, [r13, #12]

    LDR             r8, [r6, r7, lsl #2]
    SMLAWT          r12, r9, r9, r11
    LDR             r9, [r10, r7, lsl #2]
    LDRSH           r7, [r5, #0x0b2]
    SMULWT          r11, r8, r8
    STR             r12, [r13, #16]

    LDR             r8, [r6, r7, lsl #2]
    SMLAWT          r12, r9, r9, r11
    LDR             r9, [r10, r7, lsl #2]
    LDRSH           r7, [r5, #0x0b4]
    SMULWT          r11, r8, r8
    STR             r12, [r13, #20]

    LDR             r8, [r6, r7, lsl #2]
    SMLAWT          r12, r9, r9, r11
    LDR             r9, [r10, r7, lsl #2]
    LDR             r6, [r1, #12]
    SMULWT          r11, r8, r8
    LDR             r7, [r2, #12]
    SMLAWT          r14, r9, r9, r11
    STR             r12, [r13, #24]


    LDR             r8, [r1, #16]
    SMULWT          r10, r6, r6
    LDR             r9, [r2, #16]
    SMLAWT          r12, r7, r7, r10
    STR             r14, [r13, #28]

    SMULWT          r10, r8, r8
    LDR             r6, [r1, #20]
    SMLAWT          r14, r9, r9, r10

    LDR             r7, [r2, #20]
    STR             r12, [r13, #32]

    LDR             r8, [r1, #24]
    SMULWT          r10, r6, r6
    LDR             r9, [r2, #24]
    SMLAWT          r12, r7, r7, r10
    STR             r14, [r13, #36]

    SMULWT          r10, r8, r8
    LDR             r6, [r1, #28]
    SMLAWT          r14, r9, r9, r10

    LDR             r7, [r2, #28]
    STR             r12, [r13, #40]


    LDR             r8, [r1, #32]
    SMULWT          r10, r6, r6
    LDR             r9, [r2, #32]
    SMLAWT          r12, r7, r7, r10
    STR             r14, [r13, #44]

    SMULWT          r10, r8, r8
    ADD             r7, r0, #0x06c0
    SMLAWT          r14, r9, r9, r10
    STR             r12, [r13, #48]
    LDRSH           r12, [r7, #2]
    STR             r14, [r13, #52]
    STR             r0, [r13, #-4]!
    STR             r5, [r13, #-4]!


    ADD             r3, r5, #0x0c2
    MOV             r0, #6
    ADD             r4, r13, #64

DE_COR_LOOP1:

    LDRSH           r6, [r3, #2]
    LDRSH           r7, [r3], #2
    MOV             r14, #0
    CMP             r6, r12
    MOVGT           r6, r12


    SUBS            r6, r6, r7


    ADD             r8, r1, r7, lsl #2
    ADD             r9, r2, r7, lsl #2
    BLE             DE_COR_NEXT1
    LDR             r10, [r8], #4
    LDR             r11, [r9], #4
    LDRSH           r7, [r3, #0x0c]


DE_COR_LOOP1_1:

    SMULWT          r5, r10, r10
    SUBS            r6, r6, #1
    SMLAWT          r5, r11, r11, r5
    LDR             r10, [r8], #4
    LDR             r11, [r9], #4
    MOV             r5, r5, asr  r7
    QADD            r14, r14, r5
    BGT             DE_COR_LOOP1_1

DE_COR_NEXT1:
    SUBS            r0, r0, #1
    STR             r14, [r4], #4
    BGT             DE_COR_LOOP1

    LDR             r5, [r13], #4
    LDR             r0, [r13], #4

    MOV             r14, #20
    MOV             r7, r13
    LDR             r3, [r0, #0x020]
    LDR             r4, [r0, #0x028]
    MOVW            r12, #0x620a
    LDR             r6, [r0, #0x024]
    STR             r0, [r13, #-4]!
    STR             r5, [r13, #-4]!
    MOV             r11, #0x6000
    MOV             r8, #0x0ff
    ADD             r5, r7, #80
    ORR             r8, r8, #0x07f00

DE_COR_LOOP2:

    LDR             r9, [r3]
    LDR             r2, [r7]
    LDR             r10, [r4]
    SMULWB          r9, r9, r12

    MOVS            r2, r2, lsl #1
    SMULWB          r10, r10, r11
    MOVLT           r2, #0
    MOV             r9, r9, lsl #1
    CMP             r2, r9
    MOVGT           r9, r2

    STR             r9, [r3], #4
    QSUB            r9, r9, r2

    MOV             r10, r10, lsl #1
    STR             r2, [r7], #4
    MOV             r9, r9, asr #2
    QADD            r9, r9, r10




    LDR             r10, [r6]
    MOV             r2, r2, asr #2
    STR             r9, [r4], #4
    SMULWB          r10, r10, r11


    MOV             r0, r9, asr #1
    QADD            r1, r9, r0
    MOV             r10, r10, lsl #1
    ADD             r0, r10, r2


    STR             r0, [r6], #4
    CMP             r1, r0

    MOVLE           r0, r8
    BLE             DE_COR_NEXT2








    MOVS            r2, r1
    MVNLT           r2, r2
    CLZ             r2, r2
    SUB             r2, r2, #1
    MOV             r0, r0, lsl r2
    MOV             r1, r1, lsl r2
    MOV             r2, r1, lsr #16
    MOV             r0, r0, lsr #16
    MOVS            r0, r0, lsl #16
    MOV             r2, r2, lsl #16
    BEQ             DE_COR_NEXT1_1
    MOV             r1, #04
    MOV             r10, #1


DE_COR_DIV_LOOP:
    SUBS            r9, r0, r2
    MOVCC           r0, r0, lsl #1

    ADDCS           r0, r10, r9, lsl #1


    SUBS            r9, r0, r2
    MOVCC           r0, r0, lsl #1

    ADDCS           r0, r10, r9, lsl #1


    SUBS            r9, r0, r2
    MOVCC           r0, r0, lsl #1

    ADDCS           r0, r10, r9, lsl #1


    SUBS            r9, r0, r2
    MOVCC           r0, r0, lsl #1

    ADDCS           r0, r10, r9, lsl #1


    SUBS            r1, r1, #1
    BGT             DE_COR_DIV_LOOP



DE_COR_NEXT1_1:




    MOV             r0, r0, lsl #16
    MOV             r0, r0, asr #16

DE_COR_NEXT2:

    STRH            r0, [r5], #2
    SUBS            r14, r14, #1
    BGT             DE_COR_LOOP2

    LDR             r5, [r13], #4
    LDR             r0, [r13]
    MOV             r1, r5
    ADD             r2, r13, #84
    BL              ixheaacd_decorr_filter1_armv7

    LDR             r0, [r13]
    LDR             r1, [r13, #140]
    LDR             r2, [r13, #136]

    ADD             r7, r13, #84
    MOV             r8, #0

    LDR             r3, [r13, #132]
    LDR             r6, [r13, #128]

    STRH            r8, [r7, #40]
    STR             r7, [r13, #-4]!
    STR             r5, [r13, #-4]!
    STR             r6, [r13, #-4]!
    BL              ixheaacd_decorr_filter2_armv7




    ADD             r13, r13, #12
    LDR             r0, [r13], #4

    LDR             r1, [r13, #136]
    LDR             r2, [r13, #132]
    ADD             r6, r0, #0x06c0
    LDR             r3, [r13, #128]
    LDR             r4, [r13, #124]

    LDRSH           r10, [r0, #0x1e]
    LDR             r9, [r0, #4]
    LDRSH           r12, [r6, #2]
    LDRSH           r14, [r5, #0x0cc]
    MOV             r6, #0x030
    LDRSH           r11, [r5, #0x0ca]
    MLA             r6, r10, r6, r9




    ADD             r10, r10, #1

    CMP             r10, #14
    MOVGE           r10, #0


    CMP             r12, r14
    MOVLT           r14, r12
    STRH            r10, [r0, #0x1e]
    ADD             r1, r1, r11, lsl #2
    ADD             r2, r2, r11, lsl #2
    ADD             r3, r3, r11, lsl #2
    ADD             r4, r4, r11, lsl #2
    LDRSH           r8, [r7, #36]
    SUBS            r14, r14, r11
    MOV             r7, #0x08000
    BLE             DE_COR_NEXT3



DE_COR_LOOP3:

    LDR             r10, [r1], #4
    LDR             r11, [r2], #4
    LDR             r9, [r6]
    QADD            r10, r10, r7
    QADD            r11, r11, r7
    SUBS            r14, r14, #1

    MOV             r10, r10, asr #16
    STRH            r10, [r6], #2

    SMULBB          r10, r9, r8


    MOV             r11, r11, asr #16
    STRH            r11, [r6], #2

    SMULTB          r11, r9, r8


    MOV             r10, r10, lsl #1
    STR             r10, [r3], #4

    MOV             r11, r11, lsl #1
    STR             r11, [r4], #4
    BGT             DE_COR_LOOP3

DE_COR_NEXT3:
    LDR             r1, [r13, #136]
    LDR             r2, [r13, #132]
    LDR             r3, [r13, #128]
    LDR             r4, [r13, #124]

    LDRSH           r8, [r13, #118]
    LDRSH           r14, [r5, #0x0ce]
    LDRSH           r11, [r5, #0x0cc]
    LDR             r6, [r0, #8]
    LDRSH           r8, [r13, #118]

    CMP             r12, r14
    MOVLT           r14, r12

    ADD             r1, r1, r11, lsl #2
    ADD             r2, r2, r11, lsl #2
    ADD             r3, r3, r11, lsl #2
    ADD             r4, r4, r11, lsl #2

    SUBS            r14, r14, r11
    BLE             DE_COR_NEXT4



DE_COR_LOOP4:
    LDR             r10, [r1], #4
    LDR             r11, [r2], #4
    LDR             r9, [r6]
    QADD            r10, r10, r7
    QADD            r11, r11, r7
    SUBS            r14, r14, #1

    MOV             r10, r10, asr #16
    STRH            r10, [r6], #2

    SMULBB          r10, r9, r8


    MOV             r11, r11, asr #16
    STRH            r11, [r6], #2

    SMULTB          r11, r9, r8

    MOV             r10, r10, lsl #1
    STR             r10, [r3], #4

    MOV             r11, r11, lsl #1
    STR             r11, [r4], #4
    BGT             DE_COR_LOOP4

DE_COR_NEXT4:

    LDR             r3, [r13, #128]
    LDR             r4, [r13, #124]

    RSBS            r14, r12, #64


    MOV             r9, #0
    LDRSH           r10, [r0, #0x01c]
    ADD             r3, r3, r12, lsl #2
    ADD             r4, r4, r12, lsl #2
    BLE             DE_COR_NEXT5

DE_COR_LOOP5:

    STR             r9, [r3], #4
    STR             r9, [r4], #4
    SUBS            r14, r14, #1
    BGT             DE_COR_LOOP5

DE_COR_NEXT5:


    ADD             r10, r10, #1
    CMP             r10, #2
    MOVGE           r10, #0

    STRH            r10, [r0, #0x01c]



    LDRSH           r1, [r0, #0x0c]
    LDRSH           r4, [r0, #0x12]
    LDRSH           r2, [r0, #0x0e]
    LDRSH           r5, [r0, #0x14]
    LDRSH           r3, [r0, #0x10]
    LDRSH           r6, [r0, #0x16]

    ADD             r13, r13, #140
    ADD             r1, r1, #1
    ADD             r2, r2, #1
    ADD             r3, r3, #1

    CMP             r1, r4
    MOVGE           r1, #0



    CMP             r2, r5
    MOVGE           r2, #0



    CMP             r3, r6
    MOVGE           r3, #0



    STRH            r1, [r0, #0x0c]
    STRH            r2, [r0, #0x0e]
    STRH            r3, [r0, #0x10]

    LDMFD           r13!, {r4-r12, pc}

