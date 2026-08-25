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
.global ixheaacd_covariance_matrix_calc_armv7


ixheaacd_covariance_matrix_calc_armv7:

    STMFD           sp!, {r4-r12, r14}
    MOVS            r12, r2
    BLE             EXIT
AUTOCORR:
    MOV             r12, r0

    MOV             r5, #9728
    LDR             r4, [r12, r5]
    ADD             r5, r5, #256
    LDR             r3, [r12, r5]

    LDR             r6, [r12], #256
    LDR             r5, [r12], #256

    MOV             r4, r4, ASR     #3
    MOV             r3, r3, ASR     #3
    MOV             r6, r6, ASR     #3
    MOV             r5, r5, ASR     #3

    SMULWT          r11, r3, r4
    SMULWT          r9, r5, r6

    SMULWT          r14, r4, r4
    SUB             r11, r9, r11


    SMULWT          r9, r6, r6


    MOV             r3, #12

    SUB             r14, r9, r14

    MOV             r7, #0
    MOV             r8, #0
    MOV             r9, #0

AUTO_CORR_RIGHT:
    LDR             r4, [r12], #256
    LDR             r10, [r12], #256

    MOV             r4, r4, ASR     #3
    SMLAWT          r9, r5, r5, r9
    SMLAWT          r7, r4, r5, r7
    SMLAWT          r8, r4, r6, r8

    MOV             r6, r10, ASR     #3
    SMLAWT          r9, r4, r4, r9
    SMLAWT          r8, r6, r5, r8

    LDR             r5, [r12], #256
    SMLAWT          r7, r6, r4, r7


    MOV             r5, r5, ASR     #3
    SMLAWT          r9, r6, r6, r9
    SMLAWT          r7, r5, r6, r7
    SMLAWT          r8, r5, r4, r8

    SUBS            r3, r3, #1
    BNE             AUTO_CORR_RIGHT

    LDR             r4, [r12], #256
    MOV             r4, r4, ASR     #3
    SMLAWT          r9, r5, r5, r9
    SMLAWT          r7, r4, r5, r7
    SMLAWT          r8, r4, r6, r8

    LDR             r6, [r12], #256
    MOV             r6, r6, ASR     #3
    SMLAWT          r9, r4, r4, r9
    SMLAWT          r7, r6, r4, r7
    SMLAWT          r8, r6, r5, r8

CAL_AUTOCORR:

    ADD             r12, r7, r11
    ADD             r14, r9, r14

    EOR             r5, r7, r7, ASR #31
    EOR             r6, r8, r8, ASR #31

    ORR             r5, r6, r5
    EOR             r6, r12, r12, ASR #31
    ORR             r5, r6, r5

    ORR             r5, r9, r5
    ORR             r5, r14, r5

    CLZ             r5, r5
    SUB             r5, r5, #1

    MOV             r7, r7, LSL r5
    MOV             r8, r8, LSL r5
    MOV             r9, r9, LSL r5
    MOV             r12, r12, LSL r5
    MOV             r14, r14, LSL r5


    STR             r9, [r1], #4
    STR             r14, [r1], #4
    STR             r7, [r1], #4


    SMULL           r6, r5, r9, r14
    SMULL           r6, r10, r12, r12

    STR             r8, [r1], #4
    STR             r12, [r1], #4

    QSUB            r10, r5, r10
    ADD             r0, r0, #4

    ADD             r1, r1, #12

    STR             r10, [r1], #4


    SUBS            r2, r2, #1
    BNE             AUTOCORR


EXIT:

    LDMFD           sp!, {r4-r12, r15}


