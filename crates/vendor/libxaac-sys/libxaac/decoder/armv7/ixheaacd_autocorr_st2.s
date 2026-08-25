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
.global ixheaacd_covariance_matrix_calc_2_armv7
ixheaacd_covariance_matrix_calc_2_armv7:
    STMFD           sp!, {r4-r12, r14}

AUTO_CORR_LOOP:

    STR             r0 , [sp, #-4]!










    LDR             r4 , [r1, #-4*128]
    LDR             r5 , [r1, #4*(64-128)]
    LDR             r6 , [r1]
    LDR             r7 , [r1, #4*64]

    MOV             r4, r4, ASR #3
    MOV             r5, r5, ASR #3
    MOV             r6, r6, ASR #3
    MOV             r7, r7, ASR #3


    SMULWT          r8 , r6 , r4
    SMULWT          r9 , r7 , r4
    SMULWT          r10, r6 , r5
    SMLAWT          r8 , r7 , r5, r8
    SMULWT          r11, r4 , r4
    SUB             r9 , r9 , r10
    SMLAWT          r11, r5 , r5, r11














    MOV             r10, r1
    ADD             r12, r1, #64*4
    MOV             r4 , r6
    MOV             r5 , r7

    SUB             r14, r3 , #2
    MOVS            r14, r14, LSR #1
    BEQ             ENDLOOP2
LOOP2:
    LDR             r6 , [r10, #4*128]!
    LDR             r7 , [r12, #4*128]!

    MOV             r6, r6, ASR #3
    MOV             r7, r7, ASR #3

    SMLAWT          r8 , r6 , r4, r8
    SMLAWT          r9 , r7 , r4, r9
    SMLAWT          r8 , r7 , r5, r8
    SMULWT          r0 , r6 , r5
    SMLAWT          r11, r4 , r4, r11
    SUB             r9 , r9 , r0
    SMLAWT          r11, r5 , r5, r11

    LDR             r4  , [r10, #4*128]!
    LDR             r5  , [r12, #4*128]!

    MOV             r4, r4, ASR #3
    MOV             r5, r5, ASR #3

    SUBS            r14, r14, #1

    SMLAWT          r8 , r4 , r6, r8
    SMLAWT          r9 , r5 , r6, r9
    SMLAWT          r8 , r5 , r7, r8
    SMULWT          r0 , r4 , r7
    SMLAWT          r11, r6 , r6, r11
    SUB             r9 , r9 , r0
    SMLAWT          r11, r7 , r7, r11

    BNE             LOOP2

    ANDS            r0, r3, #0x01
    BEQ             ENDLOOP2
ODDLOOP:

    LDR             r6 , [r10, #4*128]!
    LDR             r7 , [r12, #4*128]!

    MOV             r6, r6, ASR #3
    MOV             r7, r7, ASR #3

    SMLAWT          r8 , r6 , r4, r8
    SMLAWT          r9 , r7 , r4, r9
    SMLAWT          r8 , r7 , r5, r8
    SMULWT          r0 , r6 , r5
    SMLAWT          r11, r4 , r4, r11
    SUB             r9 , r9 , r0
    SMLAWT          r11, r5 , r5, r11



ENDLOOP2:









    MOV             r12, r11
    LDR             r6 , [r1, #-8*128]
    LDR             r7 , [r1, #4*64-8*128]

    MOV             r6, r6, ASR #3
    MOV             r7, r7, ASR #3

    SMLAWT          r12, r6 , r6, r12

    SUB             r10, r3, #2
    SMLAWT          r12, r7 , r7, r12




    MOV             r0, r10, LSL #(2+7)
    ADD             r0, r0, #0x100
    LDR             r4 , [r1, r10, LSL #(2+7)]
    LDR             r5 , [r1, r0]

    MOV             r4, r4, ASR #3
    MOV             r5, r5, ASR #3

    SMLAWT          r11, r4, r4, r11
    LDR             r0 , [sp], #4
    SMLAWT          r11, r5, r5, r11

    STR             r12, [r0, #4]
    STR             r11, [r0]












    MOV             r11, r8
    LDR             r12, [r1, #-4*128]
    LDR             r14, [r1, #4*(64-128)]

    MOV             r12, r12, ASR #3
    MOV             r14, r14, ASR #3


    SMLAWT          r11, r12, r6, r11
    ADD             r10, r10, #1

    LDR             r12, [r1, r10, LSL#(2+7)]
    SMLAWT          r11, r14, r7, r11

    MOV             r14, r10, LSL #(2+7)
    ADD             r14, r14, #0x100


    MOV             r12, r12, ASR #3

    LDR             r14, [r1, r14]

    SMLAWT          r8 , r12, r4, r8

    MOV             r14, r14, ASR #3
    MOV             r10, r9

    SMLAWT          r8 , r14, r5, r8
    STR             r11, [r0, #16]
    STR             r8 , [r0, #8]












    SMLAWT          r9 , r14, r4 , r9
    SMULWT          r8 , r12, r5
    LDR             r14, [r1, #4*(64-128)]
    SUB             r9 , r9 , r8

    MOV             r14, r14, ASR #3
    LDR             r12, [r1, #-4*128]
    SMLAWT          r10, r14, r6 , r10

    MOV             r12, r12, ASR #3
    SMULWT          r8 , r12, r7
    STR             r9 , [r0, #20]
    SUB             r10, r10, r8
    STR             r10, [r0, #28]








    STR             r1 , [sp, #-4]!

    STMFD           sp!, {r0, r3}
    MOVS            r0 , r3 , LSR #2

    MOV             r12, #0
    MOV             r3 , #0
    LDR             r5 , [r1, #-8*128]
    LDR             r7 , [r1, #-4*128]
    LDR             r9 , [r1, #4*(64-256)]
    LDR             r11, [r1, #4*(64-128)]



    MOV             r5, r5, ASR #3
    MOV             r7, r7, ASR #3
    MOV             r9, r9, ASR #3
    MOV             r11, r11, ASR #3


    BEQ             ENDLOOP3
LOOP3:






    LDR             r4 , [r1], #4*128
    LDR             r8 , [r1, #4*(64-128)]

    MOV             r4, r4, ASR #3
    MOV             r8, r8, ASR #3

    SMLAWT          r12, r4 , r5 , r12
    SMLAWT          r12, r8 , r9 , r12
    SMULWT          r14, r4 , r9
    SMLAWT          r3 , r8 , r5 , r3

    LDR             r6 , [r1], #4*128
    SUB             r3 , r3 , r14








    LDR             r10, [r1, #4*(64-128)]

    MOV             r6, r6, ASR #3
    MOV             r10, r10, ASR #3

    SMLAWT          r12, r6 , r7 , r12
    SMLAWT          r12, r10, r11, r12
    SMULWT          r14, r6 , r11
    SMLAWT          r3 , r10, r7 , r3

    LDR             r5 , [r1], #4*128
    SUB             r3 , r3 , r14








    LDR             r9 , [r1, #4*(64-128)]

    MOV             r5, r5, ASR #3
    MOV             r9, r9, ASR #3

    SMLAWT          r12, r5 , r4 , r12
    SMLAWT          r12, r9 , r8 , r12
    SMULWT          r14, r5 , r8
    SMLAWT          r3 , r9 , r4 , r3

    LDR             r7 , [r1], #4*128
    SUB             r3 , r3 , r14










    LDR             r11, [r1, #4*(64-128)]

    MOV             r7, r7, ASR #3
    MOV             r11, r11, ASR #3

    SMLAWT          r12, r7 , r6 , r12
    SMLAWT          r12, r11, r10, r12
    SMULWT          r14, r7 , r10
    SMLAWT          r3 , r11, r6 , r3

    SUBS            r0 , r0 , #1
    SUB             r3 , r3 , r14

    BNE             LOOP3
ENDLOOP3:
    MOV             r4 , r3
    LDMFD           sp!, {r0, r3}

    ANDS            r5 , r3 , #3
    BEQ             ENDLOOP4

LOOP4:
    LDR             r6 , [r1, #-8*128]
    LDR             r10, [r1, #4*(64-256)]

    LDR             r7 , [r1], #4*128
    LDR             r11, [r1, #4*(64-128)]


    MOV             r6, r6, ASR #3
    MOV             r7, r7, ASR #3
    MOV             r10, r10, ASR #3
    MOV             r11, r11, ASR #3


    SMLAWT          r12, r7 , r6 , r12
    SMLAWT          r12, r11, r10, r12
    SMULWT          r14, r7 , r10
    SMLAWT          r4 , r11, r6 , r4

    SUBSNE          r5 , r5 , #1

    SUB             r4 , r4 , r14

    BNE             LOOP4
ENDLOOP4:
    STR             r12, [r0, #12]
    STR             r4 , [r0, #24]
    LDR             r1 , [sp], #4

    SUBS            R2, R2, #1

    ADD             r0, r0, #4*9


    ADD             r1, r1, #4
    BGT             AUTO_CORR_LOOP

END_OF_AUT0:

    LDMFD           sp!, {r4-r12, r15}

