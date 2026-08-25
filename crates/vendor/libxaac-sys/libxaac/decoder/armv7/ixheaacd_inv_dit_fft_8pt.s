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
        .global ixheaacd_inv_dit_fft_8pt_armv7

ixheaacd_inv_dit_fft_8pt_armv7:


    STMFD           sp!, {r4-r12, lr}
    LDR             r3, [r0, #0]
    LDR             r4, [r0, #0x20]
    LDR             r5, [r0, #0x24]
    QADD            r12, r3, r4
    LDR             r6, [r0, #0x30]
    QSUB            r8, r3, r4
    LDR             r3, [r0, #4]
    LDR             r9, [r0, #0x34]
    QADD            r4, r3, r5
    SUB             sp, sp, #0x14
    QSUB            r5, r3, r5
    LDR             lr, [r0, #0x10]
    LDR             r3, [r0, #0x14]
    QADD            r10, lr, r6
    QSUB            r6, lr, r6
    QADD            r7, r3, r9
    QSUB            r9, r3, r9


    QADD            r3, r12, r10
    QSUB            lr, r12, r10
    QADD            r12, r4, r7
    QSUB            r7, r4, r7
    QSUB            r4, r8, r9

    STR             r7, [sp, #8]
    QADD            r7, r8, r9
    QADD            r8, r5, r6
    STR             r7, [sp, #0xc]
    QSUB            r5, r5, r6

    STMIA           sp, {r8, lr}
    STR             r5, [sp, #0x10]



    LDR             r5, [r0, #8]
    LDR             lr, [r0, #0x28]
    LDR             r9, [r0, #0x2c]
    QADD            r7, r5, lr
    LDR             r11, [r0, #0x38]
    LDR             r6, [r0, #0xc]
    QSUB            r5, r5, lr
    LDR             lr, [r0, #0x18]
    QADD            r8, r6, r9
    QSUB            r6, r6, r9



    QADD            r10, lr, r11
    QSUB            r9, lr, r11
    LDR             r11, [r0, #0x1c]
    LDR             r0, [r0, #0x3c]

    MOV             lr, r11
    QADD            r11, r11, r0
    QSUB            r0, lr, r0


    QADD            lr, r7, r10
    QSUB            r10, r7, r10
    QADD            r7, r8, r11
    QSUB            r11, r8, r11

    QSUB            r8, r5, r0
    QADD            r5, r5, r0
    QADD            r0, r6, r9
    QSUB            r6, r6, r9


    QADD            r9, r3, lr
    QSUB            r3, r3, lr
    STR             r9, [r1, #0]

    QADD            r9, r12, r7
    LDR             lr, [sp, #4]
    STR             r9, [r2, #0]
    QSUB            r9, r12, r7


    QSUB            r12, lr, r11
    QADD            r11, lr, r11
    LDR             lr, [sp, #8]
    STR             r11, [r1, #0x10]
    QADD            r7, lr, r10
    QSUB            r10, lr, r10

    MOVW            r11, #0x00005a82
    STR             r10, [r2, #0x10]

    QSUB            r10, r8, r0
    QADD            r0, r8, r0
    SMULWB          r10, r10, r11
    SMULWB          r0, r0, r11
    MOV             r10, r10, LSL #1

    QADD            r8, r4, r10
    LDR             lr, [sp, #0]

    STR             r8, [r1, #4]
    MOV             r0, r0, LSL #1
    QADD            r8, lr, r0

    QSUB            r4, r4, r10
    STR             r8, [r2, #4]
    QSUB            r0, lr, r0

    QADD            r12, r12, r4
    QADD            r0, r7, r0
    STR             r12, [r1, #8]
    STR             r0, [r2, #8]

    QADD            r0, r5, r6
    LDR             r7, [sp, #0xc]
    SMULWB          r0, r0, r11

    QSUB            r12, r5, r6
    MOV             r0, r0, LSL #1
    SMULWB          r12, r12, r11
    LDR             r5, [sp, #0x10]
    QSUB            r4, r7, r0
    MOV             r12, r12, LSL #1
    QADD            r10, r5, r12
    QADD            r3, r3, r4
    QADD            lr, r9, r10
    QADD            r0, r7, r0
    QSUB            r10, r5, r12
    STR             r3, [r1, #0xc]
    STR             lr, [r2, #0xc]
    STR             r0, [r1, #0x14]
    STR             r10, [r2, #0x14]
    ADD             sp, sp, #0x14
    LDMFD           sp!, {r4-r12, pc}

