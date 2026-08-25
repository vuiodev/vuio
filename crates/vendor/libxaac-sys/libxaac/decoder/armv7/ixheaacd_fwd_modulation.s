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
 .extern ixheaacd_cos_sin_mod
.hidden ixheaacd_cos_sin_mod
 .global ixheaacd_fwd_modulation
 .type ixheaacd_fwd_modulation, %function
ixheaacd_fwd_modulation:

    STMFD           sp!, {r3-r9, r12, lr}
    MOV             r5, r2
    LDR             r2, [sp, #0x24]
    MOV             lr, r1
    MOV             r4, r1
    LDR             r1, [r3]
    SUB             r1, r1, #1
    CMP             r1, #0x3f
    MOV             r7, r5
    ADD             r8, r0, #0xfc
    ADDEQ           r8, r8, #0x100
    MOV             r6, r3
LOOP1:
    LDR             r3, [r0], #4
    LDR             r12, [r8], #-4

    MOV             r3, r3, ASR #4
    MOV             r12, r12, ASR #4

    QSUB            r9, r3, r12
    ADD             r3, r3, r12

    STR             r9, [lr], #4
    SUBS            r1, r1, #1
    STR             r3, [r7], #4

    BPL             LOOP1

    MOV             r1, r6
    MOV             r0, r4



    MOV             r3, #0xd8
    LSL             r3, r3, #4
    ADD             r3, r3, #8

    ADD             r3, r2, r3


    ADD             r2, r3, #4



    BL              ixheaacd_cos_sin_mod

    LDR             r1, [sp, #0x28]
    LDRSH           r2, [r6]
    CMP             r1, #1
    BEQ             LOOP3
    LDRSH           r1, [r6, #0x2c]
    LDRSH           r2, [r6, #0x2a]
    LDR             r0, [r6, #0x18]
    SUBS            r2, r1, r2

@   LDMLEFD         sp!, {r3-r9, r12, pc}
    LDMFDLE         sp!, {r3-r9, r12, pc}
LOOP2:
    LDR             r1, [r0], #4
    LDR             r12, [r5, #0]
    LDR             r3, [r4, #0]

    SMULWT          r6, r12, r1
    SMULWB          lr, r3, r1

    SMULWB          r12, r12, r1
    SMULWT          r1, r3, r1




    ADD             lr, lr, r6
    QSUB            r1, r12, r1
    MOV             r3, lr, LSL #1
    MOV             r1, r1, LSL #1
    STR             r3, [r4], #4
    SUBS            r2, r2, #1
    STR             r1, [r5], #4
    BGT             LOOP2
    B               END
LOOP3:
    LDR             r12, [r5, #0]
    LDR             r3, [r4, #0]

    MOV             r1, r3
    MOV             r3, #0
    SUB             r3, r3, r12

    STR             r3, [r4], #4
    STR             r1, [r5], #4

    LDR             r12, [r5, #0]
    LDR             r3, [r4, #0]

    MOV             r1, r12
    MOV             r12, #0
    SUB             r12, r12, r3

    STR             r12, [r5], #4
    STR             r1, [r4], #4

    SUBS            r2, r2, #2
    BGT             LOOP3
END:
    LDMFD           sp!, {r3-r9, r12, pc}
