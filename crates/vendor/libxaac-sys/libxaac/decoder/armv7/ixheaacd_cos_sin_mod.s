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
    .extern ixheaacd_radix4bfly
.hidden ixheaacd_radix4bfly
    .extern ixheaacd_postradixcompute2
.hidden ixheaacd_postradixcompute2
    .extern ixheaacd_postradixcompute4
.hidden ixheaacd_postradixcompute4




    .extern ixheaacd_sbr_imdct_using_fft
.hidden ixheaacd_sbr_imdct_using_fft


    .global ixheaacd_cos_sin_mod
ixheaacd_cos_sin_mod:
    STMFD           SP!, {R4-R12, R14}

    LDR             R5, [R1]
    MOV             R7, R5, ASR #1
    LDR             R4, [R1, #12]
    MOV             R5, R7, ASR #2

    MOV             R8, R0
    MOV             R6, R7, LSL #3


    SUB             R10, SP, #516
    SUB             SP, SP, #516

    AND             R12, R10, #7
    CMP             R12, #0
    ADDNE           R10, R10, #4







    STMFD           SP!, {R0-R3}

    SUB             R6, R6, #4
    ADD             R9, R0, R6

    LDR             R2, [R4], #4
    LDR             R1, [R9], #-4
    LDR             R0, [R8], #4
    ADD             R11, R10, R6
















LOOP1:

    SUBS            R5, R5, #1

    SMULWT          R12, R1, R2
    SMULWB          R6, R0, R2
    SMULWT          R14, R0, R2


    LDR             R0, [R8, #0xFC]

    QSUB            R12, R12, R6

    SMLAWB          R14, R1, R2, R14

    LDR             R1, [R9, #0x104]

    STR             R12, [R10, #4]
    STR             R14, [R10], #8

    SMULWT          R6, R0, R2
    SMULWB          R12, R1, R2
    SMULWT          R14, R1, R2

    LDR             R1, [R8], #4

    QSUB            R12, R12, R6

    SMLAWB          R14, R0, R2, R14

    LDR             R2, [R4], #4
    LDR             R0, [R9], #-4

    STR             R12, [R10, #0xF8]
    STR             R14, [R10, #0xFC]

    SMULWT          R3, R1, R2
    SMULWB          R6, R0, R2
    SMULWT          R12, R0, R2

    LDR             R0, [R9, #0x104]

    QSUB            R3, R3, R6

    SMLAWB          R12, R1, R2, R12

    LDR             R1, [R8, #0xFC]

    STR             R12, [R11, #-4]
    STR             R3, [R11], #-8

    SMULWT          R6, R0, R2
    SMULWB          R14, R1, R2
    SMULWT          R12, R1, R2

    LDR             R1, [R9], #-4

    QSUB            R14, R14, R6

    SMLAWB          R3, R0, R2, R12

    LDR             R2, [R4], #4
    LDR             R0, [R8], #4

    STR             R3, [R11, #0x108]
    STR             R14, [R11, #0x104]

    SMULWT          R12, R1, R2
    SMULWB          R6, R0, R2
    SMULWT          R14, R0, R2

    LDR             R0, [R8, #0xFC]

    QSUB            R12, R12, R6

    SMLAWB          R14, R1, R2, R14

    LDR             R1, [R9, #0x104]

    STR             R12, [R10, #4]
    STR             R14, [R10], #8

    SMULWT          R6, R0, R2
    SMULWB          R12, R1, R2
    SMULWT          R14, R1, R2

    LDR             R1, [R8], #4

    QSUB            R12, R12, R6

    SMLAWB          R14, R0, R2, R14

    LDR             R2, [R4], #4
    LDR             R0, [R9], #-4
    STR             R12, [R10, #0xF8]
    STR             R14, [R10, #0xFC]

    SMULWT          R3, R1, R2
    SMULWB          R6, R0, R2
    SMULWT          R12, R0, R2

    LDR             R0, [R9, #0x104]

    QSUB            R3, R3, R6
    SMLAWB          R12, R1, R2, R12

    LDR             R1, [R8, #0xFC]
    STR             R3, [R11], #-4
    STR             R12, [R11], #-4

    SMULWT          R6, R0, R2
    SMULWB          R3, R1, R2
    SMULWT          R12, R1, R2


    LDRGT           R1, [R9], #-4

    QSUB            R3, R3, R6
    SMLAWB          R12, R0, R2, R12


    LDRGT           R2, [R4], #4
    LDRGT           R0, [R8], #4

    STR             R3, [R11, #0x104]
    STR             R12, [R11, #0x108]


    BGT             LOOP1
    LDR             R1, [SP, #4]
    LDR             R5, [R1]
    LDR             R4, [SP, #8]
    LDR             R0, [SP, #8]
    ADD             R1, SP, #16


    AND             R2, R1, #7
    CMP             R2, #0
    ADDNE           R1, R1, #4


    CMP             R5, #64
    LDR             R5, [SP, #12]
    MOV             R2, #1

    BNE             THIRTY2BAND






    MOV             R2, R1
    MOV             R1, #32
    LDR             R3, [SP]
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!

    BL              ixheaacd_sbr_imdct_using_fft
    ADD             SP, SP, #16

    MOV             R0, R4
    MOV             R1, #32
    ADD             R2, SP, #16


    AND             R6, R2, #7
    CMP             R6, #0
    ADDNE           R2, R2, #4


    LDR             R3, [SP]
    ADD             R2, R2, #256
    ADD             R3, R3, #256

    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!

    BL              ixheaacd_sbr_imdct_using_fft

    ADD             SP, SP, #16

    LDR             R8, [SP]
    LDR             R12, [SP, #4]
    MOV             R3, #32
    LDR             R6, [R8]
    LDR             R11, [R8, #4]

    ADD             R9, R8, #252


    B               LOOP2_PRO

THIRTY2BAND:



    MOV             R2, R1
    MOV             R1, #16
    LDR             R3, [SP]

    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!

    BL              ixheaacd_sbr_imdct_using_fft
    ADD             SP, SP, #16

    MOV             R0, R4
    MOV             R1, #16
    ADD             R2, SP, #16


    AND             R6, R2, #7
    CMP             R6, #0
    ADDNE           R2, R2, #4


    LDR             R3, [SP]
    ADD             R2, R2, #256
    ADD             R3, R3, #256

    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!
    STR             R5, [SP, #-4]!

    BL              ixheaacd_sbr_imdct_using_fft

    ADD             SP, SP, #16
    LDR             R8, [SP]
    LDR             R12, [SP, #4]
    LDR             R6, [R8]
    LDR             R11, [R8, #4]
    ADD             R9, R8, #124




LOOP2_PRO:


















    LDR             R4, [R12, #20]
    MOV             R6, R6, ASR #1
    STR             R6, [R8], #4
    LDR             R0, [R9]
    LDR             R2, [R4], #4
    MOV             R11, R11, ASR #1
    LDR             R1, [R9, #-4]
    RSB             R12, R11, #0
    STR             R12, [R9], #-4

    SMULWT          R14, R1, R2
    SMULWB          R6, R0, R2
    SMULWT          R12, R0, R2

    LDR             R0, [R9, #260]
    QSUB            R14, R14, R6
    SMLAWB          R12, R1, R2, R12

    LDR             R6, [R8, #252]
    LDR             R11, [R8, #256]
    STR             R14, [R8], #4
    STR             R12, [R9], #-4

    MOV             R6, R6, ASR #1
    MOV             R11, R11, ASR #1
    LDR             R1, [R9, #260]

    RSB             R6, R6, #0
    STR             R6, [R9, #264]
    STR             R11, [R8, #248]

    SMULWT          R12, R0, R2
    SMULWT          R14, R1, R2
    SMULWB          R6, R0, R2
    SMLAWB          R12, R1, R2, R12

    MOV             R11, #0
    QSUB            R14, R6, R14
    QSUB            R12, R11, R12
    LDR             R0, [R8, #4]
    LDR             R1, [R8]
    STR             R12, [R8, #252]
    STR             R14, [R9, #260]

    LDR             R5, [SP, #4]
    LDR             R5, [R5]
    MOV             R5, R5, ASR #2
    SUB             R5, R5, #2







LOOP2:
    SMULWB          R12, R0, R2
    SMULWB          R14, R1, R2
    SMULWT          R6, R0, R2
    SMLAWT          R12, R1, R2, R12

    LDR             R10, [R9]
    QSUB            R14, R14, R6
    LDR             R0, [R8, #260]
    LDR             R1, [R8, #256]
    STR             R12, [R8], #4
    STR             R14, [R9], #-4

    SMULWB          R3, R0, R2
    SMULWT          R6, R0, R2
    SMULWB          R14, R1, R2
    SMLAWT          R3, R1, R2, R3

    LDR             R7, [R9, #260]
    QSUB            R6, R6, R14
    QSUB            R3, R11, R3
    LDR             R2, [R4], #4
    LDR             R1, [R9]

    STR             R3, [R9, #260]
    STR             R6, [R8, #252]

    SMULWT          R12, R10, R2
    SMULWT          R14, R1, R2
    SMULWB          R6, R10, R2
    SMLAWB          R12, R1, R2, R12

    LDR             R1, [R9, #256]
    QSUB            R14, R14, R6

    STR             R12, [R9], #-4
    STR             R14, [R8], #4

    SUBS            R5, R5, #1

    SMULWT          R12, R7, R2
    SMULWT          R14, R1, R2
    SMULWB          R6, R7, R2
    SMLAWB          R12, R1, R2, R12

    LDRGE           R0, [R8, #4]
    LDRGE           R1, [R8]

    QSUB            R12, R11, R12
    QSUB            R14, R6, R14

    STR             R12, [R8, #252]
    STR             R14, [R9, #260]

    BGE             LOOP2
ENDLOOP2:

    ADD             SP, SP, #532
    LDMFD           sp!, {r4-r12, r15}







